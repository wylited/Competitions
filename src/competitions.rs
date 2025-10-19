use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use futures_util::TryStreamExt;
use mongodb::{options::FindOptions, Collection, bson::{doc, oid::ObjectId}};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::{models::Competition, AppState, ApiResponse};

/// Query parameters for filtering competitions
#[derive(Debug, Deserialize)]
pub struct CompetitionQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
}

/// Response for paginated results
#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub limit: u32,
    pub total: u64,
}

/// Helper function to create MongoDB filter from query parameters using functional approach
fn build_competition_filter(query: &CompetitionQuery) -> mongodb::bson::Document {
    let mut filter = doc! {};
    
    // Using functional approach to apply filters
    let filters = vec![
        query.status.as_ref().map(|status| ("status", status.as_str())),
        query.host.as_ref().map(|host| ("host", host.as_str())),
    ];
    
    for filter_opt in filters {
        if let Some((key, value)) = filter_opt {
            filter.insert(key, value);
        }
    }
    
    // Handle date filters separately since they require parsing
    if let Some(date_from) = &query.date_from {
        if let Ok(from_date) = date_from.parse::<DateTime<Utc>>() {
            // Convert to BsonDateTime for MongoDB using timestamp milliseconds
            let bson_date = mongodb::bson::DateTime::from_millis(from_date.timestamp_millis());
            filter.insert("date", doc! { "$gte": bson_date });
        }
    }
    
    if let Some(date_to) = &query.date_to {
        if let Ok(to_date) = date_to.parse::<DateTime<Utc>>() {
            // Convert to BsonDateTime for MongoDB using timestamp milliseconds
            let bson_date = mongodb::bson::DateTime::from_millis(to_date.timestamp_millis());
            match filter.get_mut("date") {
                Some(mongodb::bson::Bson::Document(date_doc)) => {
                    date_doc.insert("$lte", bson_date);
                }
                _ => {
                    filter.insert("date", doc! { "$lte": bson_date });
                }
            }
        }
    }
    
    filter
}

/// Helper function to get collection reference
fn get_competition_collection(state: &AppState) -> Collection<Competition> {
    state.db.collection("competitions")
}

/// Helper function to create pagination options
fn create_pagination_options(page: u32, limit: u32) -> FindOptions {
    let skip = (page.saturating_sub(1)) * limit;
    
    FindOptions::builder()
        .skip(Some(skip as u64))
        .limit(Some(limit as i64))
        .sort(Some(doc! { "date": 1 }))
        .build()
}

