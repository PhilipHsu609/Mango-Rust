# Mango-Rust Caching System

Mango-Rust implements a two-tier caching system to optimize performance for large manga libraries:

1. **Library Cache File** - Persistent disk cache for library structure
2. **LRU Cache** - In-memory runtime cache for computed sorted lists

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
├─────────────────────────────────────────────────────────────┤
│                    Cache Facade (Cache)                     │
├──────────────────────────┬──────────────────────────────────┤
│   LRU Cache (In-Memory)  │  Library Cache File (Disk)      │
│   - Sorted lists         │  - Full library structure       │
│   - Automatic eviction   │  - MessagePack + gzip           │
│   - SHA256 cache keys    │  - Atomic writes                │
└──────────────────────────┴──────────────────────────────────┘
```

## Library Cache File

### Purpose
- Eliminates slow filesystem scans on startup
- Preserves library structure between restarts
- Validates against database title count

### Format
- **Serialization**: MessagePack (via `rmp-serde`)
- **Compression**: gzip (via `flate2`)
- **Location**: Configured via `library_cache_path` (default: `./mango_cache.bin`)

### File Structure
```rust
struct CachedLibraryData {
    path: PathBuf,              // Library root directory
    titles: HashMap<String, Title>,  // All titles with entries
}
```

### Validation
On load, the cache validates:
1. **Path match**: Cache path must match configured library path
2. **Title count**: Cached title count must equal database count

If validation fails, the cache is deleted and a fresh scan occurs.

### Atomic Writes
Cache saves use atomic write pattern:
1. Write to temporary file (`.tmp` extension)
2. Set restrictive permissions (0600 on Unix)
3. Rename temp file to target path (atomic operation)

This ensures the cache file is never corrupt, even if the process crashes during save.

## LRU Cache

### Purpose
- Caches computed sorted lists to avoid repeated sorting
- Automatic eviction based on memory limits
- Thread-safe with minimal lock contention

### Configuration
```toml
[cache]
cache_enabled = true
cache_size_mbs = 100
cache_log_enabled = false  # Enable for debugging
```

### Cache Keys
Cache keys use SHA256 hashing for deterministic, collision-resistant keys:

**Format**: `{namespace}:{username}:{hash}:{sort}:{ascending}`

**Examples**:
- `sorted_titles:admin:a3f2c1...:name:true`
- `sorted_entries:title123:admin:b4e9d8...:modified:false`
- `progress_sum:title123:admin:c7a3f2...`

The hash component is a SHA256 digest of all relevant IDs (title IDs or entry IDs), ensuring the cache key changes when library content changes.

### User Isolation
Each user gets separate cache entries. This enables:
- Per-user sorting preferences
- Progress-based sorting (different for each user)
- No cache pollution between users

### Eviction Strategy
When cache size exceeds limit:
1. Sort entries by least recently used (LRU)
2. Evict oldest entries until size is below limit
3. Track eviction count in statistics

### Cache Invalidation

#### Automatic Invalidation
- **Progress updates**: Invalidates all caches for affected user and title
- **Library scan**: Old cache entries naturally expire as keys change

#### Manual Invalidation
- **Clear all**: Removes all cached entries (via debug page or API)
- **Pattern-based**: Invalidates entries matching a prefix pattern

## Cache Debug Page

Access the cache debug page at `/debug/cache` (admin only).

### Statistics Displayed
- **Memory Usage**: Current size vs configured limit (with progress bar)
- **Hit Rate**: Percentage of cache hits vs misses
- **Entry Count**: Number of cached items
- **Eviction Count**: Total evictions since startup
- **Library Cache File**: File size, path, modification time

### Operations

#### Refresh Statistics
Reloads current cache statistics without modifying cache state.

#### Save Library to Cache
Manually triggers library cache file save. Useful for:
- Preserving current state before shutdown
- Testing cache save functionality

#### Load Library from Cache
Reloads library from cache file, replacing in-memory library. Useful for:
- Testing cache load functionality
- Restoring library without full scan

#### Clear All Cache
Removes all LRU cache entries. Does **not** delete the library cache file.

**Warning**: Clearing cache causes performance degradation until cache warms up.

#### Invalidate by Pattern
Advanced operation: Invalidates cache entries matching a prefix pattern.

**Examples**:
- `sorted_titles:user1:` - Invalidate all title sorts for user1
- `sorted_entries:title123:` - Invalidate all entry sorts for title123
- `progress_sum:` - Invalidate all progress sum caches

## Performance Impact

### Startup Performance
- **Without cache**: Full library scan (slow for large libraries)
- **With cache**: Instant load if cache valid

**Example**: 1000 titles, 10,000 entries
- Cold start (no cache): ~5-10 seconds
- Warm start (cache hit): ~100ms

### Sorting Performance
- **First sort**: Computed and cached (~50-200ms for 1000 titles)
- **Cached sort**: Near-instant (~1-5ms)

### Memory Usage
Configure `cache_size_mbs` based on library size:
- **Small library** (< 100 titles): 10-20 MB
- **Medium library** (100-1000 titles): 50-100 MB
- **Large library** (1000+ titles): 100-200 MB

## API Endpoints

### GET /debug/cache
Renders cache debug page (admin only)

### POST /api/cache/clear
Clears all LRU cache entries

**Response**:
```json
{
  "success": true,
  "message": "Cache cleared successfully",
  "entries_remaining": 0
}
```

### POST /api/cache/save-library
Saves library to cache file

**Response**:
```json
{
  "success": true,
  "message": "Library cache saved successfully"
}
```

### POST /api/cache/load-library
Loads library from cache file

**Response** (success):
```json
{
  "success": true,
  "message": "Library loaded from cache successfully",
  "titles": 523,
  "entries": 4892
}
```

**Response** (no cache):
```json
{
  "success": false,
  "message": "No valid cache file found"
}
```

### POST /api/cache/invalidate
Invalidates cache entries by pattern

**Request**:
```json
{
  "pattern": "sorted_titles:user1:"
}
```

**Response**:
```json
{
  "success": true,
  "message": "Invalidated 15 cache entries",
  "count": 15
}
```

## Implementation Details

### Thread Safety
- **Library**: `Arc<RwLock<Library>>` - Multiple readers, single writer
- **Cache**: `Mutex<Cache>` - Interior mutability for cache operations
- **LRU Cache**: Internal mutex for thread-safe access tracking

### Lock Ordering
To prevent deadlocks, always acquire locks in this order:
1. Library read/write lock
2. Cache mutex
3. LRU cache internal mutex (automatic)

**Example**:
```rust
let lib = state.library.read().await;  // 1. Library lock
let cache = lib.cache().lock().await;   // 2. Cache lock
cache.get_sorted_titles(&key);          // 3. LRU lock (internal)
```

### Background Operations
Library cache saves run in background tasks (`tokio::spawn`) to avoid blocking requests.

**Note**: The save may complete after the request returns. Check logs for confirmation.

## Configuration Reference

```toml
# Library cache file location
library_cache_path = "./mango_cache.bin"

