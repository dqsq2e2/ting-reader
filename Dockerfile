# Stage 1: Build Frontend
FROM node:20-slim AS frontend-builder
WORKDIR /app/frontend
COPY ting-reader-frontend/package*.json ./
# Use npm ci for faster and more reliable builds
RUN npm ci
COPY ting-reader-frontend/ ./
RUN npm run build

# Stage 2: Runtime
FROM node:20-slim
WORKDIR /app

# Install build dependencies for better-sqlite3
RUN apt-get update && apt-get install -y \
    python3 \
    make \
    g++ \
    && rm -rf /var/lib/apt/lists/*

COPY ting-reader-backend/package*.json ./
# Use npm ci and omit devDependencies
RUN npm ci --omit=dev

# Remove build dependencies to keep image small
RUN apt-get purge -y python3 make g++ && apt-get autoremove -y

COPY ting-reader-backend/ ./
# Copy built frontend from stage 1
COPY --from=frontend-builder /app/frontend/dist ./public

# Create storage, cache and data directories
RUN mkdir -p storage cache data && chmod 777 storage cache data

# Environment variables
ENV PORT=3000
ENV NODE_ENV=production
ENV DB_PATH=/app/data/ting-reader.db

EXPOSE 3000
CMD ["node", "index.js"]
