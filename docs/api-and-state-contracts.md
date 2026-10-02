# Mango-Rust Migration: Contracts and TODOs

## Status

**Migration incomplete.** Two source passes now inventory the production Crystal application and Rust port by implementation area, route, template, and asset. This is an exhaustive *source map* for the scoped trees, not a claim of behavioral parity: source-level matches still need external verification where noted, and confirmed differences remain open below.

## Open in-scope behavior gaps

1. **Existing-user rename.** Mango's `POST /admin/user/edit/:original_username` applies the submitted username. Rust's `src/routes/admin.rs::user_edit_post_existing` passes the path username as both old and new names, ignoring the editable form field.
2. **User-input validation.** `Mango/src/util/validation.cr` requires usernames of at least 3 characters matching `[A-Za-z_][A-Za-z0-9_-]*` and passwords of at least 6 ASCII characters. Rust web/API create and update paths call `Storage::create_user`/`update_user` without those checks; the CLI checks only password length.
3. **Authentication modes.** Mango supports Bearer tokens, `disable_login` with `default_username`, and `auth_proxy_header_name` (`Mango/src/handlers/auth_handler.cr`). Rust `require_auth` supports sessions, Basic credentials on OPDS/archive-download paths, and browser/API failures, but not those Mango modes.
4. **Configuration contracts.** Mango honors `CONFIG_PATH`, derives environment variables for every option, uses file > environment > default precedence, and normalizes `base_url` to end in `/`. Rust loads a fixed default path unless its caller passes a path, overrides only selected `MANGO_*` variables after YAML, and validates but does not store the normalized trailing slash. Default database paths also differ: `~/mango.db` in Mango, `~/mango/mango.db` in Rust.
5. **CLI parity.** Mango supports `admin user add/delete/update/list` and `--config`; Rust implements only `admin user update <username> --password <password>` and loads the default config.
6. **API-reference surface.** Mango serves the `/api` page and `/openapi.json`; Rust registers neither. Documentation surface only, not a missing API operation.
7. **Reader error branch.** Mango renders `reader-error.html.ecr` when an entry has `err_msg`, with next-entry and return-to-title actions. Rust's reader has no equivalent error page/branch.
8. **HTTP and state differences.** CORS/preflight headers, 365-day versus 7-day sessions, login callback redirects, HTML error pages versus Rust text errors, route status/body behavior, image ETags/cache headers, scan timing, and cover upload validation/path semantics differ in inspected source; details are recorded in the route matrix.
9. **Persistence and scanning differences.** Mango and Rust migration histories are not interchangeable; Rust's gzip MessagePack library snapshot cannot read Mango's gzip YAML snapshot. Invalid archives are retained as error entries by Mango but dropped by the Rust scanner. Signature/ordering algorithms, recursive unread state, and corrupt `info.json` handling differ as detailed below.

## Status vocabulary

- **Matched:** a counterpart was inspected and the named behavior appears present; this alone does not prove complete parity.
- **Different:** source shows a concrete contract, state, algorithm, or route divergence.
- **Missing:** no counterpart was found in the inventoried Rust runtime surface.
- **Explicitly excluded:** present in Crystal but outside the current agreed scope (“downloads”).
- **Unverified:** source was inspected, but a field-level/runtime conclusion is not supported.

## Route inventory

### HTML/XML routes