# Enable/disable caching system
cache_enabled = true

# LRU cache size limit (megabytes)
cache_size_mbs = 100

# Enable detailed cache operation logging
cache_log_enabled = false
```

## Troubleshooting

### Cache not loading on startup
**Symptoms**: Library scans on every startup despite cache file existing

**Possible causes**:
1. Title count mismatch (library changed)
2. Path mismatch (library_path changed)
3. Corrupt cache file

**Solution**: Check logs for validation failure reason. Cache will auto-regenerate.

### High memory usage
**Symptoms**: Application using more memory than expected

**Possible causes**:
1. `cache_size_mbs` set too high
2. Many users with many cached sorts

**Solution**: Reduce `cache_size_mbs` or clear cache via debug page.

### Poor cache hit rate
**Symptoms**: Low hit rate percentage in debug page

**Possible causes**:
1. Library frequently changing (scans, uploads)
2. Users using different sort orders frequently
3. Cache size too small (frequent evictions)

**Solution**:
- Increase `cache_size_mbs`
- Check eviction_count - if high, cache is too small
- Enable `cache_log_enabled` to see cache operations

### Cache file permissions error
**Symptoms**: Permission denied errors when saving cache

**Solution**: Ensure application has write permission to `library_cache_path` directory.

## Monitoring

### Log Messages
Enable cache logging for detailed operation tracking:
```toml
cache_log_enabled = true
log_level = "debug"
```

**Example log output**:
```
INFO Library loaded from cache: 523 titles, 4892 entries
DEBUG Cache hit: sorted_titles:admin:a3f2c1...:name:true
DEBUG Cache miss: sorted_entries:title123:admin:b4e9d8...:modified:false
INFO Library cache saved successfully in background
```

### Metrics
Monitor cache performance via debug page:
- **Hit rate** > 80%: Excellent
- **Hit rate** 50-80%: Good
- **Hit rate** < 50%: Consider tuning

## Best Practices

1. **Set appropriate cache size**: Start with 100 MB, adjust based on hit rate and evictions
2. **Enable cache file**: Always enable for production to improve startup time
3. **Monitor hit rate**: Aim for > 80% hit rate for optimal performance
4. **Background saves**: Let cache saves happen in background, don't wait for them
5. **Invalidation strategy**: Only clear cache when necessary (library structure changes)

## Future Enhancements

Potential improvements for future versions:
- **TTL-based expiration**: Auto-expire old cache entries
- **Partial invalidation**: Invalidate only affected entries on library changes
- **Compression**: Compress LRU cache entries in memory
- **Persistence**: Persist LRU cache across restarts
- **Metrics export**: Prometheus-compatible metrics endpoint
