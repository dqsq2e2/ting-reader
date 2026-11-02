# Stage 1: Build Frontend
FROM node:24-alpine AS frontend-builder
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

# Install build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    clang \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

ARG TARGETARCH
ARG FFMPEG_BUILD_VERSION=8.1.2-2
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) \
        ffmpeg_arch="x86_64"; \
        ffmpeg_sha256="58246304b840d40f600e7a03673695061d51ee864b11b35dbaa0983aa6506bc6"; \
        ;; \
      arm64) \
        ffmpeg_arch="arm64"; \
        ffmpeg_sha256="c12dca8818065a1ad04023eecec3d2d44542b9c1095f13eb9b028fd224ac3d28"; \
        ;; \
      *) echo "Unsupported Docker architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    ffmpeg_asset="ffmpeg-8.1.2-audio-encode-${ffmpeg_arch}-linux-gnu.tar.gz"; \
    ffmpeg_root="ffmpeg-8.1.2-audio-encode-${ffmpeg_arch}-linux-gnu"; \
    mkdir -p /app/bundled-bin; \
    curl --fail --location --retry 3 \
      --output /tmp/ffmpeg.tar.gz \
      "https://github.com/dqsq2e2/ffmpeg-build/releases/download/v${FFMPEG_BUILD_VERSION}/${ffmpeg_asset}"; \
    echo "${ffmpeg_sha256}  /tmp/ffmpeg.tar.gz" | sha256sum --check --status; \
    tar -xzf /tmp/ffmpeg.tar.gz --strip-components=2 \
      -C /app/bundled-bin \
      "${ffmpeg_root}/bin/ffmpeg" "${ffmpeg_root}/bin/ffprobe"; \
    chmod 755 /app/bundled-bin/ffmpeg /app/bundled-bin/ffprobe; \
    /app/bundled-bin/ffmpeg -hide_banner -version; \
    /app/bundled-bin/ffprobe -hide_banner -version; \
    rm -f /tmp/ffmpeg.tar.gz

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
COPY --from=backend-builder /app/bundled-bin/ffmpeg /app/bin/ffmpeg
COPY --from=backend-builder /app/bundled-bin/ffprobe /app/bin/ffprobe

# Copy default configuration
COPY backend/config.toml /app/config.toml

# Copy frontend static files
COPY --from=frontend-builder /app/frontend/dist /app/static

# Create necessary directories
RUN mkdir -p /app/data /app/plugins /app/temp /app/storage /app/preinstalled-plugins

ARG TING_PLUGIN_STORE_VERSION=2.0.4
ADD https://github.com/dqsq2e2/ting-reader-plugin-store/releases/download/v${TING_PLUGIN_STORE_VERSION}/ting-reader-plugin-store-${TING_PLUGIN_STORE_VERSION}.tr /app/preinstalled-plugins/ting-reader-plugin-store.tr

# Set environment variables
ENV RUST_LOG=info
ENV STATIC_DIR=/app/static
ENV DATA_DIR=/app/data
ENV TEMP_DIR=/app/temp
ENV STORAGE_DIR=/app/storage
ENV TING_PLUGINS__PREINSTALLED_DIR=/app/preinstalled-plugins
ENV TING_CONFIG_PATH=/app/config.toml
ENV TING_SERVER__HOST=0.0.0.0
ENV TING_SERVER__PORT=3000

# Expose port
EXPOSE 3000

# Start command
CMD ["./ting-reader"]
