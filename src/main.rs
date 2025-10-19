use axum::{
    response::Json,
    routing::get,
    Router,
};
use mongodb::{options::ClientOptions, Client, Database};
use serde::Serialize;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod models;
mod competitions;

// Application state to hold the database connection
#[derive(Clone)]
pub struct AppState {
    db: Database,
}

// Response for API endpoints
#[derive(Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
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
        .nest("/competitions", competitions::create_competition_router())
        .with_state(app_state);

    // Run the server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
