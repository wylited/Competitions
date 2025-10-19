FROM rust:1.88 as builder

WORKDIR /app

# Copy source code
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

# Install MongoDB client tools for debugging
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/comp /usr/local/bin/comp

# Create a non-root user
RUN useradd -m -u 1000 appuser
USER appuser

# Expose port 3000
EXPOSE 3000

# Command to run the application
CMD ["comp"]
