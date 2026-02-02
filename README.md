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
- Tags, search, sorting
- Dark/light themes
- OPDS catalog for e-readers
- ZIP/CBZ, RAR/CBR, 7z/CB7 archives

## Migration from Mango

Just swap the Docker image. All data (database, progress, thumbnails) works as-is.

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
