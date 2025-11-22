# Mango-Rust

A self-hosted manga reader written in Rust, porting the original [Mango](https://github.com/getmango/Mango) project from Crystal.

## About

Mango-Rust is a modern reimplementation of the excellent Mango manga server, originally created by [hkalexling](https://github.com/hkalexling). This port aims to bring Mango's functionality to Rust while maintaining compatibility with the original's design philosophy and user experience.

**Credit**: This project is based on [Mango](https://github.com/getmango/Mango) by hkalexling and contributors. The original project laid the groundwork for this Rust implementation.

## Current Status

🚀 **Active Development** - Core functionality complete, enhancement features in progress.

### Completed Features

- ✅ User authentication and session management
- ✅ Library scanning and indexing (ZIP/CBZ support)
- ✅ Web-based manga reader (paged and continuous modes)
- ✅ Reading progress tracking and resume functionality
- ✅ Thumbnail generation and caching
- ✅ Search and sorting

### In Progress

- 🚧 Admin panel
- 🚧 Collections and tags
- 🚧 OPDS support

### Planned

- ⏳ Plugin system
- ⏳ Download queue
- ⏳ RAR/CBR archive support
- ⏳ Theme customization

## Prerequisites

- Rust 1.91.0 or later
- SQLite 3
- Node.js and npm (for frontend development with LESS compiler)

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/mango-rust.git
cd mango-rust

# Build the project
cargo build --release

# Run the server
cargo run --release
```

On first run, Mango will:
1. Create config file at `~/.config/mango/config.yml`
2. Initialize the database at `~/mango/mango.db`
3. Create an admin user with a random password (shown in logs)

## Configuration

Configuration file location: `~/.config/mango/config.yml`

```yaml
host: 0.0.0.0
port: 9000
base_url: /
library_path: ~/mango/library
db_path: ~/mango/mango.db
log_level: info
scan_interval_minutes: 5
```

### Environment Variables

You can override configuration with environment variables:

- `MANGO_HOST` - Server host (default: 0.0.0.0)
- `MANGO_PORT` - Server port (default: 9000)
- `MANGO_BASE_URL` - Base URL path (default: /)
- `MANGO_LIBRARY_PATH` - Manga library directory
- `MANGO_DB_PATH` - Database file path
- `MANGO_LOG_LEVEL` - Logging level (trace/debug/info/warn/error)

## Usage

1. Start the server:
   ```bash
   cargo run
   ```

2. Open your browser to `http://localhost:9000`

3. Login with the admin credentials shown in the server logs on first run

4. Change the admin password immediately after first login

## Development

```bash
# Run in development mode with hot reload
cargo watch -x run

# Run tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

### Frontend Development

Mango-Rust uses LESS for CSS preprocessing, providing variables, nesting, and modular organization.

**Prerequisites:**
- Node.js and npm (for LESS compiler)

**Setup:**
```bash
# Install LESS compiler globally
npm install -g less

# Or use npx to run without global install
npx less --version
```

**Development Workflow:**

```bash
# Watch mode - auto-recompile CSS on file changes
./watch-css.sh

# In another terminal, run the server
cargo watch -x run
```

**Production Build:**
```bash
# Compile and minify CSS
./build-css.sh

# Build the server
cargo build --release
```

**Frontend Directory Structure:**
```
static/
  src/                # Source files (edit these)
    css/
      _variables.less   # Design tokens (colors, spacing, breakpoints)
      base.less         # Global styles and layout
      _dark-theme.less  # Dark mode styles
      components/       # Reusable UI components
        _nav.less
      pages/            # Page-specific styles
        _reader.less
        _library.less
        _book.less
        _home.less
      main.less         # Entry point - imports all LESS files
    js/
      core.js           # Shared utilities (theme, localStorage)
      pages/            # Page-specific JavaScript
        reader.js
        library.js
        book.js
  dist/               # Compiled output (gitignored, auto-generated)
    css/
      main.css        # Compiled CSS bundle
      main.css.map    # Source map for debugging
```

**Key Files:**
- `static/src/css/main.less` - Main LESS entry point, imports all stylesheets
- `build-css.sh` - Production CSS build with compression
- `watch-css.sh` - Development mode with auto-compilation
- `static/src/js/core.js` - Theme management and shared utilities

**CSS Architecture:**
- All styles compiled from LESS sources into single `main.css` bundle
- Dark theme uses `body.uk-light` selector wrapper
- Variables defined in `_variables.less` for consistent design tokens
- Page-specific styles in `pages/` directory
- Component styles in `components/` directory

**Performance:**
- CSS bundle: <50KB gzipped
- Minified and compressed in production
- Source maps for debugging

## Project Structure

```
mango-rust/
├── src/
│   ├── auth.rs          # Authentication middleware
│   ├── config.rs        # Configuration loading
│   ├── lib.rs           # Library root
│   ├── main.rs          # Entry point
│   ├── server.rs        # Web server setup
│   ├── storage.rs       # Database layer
│   └── routes/          # HTTP route handlers
├── migrations/          # Database migrations
├── templates/           # HTML templates
├── Cargo.toml          # Dependencies
└── README.md
```

## Technology Stack

- **Web Framework**: [Axum](https://github.com/tokio-rs/axum) 0.7
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Database**: [SQLx](https://github.com/launchbadge/sqlx) with SQLite
- **Sessions**: [tower-sessions](https://github.com/maxcountryman/tower-sessions)
- **Authentication**: bcrypt password hashing
- **Templates**: [Askama](https://github.com/djc/askama)
- **Configuration**: YAML with [serde_yaml](https://github.com/dtolnay/serde-yaml)
- **Image Processing**: [image](https://github.com/image-rs/image) crate with JPEG/PNG/WebP support
- **Archive Handling**: [zip](https://github.com/zip-rs/zip) crate for CBZ files

## Comparison with Original Mango

| Feature | Original (Crystal) | Mango-Rust | Status |
|---------|-------------------|------------|---------|
| User Authentication | ✅ | ✅ | Complete |
| Library Scanning | ✅ | ✅ | Complete |
| ZIP/CBZ Support | ✅ | ✅ | Complete |
| Web Reader | ✅ | ✅ | Complete |
| Progress Tracking | ✅ | ✅ | Complete |
| Thumbnails | ✅ | ✅ | Complete |
| Search & Sorting | ✅ | ✅ | Complete |
| OPDS Support | ✅ | 🚧 | In Progress |
| Admin Panel | ✅ | 🚧 | In Progress |
| Tags/Collections | ✅ | ⏳ | Planned |
| RAR/CBR Support | ✅ | ⏳ | Planned |
| Plugins | ✅ | ⏳ | Future |

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

This is a port of the original [Mango](https://github.com/getmango/Mango) project, which is also MIT licensed. Both the original copyright (Alex Ling) and the Rust port copyright are preserved in the LICENSE file.

## Acknowledgments

- **[hkalexling](https://github.com/hkalexling)** - Creator of the original Mango project
- **Original Mango Contributors** - For building the foundation this project is based on
- **Rust Community** - For the excellent ecosystem of libraries

## Links

- Original Mango: https://github.com/getmango/Mango
- Report Issues: https://github.com/yourusername/mango-rust/issues
- Documentation: Coming soon

---

**Note**: This is a work in progress. Features are being added incrementally following the original Mango's development approach.
