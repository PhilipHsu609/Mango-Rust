# Mango API and State Contracts

## Purpose and method

This is the source-derived contract map for the shared Mango Crystal and Mango-Rust surface. `Mango/` is the behavioral reference. It separates route inventory, persisted state, and observable behavior so parity changes can be justified against the state model instead of inferred from a single screen. It does not claim a live-container comparison.

Detailed parity targets: Home, Library (including Reader and OPDS), Tags, shared Admin, and the APIs those workflows use. Plugin/subscription/MangaDex integrations are listed but excluded as previously agreed. Rust-only cache diagnostics are product-specific, not parity targets.

## Route inventory

### Shared pages and reader

| Method and path | Behavior | State/side effect |
| --- | --- | --- |
| `GET /login`, `POST /login`, `GET /logout` | Login form, session creation, logout | User token in SQLite; session cookie |
| `POST /api/login` | JSON login; success includes `success`, `session_id`, `is_admin`; invalid credentials return 403 and `{success:false,error}` | User token plus signed session |
| `GET /` | Home sections: Continue Reading, Start Reading, Recently Added; first-use and empty-library states | Reads per-title `info.json` progress/date state |
| `GET /library` | Root-title cards, search and sorting | Reads/writes per-user library sort preference in library-root `info.json` |
| `GET /book/:id` | Title, nested-title and entry cards; tags; admin metadata/progress controls | Reads title metadata, tags, progress, sort preference; admin controls mutate these |
| `GET /tags`, `GET /tags/:tag` | Tag counts and tagged-title cards | Reads SQLite `tags`; tagged-title ordering and progress are user-visible |
| `GET /reader/:tid/:eid`, `GET /reader/:tid/:eid/:page` | Resume/validate page and render Reader | Reads entry pages, configured sort, and per-user progress |
| `GET /change-password` | Rust page; Mango does not expose a matching route | Rust-only authenticated password workflow |
| `GET /opds`, `GET /opds/book/:title_id` | OPDS catalog and title/entry feed | Reads library, metadata and progress; Basic Auth for clients |

### Shared library, home, reader, progress, tags, and admin APIs

| Method and path | Contract summary | Persistence/observable behavior |
| --- | --- | --- |
| `GET /api/library` | `{dir,titles,...}` catalog; `depth`, `slim`, `percentage` query flags; title/entry order follows per-user sort preference | Reads nested title/entry tree, metadata, progress, sort overrides |
| `GET /api/book/:tid` | One title tree; same `depth`, `slim`, `percentage` flags | Missing title is 404; reads title data and per-user sort preference |
| `GET /api/sort_opt?tid=...`, `PUT /api/sort_opt` | Read/update per-user sort method and direction for a title or library root | Writes `sort_by` in the relevant `info.json` |
| `GET /api/library/continue_reading` | `{success,entries,entry_percentages}` | Uses last-read entry per title, selected from each title's configured entry order and persisted entry sort-title overrides |
| `GET /api/library/start_reading` | `{success,titles}` | Shuffles a sample of unread top-level titles drawn from all unread root titles; does not choose an arbitrary nested title instead of its parent |
| `GET /api/library/recently_added` | `{success,items}`; each item contains an entry or title summary, progress percentage and grouped count | Uses `date_added`; entries in one title added within the 24-hour grouping window collapse to one title card |
| `GET /api/page/:tid/:eid/:page` | 1-based page image; image bytes, MIME, ETag; 304 when the ETag matches | Reads archive/loose-image entry; archive and directory cache headers differ |
| `GET /api/cover/:tid/:eid` | Entry cover/thumbnail bytes; ETag/304 | Reads thumbnail DB row or falls back to page 1 |
| `GET /api/dimensions/:tid/:eid` | `{success,dimensions}`; weak ETag/304 | Extracts/stores dimensions; cache headers differ for archive and loose-image entries |
| `GET /api/download/:tid/:eid` | Original entry download | Reads source archive/file; missing entry returns 404 |
| `PUT /api/progress/:tid/:page?eid=...` | Save one entry's page when `eid` is present; otherwise page `0` marks title tree unread and nonzero marks it read; success `{success:true}` | Writes per-user progress and last-read timestamp into that title's `info.json` |
| `PUT /api/bulk_progress/:action/:tid` | JSON `{ids:[entry_id...]}`; `read` or `unread` updates selected entries | Writes progress for the current user without changing `last_read`; invalid action returns a failure object; success `{success:true}` |
| `GET /api/tags`, `GET /api/tags/:tid` | List tags or tags on one title | Reads SQLite `tags` |
| `PUT/DELETE /api/admin/tags/:tid/:tag` | Add/remove one title tag | Admin-only; writes SQLite `tags` |
| `POST /api/admin/scan` | Synchronous scan result `{titles,milliseconds}` | Reconciles filesystem entries and IDs, marks unavailable rows, rebuilds active library/cache |
| `GET /api/admin/thumbnail_progress`, `POST /api/admin/generate_thumbnails` | Read/start thumbnail generation | Admin-only; generated thumbnails are stored in SQLite |
| `GET /api/admin/titles/missing`, `GET /api/admin/entries/missing` | `{success,error,titles|entries}` for unavailable records | Reads `titles`/`ids` rows marked unavailable |
| `DELETE /api/admin/titles/missing[/:id]`, `DELETE /api/admin/entries/missing[/:id]` | Delete all or one unavailable record; success `{success:true,error:null}` | Deletes database metadata only; does not delete manga files |
| `DELETE /api/admin/user/delete/:username` | Delete a user; `{success:true}` or failure object | Admin-only; deletes SQLite user row |
| `PUT /api/admin/display_name/:tid/:name?eid=...` | Set title or entry display name | Mango and Rust persist title/entry names in `info.json`; the historical Rust display-name columns remain unused |
| `PUT /api/admin/sort_title/:tid?eid=...&name=...` | Set/clear title or entry sort override | Writes `sort_title` in SQLite `titles`/`ids`; sort order uses the override |
| `POST /api/admin/upload/cover?tid=...&eid=...` (Mango `/:target`; Rust fixed `/cover`) | Multipart `file`; set title or entry cover | Mango and Rust write supported image types under configured upload directory, persist URL in `info.json`, and serve the file from `/uploads`; title-level or entry-level cover selected by `eid` |

