use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use futures_util::{StreamExt, TryStreamExt};
use mongodb::{Collection, bson::doc};
use scraper::{Html, Selector};
use serde_json;
use std::collections::HashMap;

use crate::{models::Competition, AppState, ApiResponse};

/// Trait that defines the interface for all scrapers
#[async_trait::async_trait]
pub trait Scraper: Send + Sync {
    async fn scrape(&self, db: &mongodb::Database) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>>;
    fn name(&self) -> &'static str;
}

/// HKU Scraper implementation
pub struct HkuScraper;

#[async_trait::async_trait]
impl Scraper for HkuScraper {
    async fn scrape(&self, db: &mongodb::Database) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://ug.hkubs.hku.hk/competition";
        
        // Create a client that can handle SSL verification
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        
        // Fetch the page
        let response = client.get(url).send().await?;
        let body = response.text().await?;
        
        // Parse HTML and extract titles synchronously to avoid Send issues
        let titles = {
            // Parse HTML
            let document = Html::parse_document(&body);
            
            // Create selector for competition cards
            let card_selector = Selector::parse("a.card-blk__item").unwrap();
            let title_selector = Selector::parse("p.card-blk__title").unwrap();
            
            // Collect titles
            document
                .select(&card_selector)
                .filter_map(|card| {
                    card.select(&title_selector).next()
                })
                .map(|title_element| {
                    title_element.text().collect::<Vec<_>>().join(" ").trim().to_string()
                })
                .collect::<Vec<String>>()
        }; // HTML document is dropped here, so no Send issues
        
        let mut competitions = Vec::new();
        
        // Process each title
        for title in titles {
            // Create competition with HKU source
            let competition = Competition {
                id: None, // Will be set by MongoDB
                name: format!("{} [HKU]", title),
                date: chrono::Utc::now(), // Default to current time, should be parsed from actual date if available
                host: "HKU".to_string(), // Keep as HKU as requested
                source: "HKU".to_string(),
                description: None,
                signup_deadline: None,
                location: None,
                registration_link: None,
                max_participants: None,
                status: Some("upcoming".to_string()),
            };
            
            // Use fuzzy matching to check for duplicates
            if !is_duplicate_competition(db, &competition).await {
                competitions.push(competition);
            } else {
                // If it's a duplicate, update the source field to include HKU
                update_existing_competition_source(db, &competition.name, "HKU").await?;
            }
        }
        
        Ok(competitions)
    }

    fn name(&self) -> &'static str {
        "HKU"
    }
}

/// HKUST Scraper implementation
pub struct HkustScraper;

#[async_trait::async_trait]
impl Scraper for HkustScraper {
    async fn scrape(&self, db: &mongodb::Database) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://bmundergrad.hkust.edu.hk/announcement";
        
        // Create a client that can handle SSL verification differently
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)  // Equivalent to verify=False in Python
            .build()?;
        
        // Fetch the page
        let response = client.get(url).send().await?;
        let body = response.text().await?;
        
        // Keywords to filter for
        let keywords = [
            "Case", "Challenge", "Competition", "Hackathon", "Datathon"
        ];
        
        // Parse HTML and extract titles synchronously to avoid Send issues
        let titles = {
            // Parse HTML
            let document = Html::parse_document(&body);
            
            // Create selector for announcement rows
            let row_selector = Selector::parse("tr").unwrap();
            let title_selector = Selector::parse("h3").unwrap();
            
            // Collect titles that match keywords
            let mut matching_titles = Vec::new();
            
            for row in document.select(&row_selector) {
                for title_element in row.select(&title_selector) {
                    let title_text = title_element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    
                    // Check if any keyword is in the title (case insensitive)
                    if keywords.iter().any(|&keyword| {
                        title_text.to_lowercase().contains(&keyword.to_lowercase())
                    }) {
                        matching_titles.push(title_text);
                    }
                }
            }
            
            matching_titles
        }; // HTML document is dropped here, so no Send issues
        
        let mut competitions = Vec::new();
        
        // Process each matching title
        for title in titles {
            // Create competition with HKUST source
            let competition = Competition {
                id: None, // Will be set by MongoDB
                name: format!("{} [UST]", title),
                date: chrono::Utc::now(), // Default to current time, should be parsed from actual date if available
                host: "HKUST".to_string(), // Keep as HKUST as requested
                source: "HKUST".to_string(),
                description: None,
                signup_deadline: None,
                location: None,
                registration_link: None,
                max_participants: None,
                status: Some("upcoming".to_string()),
            };
            
            // Use fuzzy matching to check for duplicates
            if !is_duplicate_competition(db, &competition).await {
                competitions.push(competition);
            } else {
                // If it's a duplicate, update the source field to include HKUST
                update_existing_competition_source(db, &competition.name, "HKUST").await?;
            }
        }
        
        Ok(competitions)
    }

    fn name(&self) -> &'static str {
        "HKUST"
    }
}

