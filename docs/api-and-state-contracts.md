# Mango-Rust Migration: Contracts and TODOs

## Status

**Incomplete.** This document is the working migration register, not a completion claim. Previous parity reviews covered selected routes, state transitions, and tests; they were not an exhaustive inventory. Matching route names and passing tests do not establish full behavior parity.

## Open non-download work

1. **Build a complete source-to-port inventory.** Map every reachable behavior in `Mango/src/`—routes/views, CLI, configuration, auth middleware, storage, library/archive handling, background services, and persisted formats—to its Rust implementation or an explicit exclusion. The recent scan found gaps after earlier reviews, so no remaining area should be presumed aligned.
2. **Fix existing-user rename.** Mango's `POST /admin/user/edit/:original_username` applies the submitted username. Rust's `src/routes/admin.rs::user_edit_post_existing` passes the path username as both old and new names, ignoring the editable form field.
3. **Port or explicitly exclude Mango auth modes.** Mango supports `disable_login` with `default_username` and `auth_proxy_header_name` (`Mango/src/handlers/auth_handler.cr`). Rust deserializes these settings, but `src/auth.rs::require_auth` does not implement them.
4. **Align configuration behavior.** Mango derives environment keys for every option and documents file > environment > default precedence (`Mango/src/config.cr`). Rust handles only a subset under `MANGO_*` and applies overrides after YAML (`src/config.rs::apply_env_overrides`).
5. **Decide CLI parity.** Mango supports `admin user add/delete/update/list` and `--config` (`Mango/src/mango.cr`). Rust's CLI only updates a password and calls `Config::load(None)` (`src/main.rs`). Web user management does not replace this CLI contract.
6. **Decide the API-reference surface.** Mango serves `/api` and `/openapi.json`; Rust registers neither. This is informational documentation, not a missing API operation.

## Explicitly excluded

- Download/plugin integrations: plugin downloads and source discovery, download manager, MangaDex queue, subscriptions, and plugin updater. Keep these excluded only while the agreed scope remains “downloads.”
- Rust-only cache diagnostics and self-service password change are additions, not Mango parity gaps.

## Contract anchors to preserve

- **Persistence:** SQLite user/title/entry/tag/thumbnail state must remain compatible. The filesystem is the source for titles, entries, and pages; cache data is derived.
- **`info.json`:** Per-title state includes progress and last-read keyed by user and entry title, dates, sort preferences, display names, and cover URLs. Do not substitute IDs for those keys.
- **Reading and ordering:** Reader pages are 1-based; title-level progress zero means unread. Sorting uses saved sort-title overrides. Home selection, last-read ordering, and Recently Added grouping have separately tested rules.
- **HTTP contracts:** Auth failures, response status/body, ETags, upload persistence/serving, and OPDS links are observable behavior; route-name parity alone is insufficient.

## Completion rule

For each inventory row, record `matched`, `different`, `missing`, or `explicitly excluded`, with Crystal and Rust source evidence. For in-scope behavior, verify externally visible results and state transitions with a focused test or paired browser comparison. Close every gap or record an explicit maintainer-approved exclusion before calling the migration aligned. A future review must report its coverage and unexamined areas; do not infer global completion from a scoped pass.

## Completed work, briefly

Focused fixes and checks have covered stable IDs during scans, progress/sort ordering, home sections, dimensions and error responses, OPDS metadata, tags, missing-item administration, and cover persistence/serving. Regression coverage lives in `tests/api/{auth,library,progress,admin,opds}.test.ts` and Rust library unit tests. This list is not proof that unlisted Mango behavior is implemented.

## Source map

- Crystal reference: `Mango/src/routes/`, `Mango/src/handlers/`, `Mango/src/config.cr`, `Mango/src/mango.cr`, `Mango/src/storage.cr`, `Mango/src/library/`, and `Mango/migration/`.
- Rust port: `src/server.rs`, `src/auth.rs`, `src/config.rs`, `src/main.rs`, `src/routes/`, `src/storage.rs`, `src/library/`, and `migrations/`.
- UI comparison requires equivalent library data and per-title `info.json` state; the comparison apps use separate databases.
