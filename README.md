# Mango-Rust

A fast, self-hosted manga server. Drop-in replacement for [Mango](https://github.com/getmango/Mango) with 100% database compatibility.

## Quick Start

```bash
docker run -d -p 9000:9000 \
  -v ~/manga:/root/mango \
  -v ~/.config/mango:/root/.config/mango \
  ghcr.io/philiphsu609/mango-rust:latest
```

Open `http://localhost:9000`. Default credentials shown in logs.

## Features

- Multi-user authentication
- Web reader (paged/continuous modes)
- Progress tracking & resume
- Continue Reading selects one Mango-compatible continuation entry per title
- Homepage Recently Added groups entries of the same title added within 24 hours
- Tags, search, sorting
- Admin page lists missing titles and entries separately and supports removing their database records
- Dark/light themes
- OPDS catalog for e-readers
- ZIP/CBZ, RAR/CBR, 7z/CB7 archives

## Migration from Mango

Just swap the Docker image. All data (database, progress, thumbnails) works as-is.
Per-entry `info.json` maps use Mango entry-title keys. Startup and scans migrate earlier Rust UUID-keyed metadata; legacy UUID-keyed `date_added` values are replaced with the entry file's creation time, while existing Mango title-keyed dates are preserved.

## API compatibility

Shared catalog, homepage, progress, tag, thumbnail, image, and login APIs follow Mango's request and response contracts, including nested titles, parent metadata, and `time_added` sorting. The Rust-only `/api/stats` and progress GET/POST routes were removed; Mango's `PUT /api/progress/:tid/:page` remains. This is not full API parity: Axum extractor failures and some internal-error details can still differ.

Known gaps: plugin and subscription endpoints and MangaDex queue endpoints are not implemented.

## Configuration

`~/.config/mango/config.yml`:

host: 0.0.0.0
port: 9000
library_path: ~/mango/library
db_path: ~/mango.db
scan_interval_minutes: 30
```

`CONFIG_PATH` selects a different YAML file; the CLI also accepts `-c PATH` or `--config=PATH`, which takes precedence over `CONFIG_PATH`. Configuration precedence is YAML file, environment, then defaults. Every configuration key is also available as an uppercase environment variable, such as `HOST`, `PORT`, `LIBRARY_PATH`, and `DB_PATH`.

Login modes use `DISABLE_LOGIN=true` with `DEFAULT_USERNAME`, or `AUTH_PROXY_HEADER_NAME` when a trusted reverse proxy supplies the username.

## OPDS

E-reader apps can connect to `http://server:9000/opds` with HTTP Basic Auth.

## License

MIT. Based on [Mango](https://github.com/getmango/Mango) by hkalexling.
