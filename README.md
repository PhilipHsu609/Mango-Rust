# Mango-Rust

A self-hosted manga/comic reader written in Rust. Drop-in replacement for [Mango](https://github.com/getmango/Mango) with better performance and 100% database compatibility.

## Why Mango-Rust?

- **Compatible**: Works with existing Mango databases (drop-in replacement)
- **Modern**: Built with Rust, Axum, and Alpine.js
- **Feature-complete**: All features from original Mango plus performance improvements

## Features

- 📚 Multi-user library with authentication
- 📖 Web reader with paged/continuous modes
- 💾 Reading progress tracking and resume
- 🏷️ Tags and collections
- 🌓 Dark/light theme with system detection
- 📱 Mobile-responsive UI
- 🖼️ Automatic thumbnail generation
- 👥 User management (admin)
- 🔍 Search and sorting
- 📦 ZIP/CBZ archive support
- 📡 OPDS catalog for e-reader apps
- ⚡ Two-tier caching system for large libraries

## Quick Start (Docker)

```bash
docker run -d \
  -p 9000:9000 \
  -v ~/manga:/root/mango \
  -v ~/.config/mango:/root/.config/mango \
  --name mango-rust \
  ghcr.io/philiphsu609/mango-rust:latest
```

Access at `http://localhost:9000`. Default admin credentials shown in logs on first run.

## Migrating from Original Mango

Mango-Rust is a **drop-in replacement**. Your existing database, reading progress, tags, and thumbnails work without modification:

1. Stop original Mango: `docker stop mango`
2. Update image in docker-compose.yml to `ghcr.io/philiphsu609/mango-rust:latest`
3. Start Mango-Rust: `docker-compose up -d`

That's it! All your data is preserved.

## Configuration

Create `~/.config/mango/config.yml`:

```yaml
host: 0.0.0.0
port: 9000
library_path: ~/mango/library
db_path: ~/mango/mango.db
log_level: info
scan_interval_minutes: 30
```

Or use environment variables: `MANGO_HOST`, `MANGO_PORT`, `MANGO_LIBRARY_PATH`, `MANGO_DB_PATH`, `MANGO_LOG_LEVEL`

## OPDS Catalog

Access your library from e-reader apps (Chunky Reader, KyBook, Panels):

- **URL**: `http://your-server:9000/opds`
- **Auth**: HTTP Basic Auth with your Mango username/password

```bash
# Test with curl
curl -u username:password http://localhost:9000/opds
```

## Development

```bash
# Clone and build
git clone https://github.com/PhilipHsu609/mango-rust.git
cd mango-rust
cargo build --release

# Run tests
cargo test
npm --prefix tests test

# Build CSS
npm install -g less
./build-css.sh
```

## Tech Stack

- **Backend**: Axum, Tokio, SQLx (SQLite), bcrypt
- **Frontend**: Alpine.js, UIKit, LESS
- **Storage**: ZIP archives (CBZ)

## Status

**Production Ready** - v1.0 Release Candidate

All core features complete. Remaining for v1.0:
- RAR/CBR archive support (planned)

## Credits

Based on [Mango](https://github.com/getmango/Mango) by **hkalexling**. Both projects are MIT licensed.

## License

MIT License. See [LICENSE](LICENSE) for details.
