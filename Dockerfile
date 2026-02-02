# ===== Stage 1: Build static binary =====
FROM rust:1.91-alpine AS builder

# Install musl-dev and build tools for static linking
RUN apk add --no-cache musl-dev sqlite-dev sqlite-static nodejs npm

WORKDIR /build

# Copy dependency manifests first (layer caching optimization)
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations

# Copy source code
COPY src ./src
COPY templates ./templates
COPY static ./static

# Copy sqlx offline data for compile-time query verification
COPY .sqlx ./.sqlx

# Copy package files and build CSS
COPY package.json package-lock.json ./
RUN npm ci && npm run build

# Set environment for static linking and offline sqlx
ENV RUSTFLAGS='-C target-feature=+crt-static'
ENV SQLX_OFFLINE=true

# Build static binary
RUN cargo build --release --target x86_64-unknown-linux-musl

# ===== Stage 2: Runtime image =====
FROM alpine:latest

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/mango-rust /usr/local/bin/mango-rust

# Copy static assets and templates (needed at runtime)
COPY --from=builder /build/templates /app/templates
COPY --from=builder /build/static /app/static

# Create config and data directories
RUN mkdir -p /root/.config/mango /root/mango/library

# Expose port
EXPOSE 9000

# Environment variables (can be overridden)
ENV MANGO_HOST=0.0.0.0
ENV MANGO_PORT=9000
ENV MANGO_DB_PATH=/root/mango/mango.db
ENV MANGO_LIBRARY_PATH=/root/mango/library
ENV MANGO_LOG_LEVEL=info

CMD ["/usr/local/bin/mango-rust"]