| Crystal method/path | Rust method/path and implementation | Status |
|---|---|---|
| `GET /login`, `POST /login`, `GET /logout` | Same paths; `src/routes/login.rs` | Different: Mango consumes saved callback after login; Rust always redirects to `/`. |
| `GET /`, `GET /library`, `GET /book/:title` | Same page shapes (`/book/:id`); `src/routes/main.rs`, `book.rs` | Matched route/page purposes; home/title data selection and template contracts have specific differences below. |
| `GET /tags`, `GET /tags/:tag` | Same paths; `src/routes/main.rs` | Matched route/page purposes; sort-query parsing and error behavior differ. |
| `GET /admin`, `/admin/user`, `/admin/user/edit`, `/admin/missing` | Same paths; `src/routes/admin.rs` | Matched route/page purposes; user validation and edit-error feedback differ. |
| `POST /admin/user/edit`, `POST /admin/user/edit/:original_username` | Same create/edit paths (`:username`); `src/routes/admin.rs` | Different: Rust cannot rename the existing user and does not reproduce Mango's error-query redirect contract. |
| `GET /reader/:title/:entry`, `GET /reader/:title/:entry/:page` | Same route shapes (`:tid/:eid[/page]`); `src/routes/reader.rs` | Different: Mango continuation detects errored entries and renders a dedicated error page; no Rust equivalent found. |
| `GET /opds`, `GET /opds/book/:title_id` | Same paths; `src/routes/opds.rs` | Matched route/page purposes; Crystal omits error entries in title feed; exact rendered-field and error parity is not established. |
| `GET /api` | No Rust route/template | Missing documentation page. |
| `GET /download/plugins`, `GET /admin/downloads`, `GET /admin/subscriptions` | No Rust route/template | Explicitly excluded download/plugin UI. |

### Crystal JSON/WebSocket API routes

Every declaration in `Mango/src/routes/api.cr` was compared to the complete Rust Axum route table (`src/server.rs`) and implementations (`src/routes/{api,admin,login}.rs`). “Matched” below is route/operation mapping, not blanket response parity.

| Crystal API route(s) | Rust counterpart | Status and source-level difference |
|---|---|---|
| `POST /api/login` | `POST /api/login`; `login.rs::api_login` | Matched route; malformed JSON/session errors and failure payloads differ. |
| `GET /api/page/:tid/:eid/:page` | Same; `api.rs::get_page` | Different: ETag/cache headers and error contracts diverge. |
| `GET /api/cover/:tid/:eid` | Same; `api.rs::get_cover` | Different: thumbnail lookup/generation and page-1 fallback paths differ. |
| `GET /api/book/:tid`, `GET /api/library` | Same; `api.rs::{get_title,get_library}` | Matched routes; title/parent fields exist on both sides, but exact field/order and percentage parity is not globally verified. |
| `GET /api/sort_opt`, `PUT /api/sort_opt` | Same; `api.rs::{get_sort_opt,update_sort_opt}` | Matched routes; request fields match Crystal implementation (`sort`, `ascend`); error messages differ. |
| `GET /api/library/continue_reading`, `/start_reading`, `/recently_added` | Same; `api.rs::{continue_reading,start_reading,recently_added}` | Matched routes; both limit/group home sections, but selection, timestamp tie ordering, and grouping boundaries need external comparison. |
| `POST /api/admin/scan` | Same; `admin.rs::scan_library` | Different: Crystal scans the current library synchronously; Rust scans in background and atomically swaps a new library. |
| `GET /api/admin/thumbnail_progress`, `POST /api/admin/generate_thumbnails` | Same; `admin.rs` | Matched routes, different generation/concurrency/progress state handling. |
| `DELETE /api/admin/user/delete/:username` | Same method/path; `admin.rs::delete_user_api` | Matched route, different: Rust prevents self-deletion and checks existence; Crystal directly deletes. |
| `PUT /api/progress/:tid/:page`, `PUT /api/bulk_progress/:action/:tid` | Same; `api.rs::update_progress`, `admin.rs::bulk_progress` | Matched routes; response/error behavior differs; Rust invalidates progress cache after writes. |
| `PUT /api/admin/display_name/:tid/:name` | Same; `admin.rs::update_display_name` | Matched route; Rust propagates typed errors where Crystal returns `{success:false,error}` JSON. |
| `PUT /api/admin/sort_title/:tid` | Same; `admin.rs::update_sort_title` | Global sort-title persistence is matched: Crystal's username argument is unused. Invalid title/entry scoping and error behavior differ. |
| `POST /api/admin/upload/:target` (currently `cover`) | Fixed Rust `POST /api/admin/upload/cover`; `admin.rs::upload_cover` | Different: Rust only registers the cover path and accepts a different image-extension set; URL generation/storage/error paths differ. |
| `GET /api/dimensions/:tid/:eid` | Same; `api.rs::get_dimensions` | Different: Rust persists dimensions in SQLite and estimates unknown dimensions; ETag construction and cache semantics differ. |
| `GET /api/download/:tid/:eid` | Same; `api.rs::download_entry` | Matched route: serves an indexed archive for clients; Rust maps file-read and missing-entry errors differently from Crystal. Not the excluded plugin chapter queue. |
| `GET /api/tags`, `GET /api/tags/:tid`, `PUT/DELETE /api/admin/tags/:tid/:tag` | Same; `api.rs` tag handlers | Matched routes; Crystal uses title methods while Rust reads/writes storage directly; response parity not runtime-verified. |
| `GET /api/admin/titles/missing`, `GET /api/admin/entries/missing` | Same; `admin.rs` | Matched routes; Rust success JSON includes nullable `error`; exact serialization differs. |
| `DELETE /api/admin/titles/missing`, `/api/admin/entries/missing`, and `/:tid` or `/:eid` forms | Same semantic operations using `:id`; `admin.rs` | Matched routes; item deletion is a no-op when the record is absent/not unavailable in the observed implementations. |
| `GET /openapi.json` | No Rust route | Missing API-reference endpoint. |
| `WS /api/admin/mangadex/queue`, `GET /api/admin/mangadex/queue`, `POST /api/admin/mangadex/queue/:action` | No Rust route | Explicitly excluded MangaDex/download queue. Actions include delete/retry/pause/resume. |
| Plugin routes: `GET /api/admin/plugin`, `/plugin/info`, `/plugin/search`, `/plugin/list`; `GET/POST/DELETE /api/admin/plugin/subscriptions`; `POST /api/admin/plugin/subscriptions/update`, `/api/admin/plugin/download` | No Rust route | Explicitly excluded plugin/source discovery, chapter downloads, and subscriptions. |

