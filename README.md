# Competition Scraper API

A Rust-based web API for scraping and managing competition announcements from various sources including HKU, HKUST, and CTFTime.

## Features

- **Modular Scraper Architecture**: Easily add new scrapers for different sources
- **Fuzzy Deduplication**: Prevents duplicate competitions from different sources
- **RESTful API**: Standard HTTP endpoints for managing competitions
- **MongoDB Integration**: Persistent storage for competition data
- **Multiple Sources Supported**:
  - HKU competitions
  - HKUST announcements
  - CTFTime events

## Prerequisites

- Rust (1.70+)
- MongoDB
- Git

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd comp
```

2. Install dependencies:
```bash
cargo build
```

3. Set up environment variables:
```bash
cp .env.example .env
# Edit .env to configure your MongoDB connection
```

## Configuration

Create a `.env` file with the following variables:

```env
MONGODB_URI=mongodb://localhost:27017
RUST_LOG=debug
```

## Running the Application

### Development
```bash
cargo run
```

The server will start on `http://localhost:3000`

### Production
```bash
cargo build --release
./target/release/comp
```

## API Endpoints

### Health Check
- `GET /` - Health check endpoint
- `GET /health` - Health check endpoint

### Competitions
- `GET /competitions` - Get all competitions with optional filtering
- `GET /competitions/{id}` - Get a specific competition by ID
- `POST /competitions` - Create a new competition

### Scrapers
- `GET /scrapers` - List all available scrapers
- `POST /scrapers/run` - Run all scrapers
- `POST /scrapers/{name}` - Run a specific scraper

## Scraper Endpoints

### List Available Scrapers
```
GET /scrapers
```
Returns a list of all registered scrapers.

### Run All Scrapers
```
POST /scrapers/run
```
Runs all registered scrapers and updates the competition database.

### Run Specific Scraper
```
POST /scrapers/{name}
```
Runs a specific scraper by name (e.g., `hku`, `hkust`, `ctftime`).

## Query Parameters for Competitions

- `page` - Page number for pagination (default: 1)
- `limit` - Number of items per page (default: 10, max: 100)
- `status` - Filter by status (e.g., "upcoming", "active", "completed")
- `host` - Filter by host organization
- `date_from` - Filter competitions from a specific date (RFC3339 format)
- `date_to` - Filter competitions up to a specific date (RFC3339 format)

## Example Usage

### Get all competitions
```bash
curl http://localhost:3000/competitions
```

### Get competitions with filters
```bash
curl "http://localhost:3000/competitions?status=upcoming&limit=20"
```

### Run HKU scraper
```bash
curl -X POST http://localhost:3000/scrapers/hku
```

### Run all scrapers
```bash
curl -X POST http://localhost:3000/scrapers/run
```

## Architecture

### Modules
- `models.rs`: Data models and serialization logic
- `competitions.rs`: Competition management endpoints
- `scrapers.rs`: Modular scraper system with fuzzy deduplication
- `main.rs`: Application entry point and routing

### Scraper System
The application features a modular scraper system with:

1. **Scraper Trait**: Defines the interface for all scrapers
2. **ScraperManager**: Manages multiple scraper instances
3. **Fuzzy Matching**: Prevents duplicate entries using text similarity
4. **Source Tracking**: Maintains multiple sources for the same competition

### Deduplication Logic
- Cleans competition names by removing source indicators ([HKU], [UST], etc.)
- Uses multiple similarity algorithms to detect potential duplicates
- Updates source fields to reflect all scrapers that found the same competition

## Environment Variables

- `MONGODB_URI`: MongoDB connection string (default: `mongodb://localhost:27017`)
- `RUST_LOG`: Log level (default: `comp=debug,tower_http=debug`)

## Development

### Adding a New Scraper

1. Create a new struct that implements the `Scraper` trait
2. Implement the `scrape` method with your scraping logic
3. Register the scraper in `ScraperManager::new()`

Example:
```rust
pub struct NewScraper;

#[async_trait]
impl Scraper for NewScraper {
    async fn scrape(&self, db: &Database) -> Result<Vec<Competition>, Box<dyn std::error::Error>> {
        // Your scraping logic here
        todo!()
    }

    fn name(&self) -> &'static str {
        "NewScraper"
    }
}
```

### Running Tests

```bash
cargo test
```

## Docker Support

A Dockerfile is included for containerized deployment:

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/comp /usr/local/bin/comp
CMD ["comp"]
```

Build and run with Docker:
```bash
docker build -t comp-api .
docker run -p 3000:3000 -e MONGODB_URI=mongodb://your-mongo-host:27017 comp-api
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

[MIT License](LICENSE)

## Support

For support, please open an issue in the GitHub repository.