### Route differences and explicitly excluded surfaces

- Mango-only integrations: `/download/plugins`, `/admin/downloads`, `/admin/subscriptions`, MangaDex queue WebSocket/HTTP APIs, plugin and subscription APIs, and `openapi.json`/`/api` docs. Excluded from parity work by agreement.
- Rust-only admin APIs: `GET/POST /api/admin/users`, `PATCH/DELETE /api/admin/users/:username`; Rust also retains Mango's `/api/admin/user/delete/:username` path for its user-table action.
- Rust-only cache diagnostics: `/debug/cache`, `/api/cache/clear`, `/api/cache/save-library`, `/api/cache/load-library`, `/api/cache/invalidate`. Preserve as Rust-specific functionality.
- Rust exposes `POST /api/user/change-password`; Mango's shared UI has an admin-set-password form but no corresponding self-service route.
- Mango's upload route is parameterized (`/api/admin/upload/:target`); Rust registers only `/api/admin/upload/cover`.

## State ownership and lifetime

| State | Mango Crystal | Mango-Rust | Contract significance |
| --- | --- | --- | --- |
| Users and session tokens | SQLite `users` table; bcrypt password hashes; token stored on login; logout clears token | Same shared table/fields; session middleware stores token | Database compatibility; user/admin access gates UI and APIs |
| Title/entry identity | SQLite `titles` and `ids`: ID, library-relative path, signature, unavailable flag, sort-title override | Same identity tables and path/signature purpose; historical migration 006 adds unused display-name columns; dimensions table | Scan must preserve IDs across moves/renames and surface unavailable records |
| Tags | SQLite `tags`, unique per `(id,tag)`, title foreign key | Same | Tags survive app restarts and attach to title IDs |
| Thumbnails | SQLite `thumbnails` keyed by entry ID; generated thumbnails point to uploaded files; explicit cover URLs are read from `info.json` | SQLite `thumbnails` keyed by entry ID; generated thumbnails stored here; explicit cover URLs are read from `info.json` | Mango supports title-level cover URLs and entry-level URL overrides in addition to thumbnails |
| Reading state | One `info.json` per title directory: `progress`, `last_read`, `date_added`, `sort_by`; names and cover URLs also live here | Same JSON fields are parsed/written; Rust loads progress into an in-memory `ProgressCache`; progress updates persist back to the file | Progress keys are entry titles, not IDs; custom names/covers and dates are part of the Mango file contract |
| Sort overrides | SQLite `titles.sort_title` / `ids.sort_title` | Same columns and write APIs | Sort override affects title/entry ordering and card sort metadata |
| Dimensions | Extracted on demand; conditional HTTP caching | Extracted on demand and cached in SQLite `dimensions` | Cache is derivable; stale cache must be rejected by ETag/page-count checks |
| Uploaded cover files | File under configured upload path; public URL stored in title `info.json` | Same; upload route stores title or entry URL in `info.json` and serves the file from `/uploads` | Explicit cover overrides are separate from generated SQLite thumbnails |
| Library snapshot | Compressed YAML cache at configured `library_cache_path`; Mango also has process-wide LRU cache for `info.json`, sorted results, and progress sums | Configured persistent library snapshot plus bounded LRU sort cache; active library is atomically replaced; progress cache mirrors `info.json` | Cache contents are derived, not a second source of truth; scan invalidates/rebuilds relevant caches |
| Filesystem | Directory tree is source of titles/entries/pages; scan updates relative DB paths and unavailable flags | Same intended source; scan constructs nested titles and entries, reconciles IDs, records missing rows | File changes are confirmed by scan; missing-record deletion only removes metadata |

