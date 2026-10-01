# Mango Parity and Refactoring Review

## Scope

Compared the Mango Crystal implementation under `Mango/src/` with Rust library scanning, progress and title metadata, reader ordering, shared JSON APIs, OPDS feeds, and API route registration. `Mango/` is the behavioral reference. The local comparison apps use separate databases and `info.json` files, so their saved progress is not expected to match.

## Parity gaps corrected

- **Stable IDs after moves and revivals.** `Storage#get_title_id` and `Storage#get_entry_id` match active records by path and signature, then path, then signature with path-component similarity; fallback matches refresh path, signature, and availability. Rust now follows that lookup sequence for both titles and entries. Unused instance lookup/persistence helpers were removed.
- **Sorting.** Mango applies persisted `date_added` timestamps and the selected progress order. Rust's book/API paths now order by stored dates; continuation selection uses progress ordering; reader previous/next links use the user's configured sort and stored entry sort-title overrides.
- **Recently added.** Equal `date_added` timestamps are ordered by each title's latest timestamp before the existing grouping step. This keeps same-title entries adjacent when second-resolution timestamps tie, without changing Mango's 24-hour grouping condition.
- **Dimensions responses.** Directory-entry ETags include the formatted size and directory entries use Mango's `no-cache, max-age=86400` policy. Missing-title and missing-entry responses use Mango's JSON failure body and HTTP 200 behavior.
- **OPDS.** Rust now uses the configured base URL, emits child-title navigation entries and stored display names, and uses Mango's continue-reading URL shape.
- **Dead sort-cache code.** Removed the unused cached-entry sorting method and its unconsumed sorted-entry, progress-sum, and `info.json` cache-key paths. Retained the active per-user sorted-title cache. Removed `dead_code` allowances from used home-card constructors and loaded the title sort override for the book header.

## Known differences and boundaries

- Axum extractor failures and some internal-error details still differ from Mango's exception handling. Do not treat the APIs as fully identical on malformed requests or every internal failure path.
- Plugin, subscription, and MangaDex queue APIs remain unimplemented in Rust.
- Rust still exposes product-specific admin/cache APIs that have no direct Mango endpoint counterpart. They were not removed as part of this parity pass because they serve Rust UI/admin behavior; shared Mango route contracts were kept distinct from these additions.
- The comparison databases and per-title `info.json` files are independent. Different existing read progress and date-added state produce different visible progress until each app is given equivalent state.

## Refactoring

- Keep `CONTEXT.md` a glossary. This review document holds implementation findings and possible refactors instead of mixing those details into the domain vocabulary.
- Sorting rules are still assembled in multiple places (`routes/api.rs`, `routes/book.rs`, `routes/reader.rs`, and continuation selection). A shared user-aware sorter could reduce drift, but it would need to preserve Mango's distinct persisted date, progress, and sort-title behavior; do not consolidate until those contracts are covered by focused behavior checks.
- Continue preferring direct parity fixes over broad route/model rewrites. The review found no evidence that the separate Rust admin/cache features should be replaced by Mango routes.