/// Check if a competition already exists in the database using fuzzy matching
async fn is_duplicate_competition(db: &mongodb::Database, new_comp: &Competition) -> bool {
    let collection: Collection<Competition> = db.collection("competitions");
    
    // Get all existing competitions
    let cursor = collection.find(doc! {}, None).await.unwrap();
    let existing_competitions: Vec<Competition> = cursor.try_collect().await.unwrap();
    
    // Simple fuzzy matching by checking if the name contains similar words
    for existing in existing_competitions {
        if fuzzy_match(&new_comp.name, &existing.name) {
            return true;
        }
    }
    
    false
}

/// Improved fuzzy matching algorithm to check if two competition names are similar
fn fuzzy_match(name1: &str, name2: &str) -> bool {
    let name1_clean = clean_competition_name(name1);
    let name2_clean = clean_competition_name(name2);
    
    let name1_lower = name1_clean.to_lowercase();
    let name2_lower = name2_clean.to_lowercase();
    
    // Exact match check
    if name1_lower == name2_lower {
        return true;
    }
    
    // Check if one name contains the other
    if name1_lower.contains(&name2_lower) || name2_lower.contains(&name1_lower) {
        return true;
    }
    
    // Calculate similarity using multiple methods
    let similarity = calculate_similarity(&name1_lower, &name2_lower);
    if similarity > 0.75 {  // Higher threshold for string similarity
        return true;
    }
    
    // Calculate word overlap
    let words1: Vec<&str> = name1_lower.split_whitespace().collect();
    let words2: Vec<&str> = name2_lower.split_whitespace().collect();
    
    let mut common_words = 0;
    for word1 in &words1 {
        if word1.len() > 2 {  // Only consider words longer than 2 characters
            if words2.iter().any(|&word2| {
                word2.len() > 2 && (  // Only consider words longer than 2 characters
                    *word1 == word2 ||  // Exact match
                    word1.contains(word2) || word2.contains(word1) ||  // Partial containment
                    calculate_similarity(word1, word2) > 0.7  // High similarity
                )
            }) {
                common_words += 1;
            }
        }
    }
    
    // Check if there's significant overlap
    let max_len = words1.len().max(words2.len());
    if max_len > 0 && common_words as f32 / max_len as f32 > 0.5 {  // At least 50% overlap
        return true;
    }
    
    // Check if the ratio of common words to total unique words is high
    let all_words: std::collections::HashSet<&str> = words1.iter().chain(words2.iter()).cloned().collect();
    if !all_words.is_empty() && common_words as f32 / all_words.len() as f32 > 0.4 {
        return true;
    }
    
    false
}

/// Helper function to clean competition names by removing source indicators like [HKU], [UST]
fn clean_competition_name(name: &str) -> String {
    // Remove source indicators in brackets
    let re = regex::Regex::new(r"\s*\[.*?\]\s*$").unwrap_or_else(|_| regex::Regex::new(r"^").unwrap());
    let cleaned = re.replace_all(name, "").trim().to_string();
    
    // Remove common university indicators and normalize spaces
    let cleaned = cleaned.replace("HKU", "")
        .replace("UST", "")
        .replace("HKUST", "")
        .replace("The", "")
        .replace("the", "")
        .replace("A", "")
        .replace("a", "")
        .replace("An", "")
        .replace("an", "")
        .replace("And", "")
        .replace("and", "")
        .replace("Of", "")
        .replace("of", "")
        .replace("In", "")
        .replace("in", "")
        .replace("On", "")
        .replace("on", "")
        .replace("At", "")
        .replace("at", "")
        .replace("To", "")
        .replace("to", "")
        .replace("For", "")
        .replace("for", "")
        .replace("With", "")
        .replace("with", "")
        .replace("By", "")
        .replace("by", "")
        .replace("Up", "")
        .replace("up", "")
        .replace("Competition", "")
        .replace("competition", "")
        .replace("Case", "")
        .replace("case", "")
        .replace("Challenge", "")
        .replace("challenge", "")
        .replace("Hackathon", "")
        .replace("hackathon", "")
        .replace("Datathon", "")
        .replace("datathon", "")
        .replace("Program", "")
        .replace("program", "")
        .replace("Event", "")
        .replace("event", "")
        .replace("Session", "")
        .replace("session", "")
        .replace("Workshop", "")
        .replace("workshop", "")
        .replace("Seminar", "")
        .replace("seminar", "")
        .replace("Deadline", "")
        .replace("deadline", "")
        .replace("Register", "")
        .replace("register", "")
        .replace("Join", "")
        .replace("join", "")
        .replace("NOW", "")
        .replace("now", "")
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    
    cleaned
}

