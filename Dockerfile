# Multi-stage build for the Competition Scraper API

# Build stage
FROM rust:1.88 as builder

WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to allow cargo to download dependencies
RUN mkdir src
RUN echo "fn main() { println!(\"Dummy build\"); }" > src/main.rs

# Download and cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# Copy source code
COPY src ./src

# Build the application
RUN touch src/main.rs  # Force rebuild
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install certificates for HTTPS requests (rustls uses system certificates)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/comp /usr/local/bin/comp

# Create a non-root user for security
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Change ownership of the binary
RUN chown appuser:appuser /usr/local/bin/comp

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the application
CMD ["comp"]
