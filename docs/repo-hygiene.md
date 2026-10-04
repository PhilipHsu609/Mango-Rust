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

## Metadata, ordering, reading, and browser ownership

- `library/metadata/` owns the `TitleInfo` record, private `info.json` file I/O, immutable cached snapshots, and serialized per-path updates. Callers use `Library::metadata()` instead of coordinating persistence and cache reloads themselves. Progress, names, covers, timestamps, and sort preferences share that owner.
- `library/ordering/` owns sort parsing, title comparison, and entry ordering; chapter detection/numeric comparison are private implementation details. Explicit Catalog, Book, Reader, and Continuation profiles preserve automatic ordering, progress precision/clamping, and date-added fallback differences. Snapshot time ties remain stable in both directions.
- `library/reading/` owns continuation selection, start-reading eligibility, recent-entry grouping, and explicitly named fraction/percentage calculations. The API's truncate-before-sort continuation behavior remains distinct from the home feed's sort-before-limit behavior. Existing reading/grouping regressions moved with their policy.
- `Title` and `Entry` retain structural data and tree traversal, not persistence or reading selection. Removed unused model sorting methods and generic sorting/progress traits.
- `routes/pages/` groups home feeds, library/tag browsing, account pages, and shared card presentation. Routes retain transport DTOs and rendering; `routes/presentation.rs` owns navigation/template errors, and catalog query parameters live with the catalog pages. Removed the browser catch-all `main.rs` and empty root `util.rs`; no compatibility aliases remain.
- Verification: strict all-target Clippy and all 72 Rust tests passed. The rebuilt debug server passed the disposable HTTP smoke scenario for nested catalog/depth/slim/progress contracts, generated/reused cover bytes, dimensions, metadata mutations, sorting, tags, HTML reader/admin/cache pages, OPDS/reference pages, and stable IDs after rescanning. Its cover SHA-256 and scenario results match the pre-cutover Docker baseline.
- Final Docker verification: rebuilt `mango-rust:local`, restarted both applications with `local-dev/run-local.sh`, and ran the same disposable smoke scenario against the image. Added runtime checks for start-reading eligibility transitions and recursive whole-title read/unread updates; debug and release results matched.
- Browser comparison exercised home feeds, library navigation/name sorting, and matching book-entry modals in both applications. Covers, ordering controls, entry names, page counts, and modal actions rendered. Existing comparison data contains different progress, so badges and feed membership differ. Rust's `/change-password` page rendered; the same Crystal URL returned 404, so no account-page parity is claimed.
- Removed the disposable smoke script/container/data and released browser tabs after verification. The rebuilt local comparison applications remain running.

## Rust test hygiene

- Removed incidental constructor/default assertions, asserted-copy route DTO serialization, duplicate facade/statistics checks, and tests pinned to private cache keys, pointer identity, or chapter-rounding representation.
- Ordering tests exercise the public title/entry ordering interfaces; cache tests verify user/sort/input isolation and observable invalidation. Metadata tests retain persistence failure, immutable snapshot behavior, external edits, corruption recovery, and concurrent updates without requiring a particular allocation.
- Scan tests now separate rescan transitions from incremental publication and verify ID preservation with competing filesystem paths. Structural title fixtures use title/page vocabulary rather than continuation-specific setup.
- Kept real protocol, precision, capacity/eviction, grouping-boundary, authentication/configuration-precedence, persistence, and scan-stability coverage. No production behavior or interfaces changed.
- Verification: strict all-target Clippy and all 66 Rust tests passed.

## HTTP test hygiene

- Split the administrative catch-all into users, metadata, and maintenance suites; separated sorting and tags from catalog tests. Shared typed catalog helpers resolve explicitly named fixtures instead of whichever title happens to appear first.
- Removed generated-schema/ReDoc wiring, branding/description/help wording, fractional-timing formatting, duplicate success checks, and nonempty-only fixture assertions. Kept actual HTTP contracts, authentication precedence, state transitions, input boundaries, persistent metadata, ordering, downloads, and HTML escaping.
- Suite setup automatically creates seven deterministic five-entry titles with valid ten-page PNG archives, seeds users through the actual CLI, and waits for the initial scan. No manual fixture step or `HOME` changes remain.
- Replaced singleton `cargo run` lifecycle handling with separately owned actual-binary server handles, private loopback ports, temporary configuration/database/upload paths, bounded startup diagnostics, and awaited teardown. CLI mutations, authentication modes, and cover uploads use private servers; shared mutations restore their state.
- Removed direct SQLite/bcrypt user seeding and its four dependencies. CLI tests verify renamed credentials and roles through actual HTTP authentication. OPDS checks complete fixture/feed membership and a real ZIP acquisition without repeatedly requesting the same feed shape for seven equivalent titles.
- `tests/README.md` describes the automatic workflow. CI now runs Rust tests, TypeScript checking, and HTTP tests on Node 22; the fixture's `zip` dependency is explicit.
- Verification: TypeScript checking passed; all 73 HTTP/CLI integration tests in 13 files passed on both the host Node 24 runtime and a clean installation using Node 22 from the repository root. The focused OPDS suite passed again after removing redundant navigation requests (3.6 seconds instead of 10.4 seconds). An actual-binary lifecycle smoke verified independent servers, closed-endpoint refusal, continued usability of the other server, failed-start rejection, and owned configuration cleanup. No suite-owned temporary server/library directories remained after teardown.