/// Functional helper to process results from MongoDB cursor
async fn process_competition_cursor(
    mut cursor: mongodb::Cursor<Competition>
) -> Result<Vec<Competition>, StatusCode> {
    let mut competitions = Vec::new();
    while let Some(competition) = cursor
        .try_next()
        .await
        .map_err(|e| {
            tracing::error!("Error fetching competition from cursor: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        competitions.push(competition);
    }
    Ok(competitions)
}

/// Get all competitions with optional filtering and pagination
pub async fn get_competitions(
    State(state): State<AppState>,
    query: Option<Query<CompetitionQuery>>,
) -> Result<Json<ApiResponse<PaginatedResponse<Competition>>>, StatusCode> {
    let collection = get_competition_collection(&state);
    
    let query_params = query.unwrap_or(Query(CompetitionQuery {
        page: None,
        limit: None,
        status: None,
        host: None,
        date_from: None,
        date_to: None,
    }));
    
    let filter = build_competition_filter(&query_params.0);
    
    // Pagination
    let page = query_params.page.unwrap_or(1).max(1);
    let limit = query_params.limit.unwrap_or(10).min(100); // Max 100 per page
    
    let options = create_pagination_options(page, limit);
    
    // Get total count using functional composition
    let total = collection
        .count_documents(filter.clone(), None)
        .await
        .map_err(|e| {
            tracing::error!("Error counting competitions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Get competitions using functional approach
    let cursor = collection
        .find(filter, options)
        .await
        .map_err(|e| {
            tracing::error!("Error finding competitions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let competitions = process_competition_cursor(cursor).await?;
    
    let paginated_response = PaginatedResponse {
        data: competitions,
        page,
        limit,
        total,
    };
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(paginated_response),
        message: Some("Competitions retrieved successfully".to_string()),
    }))
}

/// Get a specific competition by ID
pub async fn get_competition_by_id(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Competition>>, StatusCode> {
    let collection = get_competition_collection(&state);
    
    // Validate and convert string ID to ObjectId
    let object_id = ObjectId::parse_str(&id)
        .map_err(|e| {
            tracing::error!("Invalid ObjectId: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    match collection
        .find_one(doc! { "_id": object_id }, None)
        .await
        .map_err(|e| {
            tracing::error!("Error finding competition by ID: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        Some(competition) => Ok(Json(ApiResponse {
            success: true,
            data: Some(competition),
            message: Some("Competition retrieved successfully".to_string()),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Create a new competition
pub async fn create_competition(
    State(state): State<AppState>,
    Json(mut competition): Json<Competition>,
) -> Result<Json<ApiResponse<Competition>>, StatusCode> {
    let collection = get_competition_collection(&state);
    
    // Set ID to None so MongoDB generates a new one
    competition.id = None;
    
    match collection
        .insert_one(competition.clone(), None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert competition: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        mongodb::results::InsertOneResult { inserted_id, .. } => {
            // Set the generated ID in the response
            let mut competition_with_id = competition;
            if let Some(id) = inserted_id.as_object_id() {
                competition_with_id.id = Some(id);
            }
            
            Ok(Json(ApiResponse {
                success: true,
                data: Some(competition_with_id),
                message: Some("Competition created successfully".to_string()),
            }))
        }
    }
}

/// Update an existing competition by ID
pub async fn update_competition(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(competition): Json<Competition>,
) -> Result<Json<ApiResponse<Competition>>, StatusCode> {
    let collection = get_competition_collection(&state);
    
    // Validate and convert string ID to ObjectId
    let object_id = ObjectId::parse_str(&id)
        .map_err(|e| {
            tracing::error!("Invalid ObjectId: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    // Prepare update document - exclude the ID from update
    let mut update_doc = mongodb::bson::to_document(&competition)
        .map_err(|e| {
            tracing::error!("Error converting competition to document: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    update_doc.remove("_id"); // Remove the ID field from update
    
    match collection
        .update_one(
            doc! { "_id": object_id },
            doc! { "$set": update_doc },
            None,
        )
        .await
        .map_err(|e| {
            tracing::error!("Error updating competition: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        mongodb::results::UpdateResult { modified_count: 0, .. } => Err(StatusCode::NOT_FOUND),
        _ => {
            // Return the updated competition
            match collection
                .find_one(doc! { "_id": object_id }, None)
                .await
                .map_err(|e| {
                    tracing::error!("Error finding updated competition: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
            {
                Some(updated_competition) => Ok(Json(ApiResponse {
                    success: true,
                    data: Some(updated_competition),
                    message: Some("Competition updated successfully".to_string()),
                })),
                None => Err(StatusCode::NOT_FOUND),
            }
        }
    }
}

/// Delete a competition by ID
pub async fn delete_competition(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let collection = get_competition_collection(&state);
    
    // Validate and convert string ID to ObjectId
    let object_id = ObjectId::parse_str(&id)
        .map_err(|e| {
            tracing::error!("Invalid ObjectId: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    match collection
        .delete_one(doc! { "_id": object_id }, None)
        .await
        .map_err(|e| {
            tracing::error!("Error deleting competition: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        mongodb::results::DeleteResult { deleted_count: 0, .. } => Err(StatusCode::NOT_FOUND),
        _ => Ok(Json(ApiResponse {
            success: true,
            data: Some(id),
            message: Some("Competition deleted successfully".to_string()),
        })),
    }
}

/// Create the router for competition routes under /competitions path
pub fn create_competition_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_competitions))
        .route("/:id", get(get_competition_by_id))
        .route("/", post(create_competition))
        .route("/:id", put(update_competition))
        .route("/:id", delete(delete_competition))
}