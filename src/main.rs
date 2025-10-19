use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::stream::TryStreamExt;
use mongodb::{options::ClientOptions, Client, Database};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod models;

// Application state to hold the database connection
#[derive(Clone)]
struct AppState {
    db: Database,
}

// Response for API endpoints
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

// Example handler using the Competition model
async fn create_competition(
    State(state): State<AppState>,
    Json(competition): Json<models::Competition>,
) -> Result<Json<ApiResponse<models::Competition>>, StatusCode> {
    // In a real app, you would save to the database here
    // For now, just return the competition as received
    Ok(Json(ApiResponse {
        success: true,
        data: Some(competition),
        message: Some("Competition created successfully".to_string()),
    }))
}

async fn health_handler() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("Server is running".to_string()),
        message: None,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "comp=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    // Set up MongoDB connection
    let mongo_uri = std::env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("comp_db");

    // Test the connection
    match client.list_database_names(None, None).await {
        Ok(dbs) => tracing::info!("Connected to MongoDB. Databases: {:?}", dbs),
        Err(e) => tracing::error!("Failed to connect to MongoDB: {}", e),
    }

    // Create application state
    let app_state = AppState { db };

    // Build our application with some routes
    let app = Router::new()
        .route("/", get(health_handler))
        .route("/health", get(health_handler))
        .route("/competitions", post(create_competition))
        .with_state(app_state);

    // Run the server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
