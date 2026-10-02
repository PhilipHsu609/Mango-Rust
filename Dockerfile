# ===== Stage 1: Build binary with dynamic system libraries =====
FROM rust:1.91-alpine AS builder

# Install musl-dev and build tools, including libarchive headers
RUN apk add --no-cache musl-dev sqlite-dev nodejs npm libarchive-dev pkgconfig

WORKDIR /build

# Install frontend dependencies before source changes to preserve the npm cache.
COPY package.json package-lock.json ./
RUN npm ci

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations

# Copy source code
COPY src ./src
COPY templates ./templates
COPY static ./static

# Copy SQLx metadata used by compile-time query checks.
COPY .sqlx ./.sqlx

# Build frontend assets
RUN npm run build

# Use dynamic musl linking for runtime libarchive
ENV RUSTFLAGS='-C target-feature=-crt-static'

# Build binary
RUN --mount=type=cache,id=mango-rust-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mango-rust-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=mango-rust-target-x86_64-unknown-linux-musl,target=/build/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl \
    && cp /build/target/x86_64-unknown-linux-musl/release/mango-rust /build/mango-rust

# ===== Stage 2: Runtime image =====
FROM alpine:latest

# Install runtime shared libraries used by the binary
RUN apk add --no-cache libarchive libgcc

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/mango-rust /usr/local/bin/mango-rust

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
