# ===== Build base shared by dependency planning and compilation =====
FROM rust:1.91-alpine AS chef

# Install musl-dev and build tools, including libarchive headers
RUN apk add --no-cache musl-dev sqlite-dev nodejs npm libarchive-dev pkgconfig
RUN cargo install cargo-chef --version 0.1.78 --locked

WORKDIR /build

# Use dynamic musl linking for runtime libarchive in both dependency and application builds.
ENV RUSTFLAGS='-C target-feature=-crt-static'

FROM chef AS planner
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# Keep dependency artifacts in this layer so the external GHA cache can restore them.
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Install frontend dependencies before application source changes.
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

# Build binary
RUN cargo build --release --target x86_64-unknown-linux-musl \
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
ENV HOST=0.0.0.0
ENV PORT=9000
ENV DB_PATH=/root/mango/mango.db
ENV LIBRARY_PATH=/root/mango/library
ENV LOG_LEVEL=info

CMD ["/usr/local/bin/mango-rust"]
