# Mango-Rust

A self-hosted manga server in Rust, based on Mango.

## Run with Docker

```sh
docker run -d --name mango-rust -p 9000:9000 \
  -v "$HOME/manga:/root/mango" \
  -v "$HOME/.config/mango:/root/.config/mango" \
  ghcr.io/philiphsu609/mango-rust:latest
```

Open <http://localhost:9000>. On a fresh database, the server creates an `admin` account and prints its random password in `docker logs mango-rust`.

## Features

- Multi-user reading, progress tracking, search, sorting, and tags
- OPDS catalog and ZIP/CBZ, RAR/CBR, and 7z/CB7 archives
- Admin tools for users, library scans, missing items, and thumbnails

Plugin, subscription, and MangaDex queue endpoints are not implemented.

## Mango data and configuration

Mount your existing database and library at the paths configured by `DB_PATH` and `LIBRARY_PATH`. Rust applies database migrations and rebuilds incompatible cache files.

Optional YAML configuration: `~/.config/mango/config.yml`. Override its path with `CONFIG_PATH` or `-c PATH`; uppercase environment variables override YAML values. The default library path is `~/mango/library`.

## License

MIT. Based on [Mango](https://github.com/getmango/Mango) by hkalexling.
