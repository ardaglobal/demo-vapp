# Use the official Rust image as the base
FROM rust:1.88.0 as builder

# Set the working directory
WORKDIR /app

# Copy the entire workspace
COPY . .

# Build the server binary
WORKDIR /app/db
RUN cargo build --release --bin server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 appuser

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/server /app/server

# Change ownership to app user
RUN chown -R appuser:appuser /app
USER appuser

# Expose port 8080
EXPOSE 8080

# Run the server
CMD ["./server", "--host", "0.0.0.0", "--port", "8080"]
