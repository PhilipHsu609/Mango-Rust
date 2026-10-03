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
