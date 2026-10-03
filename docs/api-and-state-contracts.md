# Mango-Rust Migration: Contracts and TODOs

## Status

**Migration incomplete.** Two source passes now inventory the production Crystal application and Rust port by implementation area, route, template, and asset. This is an exhaustive *source map* for the scoped trees, not a claim of behavioral parity: source-level matches still need external verification where noted, and confirmed differences remain open below.

## Open in-scope behavior gaps

1. **Route response contracts.** Remaining specific differences or unverified cases in the route matrices:
   - OPDS title feed: error-entry inclusion and MIME/display-field parity remains different or unverified.
   - Page/dimensions cache behavior and cover-upload validation/storage/URLs are tracked separately in gaps 2 and 4; scan timing is tracked in gap 3.
   - Home-section routes have been compared; Recently Added equal-timestamp ties are accepted (see below).
2. **Image cache responses.** Match Mango's ETags, conditional responses, and cache headers for page images and dimensions.
3. **Scan timing.** Match the synchronous result and state visibility of Mango's admin scan endpoint instead of Rust's background scan.
4. **Cover uploads.** Match Mango's image validation, storage paths, generated URLs, and upload error behavior.
5. **Persistence and scanning differences.** Mango and Rust migration histories are not interchangeable; Rust's gzip MessagePack library snapshot cannot read Mango's gzip YAML snapshot. Signature/ordering algorithms and recursive unread state differ as detailed below. Corrupt `info.json` handling is an accepted divergence noted below.

## Accepted divergences

- **Cookie SameSite policy.** Preserve Rust's `SameSite=Strict` session cookie (the pinned `tower-sessions` default). Mango does not specify `SameSite`; modern browsers commonly treat that as `Lax`. Strict reduces cross-site cookie exposure but can make a link from another site to Mango appear unauthenticated on the initial navigation.

- **Recently Added timestamp ties.** Crystal sorts by `date_added` alone, leaving equal-timestamp order unspecified; Rust sorts tied entries by each title's latest `date_added` descending, then `title_id` ascending. Tie-dependent grouping/order and first-eight results may differ, as documented in the route matrix.

- **`GET /api/cover/:tid/:eid` thumbnail fallback.** Keep Rust behavior: generate and persist a resized JPEG on cache miss; on thumbnail lookup or generation failure, serve the original first page. Crystal serves the original page on cache miss and errors on thumbnail lookup failure.

- **`POST /api/login` error wording.** Malformed input and exceptional failures use implementation-specific messages, but both return the same `403` JSON failure envelope. Treat the wording difference as a Crystal/Rust runtime detail.

- **`GET /api/book/:tid` and `GET /api/library` metadata differences.** Keep Rust's default-on-invalid-JSON behavior; Mango's `info.json` parser raises. On affected requests, Mango returns 404 text for `/api/book` or an HTTP-200 failure JSON for `/api/library`, while Rust continues with default metadata. A later Rust save can replace the malformed file with defaults. Also accepted: size-string formatting, inode-derived signatures across separate copies, and subsecond `TimeModified` ordering differences.

- **`PUT /api/sort_opt` noncanonical `sort` values.** Keep Rust behavior: it stores and returns the supplied string, while sorting case-folds and accepts aliases (`name`, `modified`, `time`, `added`). Crystal recognizes only canonical enum names and stores unknown/case-mismatched values as `auto`.

- **`GET/PUT /api/sort_opt` error wording.** Parser and filesystem errors may have runtime-specific messages; keep Rust wording, since both implementations return the same HTTP-200 `{success:false,error}` envelope.

- **Storage-error wording on progress, bulk-progress, display-name, sort-title, and tag handlers.** Treat message text as implementation-specific; preserve the status/envelope behavior documented in the route matrix.

## Resolved behavior gaps

