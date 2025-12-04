# Multi-stage Dockerfile for SpacePanda Core
# Optimized for production deployment with minimal attack surface

# Build stage
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY spacepanda-core/Cargo.toml ./spacepanda-core/

# Create dummy main.rs to cache dependencies
RUN mkdir -p spacepanda-core/src && \
    echo "fn main() {}" > spacepanda-core/src/lib.rs

# Build dependencies (cached layer)
RUN cargo build --release --manifest-path spacepanda-core/Cargo.toml && \
    rm -rf spacepanda-core/src

# Copy actual source code
COPY spacepanda-core/src ./spacepanda-core/src
COPY spacepanda-core/benches ./spacepanda-core/benches

# Build application
RUN cargo build --release --manifest-path spacepanda-core/Cargo.toml

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash spacepanda

# Create app directory and set ownership
WORKDIR /app
RUN chown spacepanda:spacepanda /app

# Note: spacepanda-core is a library crate, artifacts are in target/release/deps/
# In production, use this as a base for building the actual server binary

# Create necessary directories
RUN mkdir -p /app/data /app/config /app/logs && \
    chown -R spacepanda:spacepanda /app

# Switch to non-root user
USER spacepanda

# Set environment variables
ENV SPACEPANDA_LOG_LEVEL=info
ENV SPACEPANDA_LOG_JSON=true
ENV SPACEPANDA_STORE_DATA_DIR=/app/data
ENV SPACEPANDA_METRICS_ENABLED=true
ENV SPACEPANDA_METRICS_BIND_ADDRESS=0.0.0.0:9090

# Expose ports
# 8080: Main application
# 9090: Metrics endpoint
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Volume for persistent data
VOLUME ["/app/data"]

# Note: This is a library crate, so no executable to run
# In production, this would be used with spacepanda-cli or a server binary
CMD ["echo", "SpacePanda Core library - use with spacepanda-cli or server"]