### Rust-only route additions

| Rust routes | Source | Status |
|---|---|---|
| `GET /change-password`, `POST /api/user/change-password` | `src/routes/main.rs` | Rust-only self-service password change. |
| `GET /debug/cache`, `POST /api/cache/{clear,save-library,load-library,invalidate}` | `src/routes/admin.rs` | Rust-only diagnostics/cache operations. |
| `GET/POST /api/admin/users`, `PATCH/DELETE /api/admin/users/:username` | `src/routes/admin.rs` | Rust-only plural REST user API; Crystal has HTML user forms and a singular delete API. |

## Implementation and persistence inventory

| Area | Crystal → Rust sources | Status and findings |
|---|---|---|
| Startup, server, sessions, logging | `Mango/src/{mango,main_fiber,server,logger}.cr`, `Mango/src/handlers/*.cr` → `src/{main,server,auth}.rs` | Different architectures. Same session cookie name; 365-day Crystal session versus 7-day Rust inactivity expiry. Crystal adds configured base path, custom log formatting and OPTIONS/CORS behavior; Rust uses Axum/Tower tracing and filesystem static mounts. |
| Configuration | `Mango/src/config.cr` → `src/config.rs` | Different: environment key set/prefix, precedence, `CONFIG_PATH`, trailing-slash normalization, and default DB path differ. Rust's `validate` computes a normalized URL copy but leaves `base_url` unchanged. |
| User storage and validation | `Mango/src/storage.cr`, `Mango/src/util/validation.cr` → `src/storage.rs`, `src/routes/admin.rs`, `src/main.rs` | DB operations broadly overlap; validation policy does not. Rust web/API create/update paths do not apply Mango username/password checks; CLI checks password length only. |
| SQLite schema/migration history | All 12 `Mango/migration/*.cr` → all 7 `migrations/*.sql`, `src/storage.rs` | Current core tables overlap, but Rust creates final schemas rather than replaying Crystal's history. Crystal relative-path migrations and `md_account` are absent in Rust; legacy DB conversion is not demonstrated. MangaDex account token table is listed with excluded integration below. Rust also has `display_name` and `dimensions` schema additions. |
| Identity and unavailable records | `Mango/src/storage.cr`, `Mango/src/library/title.cr` → `src/storage.rs`, `src/library/{manager,title}.rs` | Both reconcile stable IDs by path/signature and track unavailable records; matching/fallback policy is analogous, but transaction/upsert/failure ordering differs. |
| Archive/directory scan | `Mango/src/{archive.cr,library/{archive_entry,dir_entry,title}.cr,util/validation.cr}` → `src/library/{entry,title}.rs`, `src/util.rs` | Different: Crystal validates archives and retains invalid entries with `err_msg`; Rust extraction errors are surfaced during scan and entries are dropped. Extractable extension sets and image filters differ; Rust uses `compress-tools`, Crystal ZIP plus `archive.cr`. |
| Signatures and ordering | `Mango/src/util/{signature,numeric_sort,chapter_sort,util}.cr`, library title/entry → `src/util.rs`, `src/library/{manager,title,entry}.rs` | Different/partly matched: Unix file/directory signature construction is similar in parts; non-Unix fallback differs. Crystal has BigInt numeric comparison and semantic `ChapterSorter`; Rust uses `natord`. Persistent ID stability for ties/extension differences is not established. |
| `info.json` state | `Mango/src/library/{types,title,entry}.cr` → `src/library/{progress,progress_cache,title}.rs` | Field/key intent matches (progress, last-read, date-added, sort, display names, covers). Different: corrupt JSON raises through Mango's read path but becomes default metadata in Rust; Rust migrates old ID keys. Unread-all stores zero in Crystal but removes progress keys in Rust. |
| Library snapshot and in-memory cache | `Mango/src/library/{library,cache}.cr` → `src/library/cache/{file,lru,key}.rs`, `src/library/manager.rs` | Different: gzip YAML versus gzip MessagePack snapshots cannot be read interchangeably; cache organization and invalidation differ. |
| Recent/home selection | `Mango/src/library/{library,title}.cr` → `src/routes/{main,recently_added,api}.rs`, `src/library/title.rs` | Counterparts exist. Crystal continuation selects one per nested title then sorts and caps; Rust selects and caps candidates in route/helper. Recently-added grouping uses same-title batches within a day, but ordering/tie behavior differs in implementation; not runtime-compared. |
| Thumbnail, dimensions, cover upload | `Mango/src/storage.cr`, `Mango/src/library/{entry,title}.cr`, `Mango/src/upload.cr` → `src/storage.rs`, `src/routes/{api,admin}.rs` | Different: thumbnails have analogous DB storage/generation; dimensions are Rust-only cache data. Crystal upload helper is generic save/path-to-URL; Rust serves upload files via `ServeDir` and exposes a specific cover upload route. |
| Rename utility | `Mango/src/rename.cr` → no Rust counterpart found | Crystal utility file has no caller in the inventoried `Mango/src` tree; treat as unreachable/dead source, not an in-scope runtime gap. |