/// Simple string similarity function using a basic algorithm
fn calculate_similarity(s1: &str, s2: &str) -> f64 {
    let s1 = s1.trim().to_lowercase();
    let s2 = s2.trim().to_lowercase();
    
    if s1.is_empty() && s2.is_empty() {
        return 1.0;
    }
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }
    if s1 == s2 {
        return 1.0;
    }
    
    // Simple character-based similarity
    let common_chars = s1.chars().filter(|c| s2.contains(*c)).count();
    let total_chars = s1.len().max(s2.len());
    
    if total_chars == 0 {
        0.0
    } else {
        common_chars as f64 / total_chars as f64
    }
}

/// Update existing competition's source field to include the new scraper
async fn update_existing_competition_source(
    db: &mongodb::Database,
    name: &str,
    scraper_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let collection: Collection<Competition> = db.collection("competitions");
    
    // Find competitions with similar names using fuzzy matching
    let cursor = collection.find(doc! {}, None).await.unwrap();
    let existing_competitions: Vec<Competition> = cursor.try_collect().await.unwrap();
    
    for existing in existing_competitions {
        // Use the clean names for fuzzy matching to ignore source indicators like [HKU], [UST]
        if fuzzy_match(&name, &existing.name) {
            // Update the source field to include the new scraper
            let mut sources: Vec<&str> = existing.source.split(',').map(|s| s.trim()).collect();
            if !sources.contains(&scraper_name) {
                sources.push(scraper_name);
                let new_source = sources.join(", ");
                
                collection
                    .update_one(
                        doc! { "_id": existing.id.unwrap() },
                        doc! { "$set": { "source": new_source } },
                        None,
                    )
                    .await?;
            }
        }
    }
    
    Ok(())
}

/// CTFTime Scraper implementation
pub struct CtfTimeScraper;

#[async_trait::async_trait]
impl Scraper for CtfTimeScraper {
    async fn scrape(&self, db: &mongodb::Database) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://ctftime.org/api/v1/events/";
        
        // Create a client that can handle SSL verification
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        
        // Fetch the page - get upcoming events (next 20)
        let response = client.get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; CTFScraper/1.0)")
            .query(&[("limit", "20")]) // Get up to 20 upcoming events
            .send()
            .await?;
        
        let body = response.text().await?;
        
        // Parse JSON response from CTFTime API
        let events: Vec<serde_json::Value> = serde_json::from_str(&body)?;
        
        let mut competitions = Vec::new();
        
        for event in events {
            // Extract relevant fields from the CTFTime API response
            if let (Some(title), Some(start_time), Some(end_time), Some(url), Some(description)) = (
                event.get("title").and_then(|v| v.as_str()),
                event.get("start").and_then(|v| v.as_str()),
                event.get("finish").and_then(|v| v.as_str()),
                event.get("url").and_then(|v| v.as_str()).or(Some("")),
                event.get("description").and_then(|v| v.as_str()).or(Some(""))
            ) {
                // Parse the start time
                let start_date = chrono::DateTime::parse_from_rfc3339(start_time)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                
                // Create competition with CTFTime source
                let competition = Competition {
                    id: None, // Will be set by MongoDB
                    name: format!("{} [CTF]", title),
                    date: start_date,
                    host: "CTFTime".to_string(),
                    source: "CTFTime".to_string(),
                    description: if description.is_empty() { None } else { Some(description.to_string()) },
                    signup_deadline: None, // CTFTime API doesn't always provide registration deadline
                    location: Some("Online".to_string()), // Most CTFs are online
                    registration_link: if url.is_empty() { None } else { Some(url.to_string()) },
                    max_participants: event.get("max_team_size")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32),
                    status: Some("upcoming".to_string()),
                };
                
                // Use fuzzy matching to check for duplicates
                if !is_duplicate_competition(db, &competition).await {
                    competitions.push(competition);
                } else {
                    // If it's a duplicate, update the source field to include CTFTime
                    update_existing_competition_source(db, &competition.name, "CTFTime").await?;
                }
            }
        }
        
        Ok(competitions)
    }

    fn name(&self) -> &'static str {
        "CTFTime"
    }
}

/// ScraperManager to manage multiple scrapers
pub struct ScraperManager {
    scrapers: HashMap<String, Box<dyn Scraper>>,
}

impl ScraperManager {
    pub fn new() -> Self {
        let mut manager = ScraperManager {
            scrapers: HashMap::new(),
        };
        
        // Register default scrapers
        manager.register_scraper(Box::new(HkuScraper));
        manager.register_scraper(Box::new(HkustScraper));
        manager.register_scraper(Box::new(CtfTimeScraper));

        manager
    }
    