- **CORS and preflight.** Rust answers unauthenticated `OPTIONS` requests under `/api`, `/uploads`, and `/img` with Mango's empty 200 response and exact allow headers. API responses carry those headers even without an `Origin` request header; unrelated browser and static responses do not.
- **Login callback redirects.** When an unauthenticated browser request reaches a protected path, Rust stores the path (not the query string) in the session and redirects to `/login`. Successful form login consumes that callback and redirects there; without a callback it redirects to `/`. API and OPDS authentication failures keep their existing non-redirect behavior.
- **Session lifetime.** Rust now issues a year-long session cookie and renews both the server-side expiry and cookie on active requests, matching Mango's 365-day rolling behavior. The pinned tower-sessions version saves modified sessions only, so a session middleware marks existing nonempty sessions modified before request handling.
- **Reverse-proxy cookie scope.** Session cookies now use the normalized `base_url` as their `Path`, as Mango does; both `/` and a non-root proxy prefix are covered by HTTP tests.
- **HTML error pages.** Browser-route 4xx and 5xx responses now render the shared Mango-style page layout with the route's error message; API, OPDS, upload, image, and static responses retain their non-HTML contracts.
- **Reader error branch.** Invalid archives remain indexed as error entries with stable IDs and `err_msg`. Reader continuation shows the archive path and error in a modal, with next-entry and return-to-title actions. Error cards and entry API responses expose the failure; archive covers use the default icon.
- **Existing-user rename.** `user_edit_post_existing` passes the URL username as the existing account key and the submitted form username as the new key. Its integration test checks the renamed listing, retained role, and unchanged password.
- **Admin and API response contracts.** User-management form failures redirect to the edit page with Mango-compatible error feedback. Display-name/sort-title failures retain Mango's HTTP-200 JSON failure envelope, and missing archive downloads return plain-text 404 responses.
- **Authentication modes.** Rust now accepts Basic credentials on every protected path, Bearer session IDs backed by the shared session store, `disable_login` with a validated `default_username`, and `auth_proxy_header_name` usernames after checking that the user exists. Focused HTTP tests cover these identities and admin role selection.
- **Configuration contracts.** Rust uses `-c/--config`, `CONFIG_PATH`, all same-named environment settings, YAML > environment > defaults precedence, Mango's `~/mango.db` default, and stored trailing-slash normalization for `base_url`. The config crate supplies builder defaults and layered YAML/environment loading; Serde deserializes the merged settings into `Config`.

- **User-management CLI.** Clap derive implements `admin user add/delete/update/list` with Mango's username/password/admin options, optional update password, and shared `-c/--config` handling. HTTP-independent integration tests exercise CRUD, list output, help, and global config placement.

- **User-input validation.** `src/storage.rs` enforces Mango's minimum username/password lengths, ASCII password requirement, and username character/first-character rules for create, update, and password-change operations. The storage boundary covers web/API and CLI callers; integration coverage checks invalid inputs and the exact minimum accepted values.
- **API reference surface.** Rust serves the authenticated `/api` ReDoc page and `/openapi.json`; Utoipa generates OpenAPI from endpoint annotations. The spec describes Rust's registered HTTP API operations.

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
| `GET /login`, `POST /login`, `GET /logout` | Same paths; `src/routes/login.rs` | Matched: successful form login consumes the saved callback and redirects there; without one it redirects to `/`. |
| `GET /`, `GET /library`, `GET /book/:title` | Same page shapes (`/book/:id`); `src/routes/main.rs`, `book.rs` | Matched route/page purposes; home/title data selection and template contracts have specific differences below. |
| `GET /tags`, `GET /tags/:tag` | Same paths; `src/routes/main.rs` | Matched route/page purposes; sort-query parsing and error behavior differ. |
| `GET /admin`, `/admin/user`, `/admin/user/edit`, `/admin/missing` | Same paths; `src/routes/admin.rs` | Matched route/page purposes; edit feedback is described on the form POST row. |
| `POST /admin/user/edit`, `POST /admin/user/edit/:original_username` | Same create/edit paths (`:username`); `src/routes/admin.rs` | Matched: failures redirect to `/admin/user/edit` with Mango-compatible error/query feedback; success redirects to `/admin/user`. |
| `GET /opds`, `GET /opds/book/:title_id` | Same paths; `src/routes/opds.rs` | Matched route/page purposes; Crystal omits error entries in title feed; exact rendered-field and error parity is not established. |
| `GET /api` | `GET /api`; `routes/reference.rs`, `templates/api.html` | Matched documentation page; ReDoc reads the generated `/openapi.json` spec. |
| `GET /download/plugins`, `GET /admin/downloads`, `GET /admin/subscriptions` | No Rust route/template | Explicitly excluded download/plugin UI. |

