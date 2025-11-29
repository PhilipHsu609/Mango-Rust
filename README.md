# Mango-Rust

A self-hosted manga/comic reader written in Rust. Modern reimplementation of [Mango](https://github.com/getmango/Mango) by [hkalexling](https://github.com/hkalexling).

## Features

- 📚 Multi-user library with authentication
- 📖 Web reader (paged/continuous modes)
- 💾 Progress tracking and resume reading
- 🏷️ Tags and collections
- 🌓 Dark/light theme with system detection
- 📱 Mobile-responsive UI
- 🖼️ Thumbnail generation and caching
- 👥 User management (admin)
- 🔍 Search and sorting
- 📦 ZIP/CBZ archive support
- 📡 OPDS catalog for e-reader apps
- ⚡ Two-tier caching system for performance

## Quick Start

```bash
# Clone and build
git clone https://github.com/yourusername/mango-rust.git
cd mango-rust
cargo build --release

# Run (creates config and admin user on first run)
./target/release/mango-rust
```

Access at `http://localhost:9000`. Admin credentials shown in logs on first run.

## Docker Deployment

Mango-Rust is compatible with existing Mango databases and can be deployed via Docker:

```bash
# Using docker-compose (recommended)
docker-compose up -d

# Or build and run manually
docker build -t mango-rust .
docker run -d \
  -p 9000:9000 \
  -v ~/mango:/root/mango \
  -v ~/.config/mango:/root/.config/mango \
  --name mango-rust \
  mango-rust
```

**Environment Variables:**

```bash
# Copy example env file and customize
cp env.example .env
nano .env
```

```env
# Main library directory (manga files)
MAIN_DIRECTORY_PATH=./mango-data

# Config directory
CONFIG_DIRECTORY_PATH=./config

# Port (default: 9000)
PORT=9000
```

**Volume Mounts:**
- `/root/mango` - Library directory (manga files and database)
- `/root/.config/mango` - Configuration files

### Migrating from Original Mango

Mango-Rust is a **drop-in replacement** for the original Crystal-based Mango:

1. **Stop original Mango**: `docker-compose down`
2. **Backup your data**: `cp -r ~/mango ~/mango-backup`
3. **Update docker-compose.yml**: Change image to `mango-rust`
4. **Start Mango-Rust**: `docker-compose up -d`

Your existing database, reading progress, tags, and thumbnails will work without modification. The schema is 100% compatible.

## Configuration

Config file: `~/.config/mango/config.yml`

```yaml
host: 0.0.0.0
port: 9000
library_path: ~/mango/library
db_path: ~/mango/mango.db
log_level: info
```

Environment variables: `MANGO_HOST`, `MANGO_PORT`, `MANGO_LIBRARY_PATH`, `MANGO_DB_PATH`, `MANGO_LOG_LEVEL`

## Performance & Caching

Mango-Rust implements a two-tier caching system for optimal performance with large libraries:

**Library Cache File** - Persistent cache eliminates slow filesystem scans on startup
**LRU Cache** - In-memory cache for sorted lists with automatic eviction

### Cache Configuration

```yaml
# Library cache file location
library_cache_path: ~/mango/mango_cache.bin

# Enable/disable caching (default: true)
cache_enabled: true

# LRU cache size in megabytes (default: 100)
cache_size_mbs: 100

# Enable detailed cache logging (default: false)
cache_log_enabled: false
```

### Cache Debug Page

Access `/debug/cache` (admin only) to:
- View cache statistics (hit rate, memory usage, evictions)
- Monitor library cache file status
- Manually save/load library cache
- Clear cache or invalidate specific entries

**Performance Impact:**
- Startup: 5-10s (cold) → 100ms (cached) for 1000 titles
- Sorting: 50-200ms (first time) → 1-5ms (cached)

See [docs/CACHING.md](docs/CACHING.md) for detailed documentation.

## OPDS Catalog

Mango-Rust provides an OPDS catalog for accessing your library from e-reader apps like Chunky Reader, KyBook, or Panels.

**Endpoints:**
- Main catalog: `http://localhost:9000/opds`
- Title details: `http://localhost:9000/opds/book/{title_id}`

**Authentication:**
OPDS endpoints require HTTP Basic Authentication. Use your Mango username and password:

```bash
# Test with curl
curl -u username:password http://localhost:9000/opds
```

**E-Reader Setup:**
Most OPDS-compatible apps allow adding custom catalogs. Use:
- **URL**: `http://your-server:9000/opds`
- **Authentication**: Basic Auth with your credentials

The catalog provides:
- Browse all titles in your library
- View chapters/volumes for each title
- Direct download links for reading offline
- Cover thumbnails

## Development

```bash
# Backend (hot reload)
cargo watch -x run

# Frontend CSS (LESS compilation)
npm install -g less
./watch-css.sh  # development mode
./build-css.sh  # production build

# Testing
cargo test
cargo clippy
cargo fmt
```

### File Structure

```
src/
  ├── routes/        # HTTP handlers
  ├── storage.rs     # Database layer
  ├── auth.rs        # Authentication
  └── server.rs      # Axum setup
static/src/
  ├── css/           # LESS sources
  │   ├── _variables.less
  │   ├── _dark-theme.less
  │   ├── _light-theme.less
  │   └── pages/
  └── js/
      └── core.js    # Theme management
migrations/          # SQLx migrations
templates/           # Askama templates
```

## Tech Stack

- **Backend**: Axum, Tokio, SQLx (SQLite), bcrypt, Askama
- **Frontend**: Alpine.js, UIKit, LESS
- **Storage**: ZIP archives (CBZ)

## Status

**Production Ready: v1.0 Release Candidate**

**Completed:**
- ✅ Multi-user authentication with sessions
- ✅ Library scanning and indexing
- ✅ Web reader (paged/continuous modes)
- ✅ Progress tracking per user
- ✅ Tags system with autocomplete
- ✅ User management (admin)
- ✅ Dark/light theme with auto-detection
- ✅ Mobile-responsive UI
- ✅ Home page with Continue/Start/Recently Added sections
- ✅ LESS build system with organized CSS architecture
- ✅ OPDS catalog support (HTTP Basic Auth)
- ✅ Two-tier caching system (library file + LRU cache)
- ✅ Docker deployment with multi-stage build
- ✅ Database schema 100% compatible with original Mango

**Remaining for v1.0:**
- 🚧 RAR/CBR archive format

**Future (v2.0+):**
- Plugin system
- Download queue
- Custom display names

## Credits

Based on [Mango](https://github.com/getmango/Mango) by **hkalexling**. Both projects are MIT licensed.

## License

MIT License. See [LICENSE](LICENSE) for details.