## Views, client assets, and static serving

| Crystal view | Rust counterpart | Status |
|---|---|---|
| `views/admin.html.ecr`, `home.html.ecr`, `library.html.ecr`, `login.html.ecr`, `missing-items.html.ecr`, `tag.html.ecr`, `tags.html.ecr`, `title.html.ecr`, `user-edit.html.ecr`, `user.html.ecr` | `templates/{admin,home,library,login,missing-items,tag,tags,book,user-edit,users}.html` | Matched page purposes. Differences include missing-item ID display formatting, route form/error behavior, active navigation, and base URL assumptions. |
| `views/layout.html.ecr` | `templates/base.html` | Matched layout purpose; Rust has active-nav state. Crystal and Rust templates reference different static URL roots; both layouts expose links to excluded download/subscription paths. |
| `views/reader.html.ecr` | `templates/reader.html` | Matched page; both active pages load the corresponding Alpine reader client. |
| `views/opds/{index,title}.xml.ecr` | `templates/opds_{index,title}.xml` | Matched feeds; title feed omits Crystal error entries and maps MIME/display fields differently or remains unverified. |
| `views/api.html.ecr` | No Rust template/route | Missing docs page. |
| `views/reader-error.html.ecr` | No Rust template/route | Missing reachable Crystal reader error branch. |
| `views/message.html.ecr` | Rust `Error::into_response` in `src/lib.rs` returns status plus text; no HTML error page | Different: Mango's 404/500 and utility error helpers render this fragment inside the shared layout. |
| `views/download-manager.html.ecr`, `plugin-download.html.ecr`, `subscription-manager.html.ecr` | No Rust page counterparts | Explicitly excluded download/plugin UI. |
| `views/components/{card,dots,entry-modal,sort-form}.html.ecr` | `templates/components/{card,dots,entry-modal,sort-form}.html` | Matched component purposes; card grouped-entry contract and base-URL handling differ; sort-form data shapes differ. |
| `views/components/{head,jquery-ui,moment,uikit}.html.ecr` | No one-to-one components | Markup/assets are inlined or relocated in Rust templates; not standalone page gaps. Crystal edit-modal markup is inline in `title.html.ecr`; Rust extracts it as `edit-modal.html`. |

