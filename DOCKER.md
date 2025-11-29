# Docker Guide

## Building and Publishing Images

### Using GitHub Actions (Recommended)

The repository includes a GitHub Actions workflow for building and publishing Docker images to GitHub Container Registry.

**To trigger a build:**

1. Go to the [Actions tab](../../actions/workflows/docker-publish.yml)
2. Click "Run workflow"
3. Configure options:
   - **Branch**: Select the branch to build from (usually `main`)
   - **Custom tag** (optional): Add a custom tag like `v1.0.0` or `beta`
   - **Build platforms**: Choose between:
     - `linux/amd64` - x86_64 only (faster, ~3 min)
     - `linux/amd64,linux/arm64` - Multi-platform (slower, ~10-15 min)
4. Click "Run workflow"

The image will be published to: `ghcr.io/philiphsu609/mango-rust`

**Automatic tags created:**
- `latest` - Latest build from main branch
- `main-<sha>` - Build from specific commit (e.g., `main-abc1234`)
- Custom tag if specified

### Building Locally

Build the Docker image locally:

```bash
docker build -t mango-rust .
```

For multi-platform builds:

```bash
docker buildx build --platform linux/amd64,linux/arm64 -t mango-rust .
```

## Running the Container

### Quick Start

```bash
docker run -d \
  --name mango \
  -p 9000:9000 \
  -v /path/to/manga:/root/mango/library:ro \
  ghcr.io/philiphsu609/mango-rust:latest
```

### With Custom Configuration

```bash
docker run -d \
  --name mango \
  -p 9000:9000 \
  -v /path/to/manga:/root/mango/library:ro \
  -v /path/to/data:/root/mango \
  -e MANGO_HOST=0.0.0.0 \
  -e MANGO_PORT=9000 \
  -e MANGO_LOG_LEVEL=info \
  ghcr.io/philiphsu609/mango-rust:latest
```

### Using Docker Compose

Create `docker-compose.yml`:

```yaml
services:
  mango:
    image: ghcr.io/philiphsu609/mango-rust:latest
    container_name: mango
    ports:
      - "9000:9000"
    volumes:
      - /path/to/manga:/root/mango/library:ro
      - mango-data:/root/mango
    environment:
      - MANGO_HOST=0.0.0.0
      - MANGO_PORT=9000
      - MANGO_LOG_LEVEL=info
    restart: unless-stopped

volumes:
  mango-data:
```

Run with:

```bash
docker-compose up -d
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MANGO_HOST` | `0.0.0.0` | Server host address |
| `MANGO_PORT` | `9000` | Server port |
| `MANGO_DB_PATH` | `/root/mango/mango.db` | SQLite database path |
| `MANGO_LIBRARY_PATH` | `/root/mango/library` | Manga library directory |
| `MANGO_LOG_LEVEL` | `info` | Log level (debug, info, warn, error) |

## Volumes

- `/root/mango/library` - Manga library (mount as read-only `:ro` recommended)
- `/root/mango` - Data directory (database, cache, config)
- `/app/templates` - Template files (built into image)
- `/app/static` - Static assets (built into image)

## First Run

On first startup, the container will:
1. Create the database at `/root/mango/mango.db`
2. Generate an admin user with random password (shown in logs)
3. Scan the library directory
4. Start the server on port 9000

**To get the admin password:**

```bash
docker logs mango | grep Password
```

Output will show:
```
Username: admin
Password: <random-password>
```

**Change the password immediately** after first login at:
`http://localhost:9000/change-password`

## Pulling Images from GHCR

The images are public and can be pulled without authentication:

```bash
docker pull ghcr.io/philiphsu609/mango-rust:latest
```

For a specific tag:

```bash
docker pull ghcr.io/philiphsu609/mango-rust:v1.0.0
```

## Image Details

- **Base Image**: Alpine Linux (minimal footprint)
- **Size**: ~33MB (static musl binary)
- **Architecture**: linux/amd64 (arm64 available on request)
- **Rust Version**: 1.91
- **Features**: Static linking, offline SQLx, optimized release build

## Troubleshooting

### Check container logs

```bash
docker logs mango
```

### Check if container is running

```bash
docker ps | grep mango
```

### Access container shell

```bash
docker exec -it mango sh
```

### Rebuild without cache

```bash
docker build --no-cache -t mango-rust .
```

### Permission issues

If you get permission errors, ensure the library directory is readable:

```bash
chmod -R 755 /path/to/manga
```

## Development

For local development with live reload, use cargo instead:

```bash
cargo run --release
```

See main README for development setup instructions.
