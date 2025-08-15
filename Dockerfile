# Use the official Rust image as the base
FROM rust:1.88.0 as builder

# Install SP1 toolchain for program compilation
RUN curl -L https://sp1.succinct.xyz | bash
ENV PATH="/root/.sp1/bin:$PATH"
RUN /root/.sp1/bin/sp1up

# Set the working directory
WORKDIR /app

# Copy the entire workspace
COPY . .

# Build the SP1 program first (creates the ELF file)
WORKDIR /app/program
RUN cargo prove build --output-directory ../build

# Build the server binary (which may reference the ELF through build.rs)
WORKDIR /app/api
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
