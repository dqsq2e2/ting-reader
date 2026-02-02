# Stage 1: Build Frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
COPY ting-reader-frontend/package*.json ./
RUN npm install
COPY ting-reader-frontend/ ./
# Explicitly delete any existing .env file to ensure ENV VITE_API_BASE_URL is used
RUN rm -f .env
# Set API base URL to empty for relative paths when served by same backend
ENV VITE_API_BASE_URL=""
RUN npm run build

# Stage 2: Runtime
FROM node:20-alpine
WORKDIR /app

# Install build dependencies for better-sqlite3
RUN apk add --no-cache python3 make g++ 

COPY ting-reader-backend/package*.json ./
RUN npm install --production

# Remove build dependencies to keep image small
RUN apk del python3 make g++

COPY ting-reader-backend/ ./
# Copy built frontend to backend's public folder
COPY --from=frontend-builder /app/frontend/dist ./public

# Create storage, cache and data directories
RUN mkdir -p storage cache data

# Environment variables
ENV PORT=3000
ENV NODE_ENV=production
ENV DB_PATH=/app/data/ting-reader.db

EXPOSE 3000
CMD ["node", "index.js"]