Client/static comparison:

| Crystal source/build | Rust source/build | Status |
|---|---|---|
| `Mango/gulpfile.js`, `Mango/package.json`; release build emits `dist/` JS/CSS and copies public assets | Root `package.json`, `Dockerfile`; `npm run build` emits `static/dist/{css,img,webfonts}`; `src/server.rs` serves filesystem `static/` at `/static` | Different packaging paths and JS processing: Crystal release Babel-transpiles/minifies public JS; Rust serves copied JS files directly. |

- All same-named Crystal/Rust JavaScript files have identical SHA-256 hashes except `missing-items.js`; Rust preserves hyphens when parsing IDs while Crystal truncates IDs at the second hyphen. Rust adds `cache_debug.js`. Download/plugin/subscription scripts are byte-identical copies but remain explicitly excluded because their Rust routes/pages are absent.
- `subscription.js` is present in both asset trees but is not included by an inspected template; it calls MangaDex subscription endpoints not declared in Crystal's API routes. Treat it as a dormant client file, not reachable behavior.
- `static/js/reader.js` is the active Rust reader script and matches `Mango/public/js/reader.js`; `templates/reader.js` was inspected but is unreferenced by the active reader template.
- `tags.less`, manifests, `robots.txt`, favicon, and all enumerated image/icon assets have matching hashes. Rust `mango.less` adds card-badge styling. Rust `uikit.less` changes hook invocation and UIkit image paths to `../dist/img/`; stylesheet sources are not identical.
- Crystal release handling embeds `dist` or `public` assets and uses `/js`, `/css`, `/img`; Rust mounts filesystem `static/` at `/static`. Upload serving also differs: Crystal configurable upload URL handler versus Rust `/uploads` `ServeDir`.

## Explicitly excluded download/plugin surface

Mango's `queue.cr`, `plugin/{plugin,downloader,subscriptions,updater}.cr`, plugin and queue routes, download-manager/plugin-download/subscription views, and corresponding public JavaScript implement a Duktape plugin runtime, remote chapter search/download, job persistence/actions, subscriptions, and periodic updates. No equivalent Rust runtime/routes were found. These are **explicitly excluded only under the agreed downloads scope**, not marked matched.

