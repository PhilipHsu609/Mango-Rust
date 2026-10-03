# Repository hygiene

Branch: `repo-hygiene`. Preserve behavior and data contracts; use Crystal Mango as a reference when intent is unclear. Commit each verified refactor separately.

## Scan worker ownership

- Each worker owns its `PendingIds` buffers and returns them alongside its completed title. The collector owns the combined buffers and persists them before publishing a snapshot.
- Removed shared mutex-protected ID vectors and per-record locking. Failed workers cannot contribute partially assigned IDs to another worker's publication batch.
- Kept immutable library snapshots, concurrency limits, persistent ID matching, and publication batching unchanged.
- Verification: `cargo test --all-targets` (64 passed across the combined scan, traversal, and cache edits); isolated executable smoke scanned nested image directories, rescanned a changed directory, confirmed stable IDs and published page counts, and checked active SQLite title/entry records.