### Crystal JSON/WebSocket API routes

Every declaration in `Mango/src/routes/api.cr` was compared to the complete Rust Axum route table (`src/server.rs`) and implementations (`src/routes/{api,admin,login}.rs`). “Matched” below is route/operation mapping, not blanket response parity.

| Crystal API route(s) | Rust counterpart | Status and source-level difference |
|---|---|---|
| `POST /api/login` | `POST /api/login`; `login.rs::api_login` | Same success fields/status and invalid-credential response (`403`, `Nil assertion failed`). Malformed input and exceptional failures use runtime-specific error text within the same `403` JSON failure envelope; accepted wording difference. |
| `GET /api/page/:tid/:eid/:page` | Same; `api.rs::get_page` | ETag/cache headers differ; nonnumeric page values return 500 plain text with Mango's `Invalid Int32` message, while page-read failures return 500 plain text. |
| `GET /api/cover/:tid/:eid` | Same; `api.rs::get_cover` | Accepted Rust behavior: when no thumbnail is stored, generate and persist a resized JPEG thumbnail; if thumbnail lookup or generation fails, fall back to the original first page. Crystal returns the original first page on a cache miss and errors on thumbnail lookup failure. Missing-title/entry responses match; first-page read failures return plain-text 500s with route-specific error text. |
| `GET /api/book/:tid`, `GET /api/library` | Same; `api.rs::{get_title,get_library}` | Clean-fixture comparisons matched response shape, `depth`/`slim`/`percentage`, and tested sort behavior. Accepted differences: invalid `info.json` errors in Crystal (`/api/book` 404 text; `/api/library` HTTP-200 failure JSON), while Rust defaults metadata and continues; `TimeModified` sorting uses Crystal's subsecond `Time` versus Rust's whole-second mtime; signatures depend on inode values across copies; size strings use `humansize::BINARY` formatting. |
| `GET /api/sort_opt`, `PUT /api/sort_opt` | Same; `api.rs::{get_sort_opt,update_sort_opt}` | Missing-title errors use Mango's `Nil assertion failed`; malformed JSON stays in the HTTP-200 `{success:false,error}` envelope. Per-title sort updates refresh Rust's progress cache before the next response. Rust preserves noncanonical `sort` strings and accepts aliases/case variants when sorting; Crystal canonicalizes unrecognized values to `auto` (accepted Rust behavior). Parser and filesystem error wording may differ; accepted because both return the same HTTP-200 failure JSON envelope. |
| `GET /api/library/continue_reading`, `/start_reading`, `/recently_added` | Same; `api.rs::{continue_reading,start_reading,recently_added}` | Continue Reading matched on empty, in-progress, completed, and distinct last-read candidates; Start Reading returned the same unread root-title set (order is randomized). Recently Added has the same response shape and 24-hour grouping threshold. Rust sorts by `date_added` descending, then breaks ties by each title's latest date descending and `title_id` ascending. Crystal sorts by `date_added` alone, so tie order is unspecified and tie-dependent grouping/first-eight outputs can differ; accepted non-contractual divergence. |
| `POST /api/admin/scan` | Same; `admin.rs::scan_library` | Different: Crystal scans the current library synchronously; Rust scans in background and atomically swaps a new library. |
| `GET /api/admin/thumbnail_progress`, `POST /api/admin/generate_thumbnails` | Same; `admin.rs` | Matched routes, different generation/concurrency/progress state handling. |
| `DELETE /api/admin/user/delete/:username` | Same method/path; `admin.rs::delete_user_api` | Direct delete semantics and success body match Mango, including self-deletion and absent usernames (`200`, `{"success":true}`). |
| `PUT /api/progress/:tid/:page`, `PUT /api/bulk_progress/:action/:tid` | Same; `api.rs::update_progress`, `admin.rs::bulk_progress` | Operation failures, including malformed bulk JSON and nonnumeric progress values, return JSON `{success,error}` at HTTP 200; missing-target and parse messages match for covered cases, while storage errors may differ. Rust invalidates its progress cache after writes. |
| `PUT /api/admin/display_name/:tid/:name` | Same; `admin.rs::update_display_name` | Both return JSON `{success,error}` at HTTP 200 for operation failures; missing-target messages match Crystal, while storage failures may differ. |
| `PUT /api/admin/sort_title/:tid` | Same; `admin.rs::update_sort_title` | Both return JSON `{success,error}` at HTTP 200. With `eid`, both update only an entry belonging directly to `tid`; a missing or mismatched entry ID is a successful no-op. Storage-error wording is accepted as implementation-specific. |
| `POST /api/admin/upload/:target` (currently `cover`) | Fixed Rust `POST /api/admin/upload/cover`; `admin.rs::upload_cover` | Different: Rust only registers the cover path and accepts a different image-extension set; URL generation/storage/error paths differ. |
| `GET /api/dimensions/:tid/:eid` | Same; `api.rs::get_dimensions` | Different: Rust persists dimensions in SQLite and estimates unknown dimensions; ETag construction and cache semantics differ. |
| `GET /api/download/:tid/:eid` | Same; `api.rs::download_entry` | Missing-entry failures return plain text 404, matching Mango; file-read failures also map to plain-text 404. Not the excluded plugin chapter queue. |
| `GET /api/tags`, `GET /api/tags/:tid`, `PUT/DELETE /api/admin/tags/:tid/:tag` | Same; `api.rs` tag handlers | Success response shapes align with Crystal; add/delete and title-tag retrieval are integration-tested. Missing-title errors use `Nil assertion failed`; underlying storage error wording may differ. |
| `GET /api/admin/titles/missing`, `GET /api/admin/entries/missing` | Same; `admin.rs` | Matched response shape; both include `error: null` on success and an error string on failure. |
| `DELETE /api/admin/titles/missing`, `/api/admin/entries/missing`, and `/:tid` or `/:eid` forms | Same semantic operations using `:id`; `admin.rs` | Matched routes; item deletion is a no-op when the record is absent/not unavailable in the observed implementations. |
| `GET /openapi.json` | Same path; `routes/reference.rs` | Matched API-reference endpoint; Utoipa generates the document from Rust handler annotations. |
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
| Startup, server, sessions, logging | `Mango/src/{mango,main_fiber,server,logger}.cr`, `Mango/src/handlers/*.cr` → `src/{main,server,auth}.rs` | Different architectures. Auth modes (Basic on protected paths, Bearer session IDs, disabled-login identity, and proxy identity) now match the inspected Mango contract. Session cookie name, configured `base_url` path, and rolling 365-day expiry match; Rust's `SameSite=Strict` instead of Mango's omitted attribute is an accepted divergence. Rust matches Mango's unauthenticated `/api`, `/uploads`, and `/img` preflight and API CORS headers; other base-path behavior, custom logging, and static mounts remain different. |
| Configuration | `Mango/src/config.cr` → `src/config.rs` | Config path, same-named environment settings, file > environment > defaults precedence, database default, path expansion, and stored trailing-slash normalization are implemented. `ConfigBuilder::set_default` supplies schema defaults, then `try_deserialize` uses Serde; the CLI forwards explicit `-c/--config` paths. |
| User storage and validation | `Mango/src/storage.cr`, `Mango/src/util/validation.cr` → `src/storage.rs`, `src/routes/admin.rs`, `src/routes/main.rs`, `src/main.rs` | Rust applies Mango's username/password rules at storage create/update/password-change boundaries, covering web/API and CLI callers. |
| SQLite schema/migration history | All 12 `Mango/migration/*.cr` → all 7 `migrations/*.sql`, `src/storage.rs` | Current core tables overlap, but Rust creates final schemas rather than replaying Crystal's history. Crystal relative-path migrations and `md_account` are absent in Rust; legacy DB conversion is not demonstrated. MangaDex account token table is listed with excluded integration below. Rust also has `display_name` and `dimensions` schema additions. |
| Identity and unavailable records | `Mango/src/storage.cr`, `Mango/src/library/title.cr` → `src/storage.rs`, `src/library/{manager,title}.rs` | Both reconcile stable IDs by path/signature and track unavailable records; matching/fallback policy is analogous, but transaction/upsert/failure ordering differs. |
| Archive/directory scan | `Mango/src/{archive.cr,library/{archive_entry,dir_entry,title}.cr,util/validation.cr}` → `src/library/{entry,title}.rs`, `src/util.rs` | Invalid archives are retained as error entries with `err_msg` and zero pages, as in Crystal. Extractable extension sets and image filters still differ; Rust uses `compress-tools`, Crystal ZIP plus `archive.cr`. |
| Signatures and ordering | `Mango/src/util/{signature,numeric_sort,chapter_sort,util}.cr`, library title/entry → `src/util.rs`, `src/library/{manager,title,entry}.rs` | Different/partly matched: Unix file/directory signature construction is similar in parts; non-Unix fallback differs. Crystal has BigInt numeric comparison and semantic `ChapterSorter`; Rust uses `natord`. Persistent ID stability for ties/extension differences is not established. |
| `info.json` state | `Mango/src/library/{types,title,entry}.cr` → `src/library/{progress,progress_cache,title}.rs` | Field/key intent matches (progress, last-read, date-added, sort, display names, covers). Corrupt JSON raises in Mango but becomes default metadata in Rust (accepted; see above); Rust migrates old ID keys. Unread-all stores zero in Crystal but removes progress keys in Rust. |
| Library snapshot and in-memory cache | `Mango/src/library/{library,cache}.cr` → `src/library/cache/{file,lru,key}.rs`, `src/library/manager.rs` | Different: gzip YAML versus gzip MessagePack snapshots cannot be read interchangeably; cache organization and invalidation differ. |
| Recent/home selection | `Mango/src/library/{library,title}.cr` → `src/routes/{main,recently_added,api}.rs`, `src/library/title.rs` | Runtime-compared on a shared fixture: Continue Reading matched for empty, active, completed, and distinct last-read cases; Start Reading returned the same unread root-title set (order randomized). Recently Added matched in response shape and grouping threshold; exact timestamp ties can affect group counts/order and the first eight items (see route contract above). |
| Thumbnail, dimensions, cover upload | `Mango/src/storage.cr`, `Mango/src/library/{entry,title}.cr`, `Mango/src/upload.cr` → `src/storage.rs`, `src/routes/{api,admin}.rs` | Different: thumbnails have analogous DB storage/generation; dimensions are Rust-only cache data. Crystal upload helper is generic save/path-to-URL; Rust serves upload files via `ServeDir` and exposes a specific cover upload route. |
| Rename utility | `Mango/src/rename.cr` → no Rust counterpart found | Crystal utility file has no caller in the inventoried `Mango/src` tree; treat as unreachable/dead source, not an in-scope runtime gap. |

