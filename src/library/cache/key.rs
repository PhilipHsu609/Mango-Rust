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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_titles_key_uniqueness() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let key1 = sorted_titles_key("user1", &ids, "name", true);
        let key2 = sorted_titles_key("user2", &ids, "name", true); // Different user
        let key3 = sorted_titles_key("user1", &ids, "mtime", true); // Different sort
        let key4 = sorted_titles_key("user1", &ids, "name", false); // Different order

        assert_ne!(key1, key2, "Different users should produce different keys");
        assert_ne!(
            key1, key3,
            "Different sort methods should produce different keys"
        );
        assert_ne!(
            key1, key4,
            "Different sort order should produce different keys"
        );
    }
}
