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

Shared library APIs use Mango's catalog and homepage response shapes. Common progress, tag, thumbnail, and image routes match Mango's success-path request and payload shapes; some error-path status/body behavior still differs. This is not full API parity.

Known gaps: Rust does not implement Mango's `POST /api/login`, plugin/subscription endpoints, or MangaDex queue endpoints. Rust also skips nested library directories, so nested-title JSON and recursive operations are not supported; the `time_added` sort method is not implemented. Rust's extra `/api/stats` and progress GET/POST routes have no Crystal counterpart.

## Configuration

`~/.config/mango/config.yml`:

```yaml
host: 0.0.0.0
port: 9000
library_path: ~/mango/library
db_path: ~/mango/mango.db
scan_interval_minutes: 30
```

Or use env vars: `MANGO_HOST`, `MANGO_PORT`, `MANGO_LIBRARY_PATH`, `MANGO_DB_PATH`

## OPDS

E-reader apps can connect to `http://server:9000/opds` with HTTP Basic Auth.

## License

MIT. Based on [Mango](https://github.com/getmango/Mango) by hkalexling.
