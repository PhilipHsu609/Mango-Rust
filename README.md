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

**Remaining for v1.0:**
- 🚧 OPDS catalog support
- 🚧 RAR/CBR archive format

**Future (v2.0+):**
- Plugin system
- Download queue
- Custom display names

## Credits

Based on [Mango](https://github.com/getmango/Mango) by **hkalexling**. Both projects are MIT licensed.

## License

MIT License. See [LICENSE](LICENSE) for details.