`info.json` contract fields: `comment`, `progress[user][entry_title]`, `entry_display_name[entry_title]`, `display_name`, `cover_url`, `entry_cover_url[entry_title]`, `last_read[user][entry_title]`, `date_added[entry_title]`, and `sort_by[user] = (method,ascending)`. Rust also migrates prior Rust UUID-keyed entry records to Mango's entry-title keys during scan.

## Observable rules that parity changes must preserve

- Mango is authoritative for defaults and ordering. Natural/chapter ordering uses the saved sort title, not necessarily the raw filename; progress sorting uses progress and title-order tie breaks.
- Progress page values are 1-based at the Reader URL/API boundary. Saved page values are clamped to available pages; zero at the title-level API means unread, while a positive title-level value means read.
- Home sections use at most eight cards. Continue Reading is based on the last-read entry per title; Mango selects candidates in library order, takes eight, then sorts those by last-read time. Start Reading randomly samples up to eight unread root titles from the full candidate set; Recently Added is limited to the recent date window and groups near-coincident additions from the same title.
- Tag listing order is count descending then tag name; tag cards expose display names, covers, counts, and per-user progress through Mango's common card component.
- Page image failures use HTTP 500 text; dimension lookup failures use the `{success:false,error}` JSON shape; missing title lookup for `/api/book/:tid` uses HTTP 404. Axum extractor failures and incidental internal errors may differ unless a route explicitly defines them.
- Admin actions are writes. All scan, thumbnail, tag, user, missing-record, metadata, and cover mutations must use disposable data in verification.

## Source-level parity findings addressed

These findings came from implementation comparison. Focused regressions cover auth status, page index, candidate limit ordering, bulk payload, bulk `last_read`, continuation sort overrides, cover persistence/serving, display-name persistence/rendering, and sort-title application. Rust API tests passed, and the affected Home, Library, Tags, Book, Reader, and Admin surfaces were compared in isolated Crystal and Rust apps.

1. **API authentication status:** Unauthenticated `/api` requests now return HTTP 401 with plain `Unauthorized`; normal pages still redirect, and OPDS/download retains its separate Basic Auth behavior.
2. **Reader page zero:** Page values are now explicitly 1-based; page `0` returns HTTP 500 rather than aliasing the first image.
3. **Home selection:** Continue Reading now selects the first eight candidates before sorting by last-read time; Start Reading shuffles the full unread root-title set before taking up to eight.
4. **Scan title count:** The scan response reports root title count, not all nested titles.
5. **Sort-title application:** Library, tag, reader, and home Continue Reading selection use persisted sort-title overrides; tag sorting exposes Auto, Date Modified, and Progress, and tag tie order is case-sensitive.
6. **Display-name/cover rendering:** HTML cards and reader surfaces use `info.json` display names and cover URLs; OPDS also uses explicit entry-cover URLs. Display-name updates write `info.json`. Historical SQLite display-name columns are retained by migration 006 but unused.
7. **Cover upload semantics:** Uploads accept title-level and entry-level targets, accept Mango's supported image types, stream to the configured upload directory, persist the URL in `info.json`, and are served publicly from `/uploads`; the Rust-only 10 MiB limit is removed.
8. **Bulk progress:** Success is `{success:true}`; bulk updates change selected progress values without changing `last_read`.
9. **User status:** Admin user rows render boolean `true`/`false`, matching Mango.
10. **Mutation response shape:** Missing-record success retains `error:null`; display-name and sort-title success responses omit `error`, matching their Mango handlers.

The source-level findings were checked against disposable data in `/tmp/mango-parity.FKVDe1`; the protected `/home/philip/mango-compare` data was not used.

## Source map

- Mango routes/auth: `Mango/src/server.cr`, `Mango/src/handlers/auth_handler.cr`, `Mango/src/routes/{main,admin,reader,api,opds}.cr`
- Mango state: `Mango/src/storage.cr`, `Mango/src/library/{library,title,entry,types,cache}.cr`, `Mango/migration/`
- Rust routes/auth: `src/server.rs`, `src/auth.rs`, `src/routes/{login,main,book,reader,admin,api,opds}.rs`
- Rust state: `src/storage.rs`, `src/library/{manager,title,entry,progress,progress_cache,cache}.rs`, `migrations/`
- Existing contract coverage: `tests/api/{auth,library,progress,admin,opds}.test.ts` and Rust library unit tests
