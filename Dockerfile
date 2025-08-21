# Balanced Dockerfile: Optimized but maintainable
# Single file that works for both development and production

FROM rust:1.88.0 AS builder

# Install SP1 toolchain (cached when Dockerfile doesn't change)
RUN curl -L https://sp1.succinct.xyz | bash
ENV PATH="/root/.sp1/bin:/usr/local/cargo/bin:$PATH"
RUN sp1up

WORKDIR /app

# Copy dependency manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY cli/Cargo.toml ./cli/
COPY db/Cargo.toml ./db/
COPY lib/Cargo.toml ./lib/
COPY program/Cargo.toml ./program/
COPY script/Cargo.toml ./script/

# Copy source code and SQLX query cache
COPY . .

# Build SP1 program first
WORKDIR /app/program
RUN cargo prove build --output-directory ../build

# Build server binary  
WORKDIR /app/api
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin server

# Runtime stage - simple and lightweight since verification is done locally
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1001 appuser

WORKDIR /app
COPY --from=builder /app/target/release/server ./server
RUN chown appuser:appuser /app/server

USER appuser
EXPOSE 8080
CMD ["./server", "--host", "0.0.0.0", "--port", "8080"]