`Mango/migration/md_account.11.cr` and `Storage` token/expiry methods form a dormant MangaDex-account persistence surface: no caller was found in the inventoried Crystal source, and the paired `subscription.js` is not included by the inspected views. The schema/methods are recorded as excluded with the integration, not as a confirmed active contract. Rust's `/api/download/:tid/:eid` only serves an already indexed local archive and is not a substitute.

### Files covered in the source map

- Crystal runtime: `Mango/src/{archive,config,logger,main_fiber,mango,queue,rename,server,storage,upload}.cr`; `Mango/src/handlers/{auth_handler,cors_handler,log_handler,static_handler,upload_handler}.cr`; `Mango/src/library/{archive_entry,cache,dir_entry,entry,library,title,types}.cr`; `Mango/src/plugin/{downloader,plugin,subscriptions,updater}.cr`; `Mango/src/routes/{admin,api,main,opds,reader}.cr`; `Mango/src/util/{chapter_sort,numeric_sort,proxy,signature,util,validation,web}.cr`.
- Crystal presentation/assets: every `Mango/src/views/**/*.ecr`, `Mango/public/js/*.js`, `Mango/public/css/*.less`, `Mango/public/{manifest.json,robots.txt,favicon.ico}`, and `Mango/public/img/**`.
- Rust implementation: `src/{auth,config,lib,main,server,storage,util}.rs`; `src/library/{cache/{file,key,lru,mod},entry,manager,mod,progress,progress_cache,title}.rs`; `src/routes/{admin,api,book,login,main,mod,opds,reader,recently_added}.rs`; inline `error` module in `src/lib.rs`.
- Rust presentation/assets: all files under `templates/` and `templates/components/`, `static/js/*.js`, `static/css/*.less`, `static/{manifest.json,robots.txt,favicon.ico}`, and `static/img/**`. `templates/reader.js` is present but no runtime reference was found; it is not the active reader script.
- Schema/build sources: all 12 Crystal migrations (`users.1`, `ids.2`, `thumbnails.3`, `tags.4`, `titles.5`, `foreign_keys.6`, `ids_signature.7`, `relative_path.8`, `unavailable.9`, `relative_path_fix.10`, `md_account.11`, `sort_title.12`) and Rust `migrations/001_users.sql` through `007_dimensions.sql`; `Mango/gulpfile.js`, `Mango/package.json`, root `package.json`, and `Dockerfile`.

The fixture `Mango/spec/asset/test-config.yml` and test/spec source files are not runtime implementations; specs were consulted only as corroboration. Binary assets were enumerated and paired hashes checked; their pixels were not compared visually.

## Source coverage and verification boundary

The inventory pass covered all production files in `Mango/src/` and `src/`, all 12 Crystal and 7 Rust migrations, all Crystal view/component/OPDS templates, all Rust templates/components, and all public/static JS, CSS, metadata, and image/icon filenames. Route declarations were checked across all Crystal route files and the full Rust router. `Mango/spec/` and Rust tests were used only as corroboration; they are not runtime implementation. Unreferenced code is explicitly identified above: Crystal `rename.cr`, Crystal `public/js/subscription.js`, dormant MangaDex account persistence methods, and Rust `templates/reader.js`.

Source checks included complete route/handler reads, a second pass over omitted library/storage/template regions, `sha256sum` comparisons for paired assets, and focused diffs for stylesheet and missing-item-client differences. No browser/runtime scenario was run in this inventory pass; source presence or matching hashes do not establish rendered UI, HTTP response, persistence, or migration behavior equivalence.

## Completion rule

Keep migration status **incomplete** until each in-scope `different`, `missing`, and `unverified` behavior is fixed or explicitly approved for exclusion, and externally visible results/state transitions are verified with focused tests or paired browser comparisons. Report coverage and unexamined areas; never infer global parity from a scoped pass.

## Source map

- Crystal reference: `Mango/src/`, `Mango/migration/`, `Mango/public/`.
- Rust port: `src/`, `migrations/`, `templates/`, `static/`.
- UI comparison requires equivalent library data and per-title `info.json` state; comparison apps use separate databases.
