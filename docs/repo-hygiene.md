# Repository hygiene

Branch: `repo-hygiene`. Preserve behavior and data contracts; use Crystal Mango as a reference when intent is unclear. Commit each verified refactor separately.

## Scan worker ownership

- Each worker owns its `PendingIds` buffers and returns them alongside its completed title. The collector owns the combined buffers and persists them before publishing a snapshot.
- Removed shared mutex-protected ID vectors and per-record locking. Failed workers cannot contribute partially assigned IDs to another worker's publication batch.
- Kept immutable library snapshots, concurrency limits, persistent ID matching, and publication batching unchanged.
- Verification: `cargo test --all-targets` (64 passed across the combined scan, traversal, and cache edits); isolated executable smoke scanned nested image directories, rescanned a changed directory, confirmed stable IDs and published page counts, and checked active SQLite title/entry records.

## Recursive title traversal

- `deep_entries` and `deep_titles` keep their existing interfaces and depth-first ordering, accumulating into one result vector rather than allocating vectors for each subtree.
- `total_pages` sums recursively without collecting entry references.
- Verification: nested traversal regression covers own entries, grandchildren, siblings, empty titles, and page totals. All 64 tests passed; the isolated scan executable also exercised nested traversal and page totals before and after a rescan.

## Cache key ownership and invalidation

- The LRU map owns each key once; debug entries derive their keys from map iteration.
- Prefix invalidation removes matching entries in place and updates byte accounting, without allocating debug metadata. Serialized values, byte budgets, invalidation logging, and eviction statistics retain their existing semantics.
- Verification: regressions cover selective prefixes, reclaimed budget reuse, unchanged counters, no matches, empty prefixes, and repeated invalidation. All 64 tests passed; the isolated executable confirmed per-user invalidation, retained debug keys, and reclaimed bytes.

## Catalog response options

- Grouped recursive response formatting and traversal controls in a private `TitleResponseOptions` value. Request dependencies stay borrowed; each child inherits resolved sorting and formatting while decrementing only positive depth.
- Removed the intermediate entry vector that wrapped sort titles in `Some` only to borrow them immediately afterward. Timestamp ordering uses `sort_by_key` with `Reverse`.
- Verification: all 64 Rust tests and strict all-target Clippy passed. A disposable container using the rebuilt image exercised nested recursion, depths zero/one/unlimited, invalid-depth defaults, parent breadcrumbs, slim output, percentage alignment, and inherited descending sorting through HTTP.

## Borrowed card model inputs

- Home and book entry-card constructors now borrow `Entry`, `Title`, and progress metadata rather than accepting eight independent primitive arguments. Template-specific card fields and sort-title defaults remain distinct.
- Continue-reading timestamp ordering uses `sort_by_key` with `Reverse`.
- Verification: all 64 Rust tests and strict all-target Clippy passed. Rebuilt `mango-rust:local`, ran `./local-dev/run-local.sh`, signed into both apps, inspected home and matching book cards, and opened entry modals. Covers, titles, page counts, badges, and modal paths/actions rendered; the two comparison databases have different progress values, so their badges/home selections are not identical.
- Comparison browser sessions use `127.0.0.1:9000` for Crystal and `localhost:9001` for Rust to isolate same-host cookies from the containers' internal port configuration.

## Idiomatic Rust cleanup

- Replaced remaining simple descending comparators with key-based ordering, a single-pattern thumbnail match with `if let`, a redundant `PathBuf` conversion with direct ownership transfer, and post-default field assignment with a struct initializer.
- Removed an orphaned doc comment. The card template uses an inclusive range check for progress badges, preserving the zero/100 boundaries and hiding out-of-range values.
- Verification: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` passed (64 tests). Rebuilt Docker image and local launcher succeeded; home/book badges and entry modals were visually inspected, and the cache debug page rendered.
- Removed the throwaway scan/cache executable and disposable HTTP-smoke container/data after successful verification. Local Crystal and Rust comparison containers remain running.

## Feature routes and scan/media ownership

- `routes/admin/mod.rs` is composition only; dashboard, users, cache controls, and maintenance have explicit owners. Ordinary reading progress and catalog metadata/media mutations live in their feature modules, regardless of the handler's authorization requirement.
- `routes/api/mod.rs` composes catalog, reading, metadata, media, and tags. Server registration and OpenAPI declarations use feature paths directly instead of flat handler re-export hubs.
- `library/snapshot.rs` owns library snapshots and lookups. `library/scan/` owns discovery, filesystem fingerprints, ID reconciliation, unavailable records, incremental publication, and scheduling. `library/media/` owns archive extraction, supported formats, and thumbnail processing/persistence.
- Removed old `manager.rs`, monolithic route `admin.rs`/`api.rs`, model scan/media methods, and an unused thumbnail-save method. No compatibility paths remain.
- Verification: strict all-target Clippy and 64 Rust tests passed. An isolated running binary matched the prior Docker image's smoke outcomes for nested catalog responses, page bytes, generated/reused thumbnail bytes, dimensions, progress, metadata, sorting, tags, HTML reader/admin/cache pages, OPDS, reference documentation, and stable IDs after rescanning.