    pub fn register_scraper(&mut self, scraper: Box<dyn Scraper>) {
        self.scrapers.insert(scraper.name().to_lowercase(), scraper);
    }
    
    pub fn get_scraper_names(&self) -> Vec<String> {
        self.scrapers.keys().cloned().collect()
    }
    
    pub async fn run_scraper(
        &self,
        name: &str,
        db: &mongodb::Database,
    ) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(scraper) = self.scrapers.get(&name.to_lowercase()) {
            scraper.scrape(db).await
        } else {
            Err("Scraper not found".into())
        }
    }
    
    pub async fn run_all_scrapers(
        &self,
        db: &mongodb::Database,
    ) -> Result<Vec<Competition>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_competitions = Vec::new();
        
        for scraper in self.scrapers.values() {
            match scraper.scrape(db).await {
                Ok(mut competitions) => {
                    all_competitions.append(&mut competitions);
                }
                Err(e) => {
                    eprintln!("Error running scraper {}: {}", scraper.name(), e);
                }
            }
        }
        
        Ok(all_competitions)
    }
}

// Use AppState directly instead of creating a separate ScraperState
// The scraper manager will be initialized in main and passed appropriately

/// Get a new scraper manager instance (in a real app, this would be shared)
fn get_scraper_manager() -> ScraperManager {
    ScraperManager::new()
}

/// Handler to list all available scrapers
pub async fn list_scrapers(
    State(_state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let manager = get_scraper_manager();
    let scraper_names = manager.get_scraper_names();
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(scraper_names),
        message: Some("Available scrapers retrieved successfully".to_string()),
    }))
}

/// Handler to run all scrapers
pub async fn run_all_scrapers(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let manager = get_scraper_manager();
    let competitions = match manager.run_all_scrapers(&state.db).await {
        Ok(comps) => comps,
        Err(e) => {
            eprintln!("Error running all scrapers: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Save the scraped competitions to the database
    let collection: Collection<Competition> = state.db.collection("competitions");
    let competitions_count = competitions.len();
    
    for mut competition in competitions {
        // Check if the competition already exists
        let existing = collection
            .find_one(
                doc! { "name": &competition.name },
                None,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if let Some(existing_comp) = existing {
            // Update the source field to include both sources
            let mut sources: Vec<&str> = existing_comp.source.split(',').map(|s| s.trim()).collect();
            let new_sources: Vec<&str> = competition.source.split(',').map(|s| s.trim()).collect();
            
            for new_source in new_sources {
                if !sources.contains(&new_source) {
                    sources.push(new_source);
                }
            }
            
            let updated_source = sources.join(", ");
            collection
                .update_one(
                    doc! { "_id": existing_comp.id.unwrap() },
                    doc! { "$set": { "source": updated_source } },
                    None,
                )
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            // Insert new competition
            competition.id = None; // Let MongoDB generate the ID
            collection
                .insert_one(competition, None)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(format!("Successfully scraped {} competitions from {} scrapers", competitions_count, manager.get_scraper_names().len())),
        message: Some("All scrapers ran successfully".to_string()),
    }))
}

/// Handler to run a specific scraper
pub async fn run_specific_scraper(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let manager = get_scraper_manager();
    let competitions = match manager.run_scraper(&name, &state.db).await {
        Ok(comps) => comps,
        Err(_) => {
            return Err(StatusCode::NOT_FOUND);
        }
    };
    
    // Save the scraped competitions to the database
    let collection: Collection<Competition> = state.db.collection("competitions");
    let competitions_count = competitions.len();
    
    for mut competition in competitions {
        // Check if the competition already exists
        let existing = collection
            .find_one(
                doc! { "name": &competition.name },
                None,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if let Some(existing_comp) = existing {
            // Update the source field to include both sources
            let mut sources: Vec<&str> = existing_comp.source.split(',').map(|s| s.trim()).collect();
            let new_sources: Vec<&str> = competition.source.split(',').map(|s| s.trim()).collect();
            
            for new_source in new_sources {
                if !sources.contains(&new_source) {
                    sources.push(new_source);
                }
            }
            
            let updated_source = sources.join(", ");
            collection
                .update_one(
                    doc! { "_id": existing_comp.id.unwrap() },
                    doc! { "$set": { "source": updated_source } },
                    None,
                )
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            // Insert new competition
            competition.id = None; // Let MongoDB generate the ID
            collection
                .insert_one(competition, None)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(format!("Successfully scraped {} competitions from {}", competitions_count, name)),
        message: Some(format!("Scraper '{}' ran successfully", name)),
    }))
}

/// Create the router for scraper routes
pub fn create_scraper_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_scrapers))
        .route("/run", post(run_all_scrapers))
        .route("/:name", post(run_specific_scraper))
}
