# Stage 1: Build Frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend

# Copy dependency files
COPY frontend/package.json frontend/package-lock.json ./
# Install dependencies
RUN npm ci

# Copy source code
COPY frontend ./
# Build frontend
RUN npm run build

# Stage 2: Build Backend
FROM rust:bookworm AS backend-builder
WORKDIR /app/backend

# Copy manifests
COPY backend/Cargo.toml backend/Cargo.lock ./

# Create dummy source to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release

# Remove dummy source and build artifacts for the app itself
RUN rm -rf src
RUN rm -f target/release/deps/ting_reader*

# Copy actual source
COPY backend/src ./src

# Build for release
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Copy backend binary
COPY --from=backend-builder /app/backend/target/release/ting_reader /app/ting-reader

# Copy frontend static files
COPY --from=frontend-builder /app/frontend/dist /app/static

# Create necessary directories
RUN mkdir -p /app/data /app/plugins /app/temp /app/storage

# Set environment variables
ENV RUST_LOG=info
ENV STATIC_DIR=/app/static
ENV DATA_DIR=/app/data
ENV TEMP_DIR=/app/temp
ENV STORAGE_DIR=/app/storage
ENV TING_CONFIG_PATH=/app/config.toml
ENV TING_HOST=0.0.0.0
ENV TING_PORT=3000

# Expose port
EXPOSE 3000

# Start command
CMD ["./ting-reader"]
