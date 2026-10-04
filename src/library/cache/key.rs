// Cache Key Generation - deterministic keys for different cacheable types

use sha2::{Digest, Sha256};

// Key prefixes for different cache types
const SORTED_TITLES_PREFIX: &str = "sorted_titles:";

/// Generate SHA256-based cache key from input data
fn hash_key(prefix: &str, data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    format!("{}{:x}", prefix, result)
}

/// Generate cache key for sorted titles
/// Includes username for user isolation and all sort parameters
pub fn sorted_titles_key(
    username: &str,
    title_ids: &[String],
    sort_method: &str,
    ascending: bool,
) -> String {
    let ids_signature = title_ids.join(",");
    let prefix = format!("{SORTED_TITLES_PREFIX}{username}:");
    let data = format!("{ids_signature}:{sort_method}:{ascending}");
    hash_key(&prefix, &data)
}