## Views, client assets, and static serving

| Crystal view | Rust counterpart | Status |
|---|---|---|
| `views/admin.html.ecr`, `home.html.ecr`, `library.html.ecr`, `login.html.ecr`, `missing-items.html.ecr`, `tag.html.ecr`, `tags.html.ecr`, `title.html.ecr`, `user-edit.html.ecr`, `user.html.ecr` | `templates/{admin,home,library,login,missing-items,tag,tags,book,user-edit,users}.html` | Matched page purposes. Differences include missing-item ID display formatting, route form/error behavior, active navigation, and base URL assumptions. |
| `views/layout.html.ecr` | `templates/base.html` | Matched layout purpose; Rust has active-nav state. Crystal and Rust templates reference different static URL roots; both layouts expose links to excluded download/subscription paths. |
| `views/reader.html.ecr` | `templates/reader.html` | Matched page; both active pages load the corresponding Alpine reader client. |
| `views/opds/{index,title}.xml.ecr` | `templates/opds_{index,title}.xml` | Matched feeds; title feed omits Crystal error entries and maps MIME/display fields differently or remains unverified. |
| `views/api.html.ecr` | `templates/api.html`, `routes/reference.rs` | Matched API documentation page; both render ReDoc and point it at `/openapi.json`. |
| `views/reader-error.html.ecr` | `templates/reader-error.html`, `src/routes/reader.rs` | Matched reader error purpose and shared-layout rendering: path and error modal with next-entry and return-to-title actions. |
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
- Rust implementation: `src/{auth,config,lib,main,server,storage,util}.rs`; `src/library/{cache/{file,key,lru,mod},entry,manager,mod,progress,progress_cache,title}.rs`; `src/routes/{admin,api,book,login,main,mod,opds,reader,recently_added,reference}.rs`; inline `error` module in `src/lib.rs`.
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
