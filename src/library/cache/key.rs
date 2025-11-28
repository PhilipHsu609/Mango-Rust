// Cache Key Generation - deterministic keys for different cacheable types

use sha2::{Digest, Sha256};
use std::path::Path;

// Key prefixes for different cache types
const SORTED_TITLES_PREFIX: &str = "sorted_titles:";
const SORTED_ENTRIES_PREFIX: &str = "sorted_entries:";
const PROGRESS_SUM_PREFIX: &str = "progress_sum:";
const INFO_JSON_PREFIX: &str = "info_json:";

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
    // Create signature from title IDs (order matters for validation)
    let ids_signature = title_ids.join(",");
    let data = format!(
        "{}:{}:{}:{}",
        username, ids_signature, sort_method, ascending
    );
    hash_key(SORTED_TITLES_PREFIX, &data)
}

/// Generate cache key for sorted entries
/// Includes title context, username, and all sort parameters
pub fn sorted_entries_key(
    title_id: &str,
    username: &str,
    entry_ids: &[String],
    sort_method: &str,
    ascending: bool,
) -> String {
    // Create signature from entry IDs (order matters for validation)
    let ids_signature = entry_ids.join(",");
    let data = format!(
        "{}:{}:{}:{}:{}",
        title_id, username, ids_signature, sort_method, ascending
    );
    hash_key(SORTED_ENTRIES_PREFIX, &data)
}

/// Generate cache key for progress sum
/// Includes entry signature to detect when entries have changed
pub fn progress_sum_key(title_id: &str, username: &str, entry_signature: &str) -> String {
    let data = format!("{}:{}:{}", title_id, username, entry_signature);
    hash_key(PROGRESS_SUM_PREFIX, &data)
}

/// Generate cache key for info.json metadata
/// Uses directory path as unique identifier
pub fn info_json_key(dir_path: &Path) -> String {
    let path_str = dir_path.to_string_lossy();
    hash_key(INFO_JSON_PREFIX, &path_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_titles_key_determinism() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let key1 = sorted_titles_key("user1", &ids, "name", true);
        let key2 = sorted_titles_key("user1", &ids, "name", true);
        assert_eq!(key1, key2, "Same inputs should produce same key");
    }

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

    #[test]
    fn test_sorted_titles_key_username_isolation() {
        let ids = vec!["id1".to_string()];
        let key_user1 = sorted_titles_key("user1", &ids, "name", true);
        let key_user2 = sorted_titles_key("user2", &ids, "name", true);
        assert_ne!(
            key_user1, key_user2,
            "Different users should have isolated caches"
        );
    }

    #[test]
    fn test_sorted_entries_key_determinism() {
        let ids = vec!["entry1".to_string(), "entry2".to_string()];
        let key1 = sorted_entries_key("title1", "user1", &ids, "name", true);
        let key2 = sorted_entries_key("title1", "user1", &ids, "name", true);
        assert_eq!(key1, key2, "Same inputs should produce same key");
    }

    #[test]
    fn test_sorted_entries_key_uniqueness() {
        let ids = vec!["entry1".to_string()];
        let key1 = sorted_entries_key("title1", "user1", &ids, "name", true);
        let key2 = sorted_entries_key("title2", "user1", &ids, "name", true); // Different title
        let key3 = sorted_entries_key("title1", "user2", &ids, "name", true); // Different user

        assert_ne!(key1, key2, "Different titles should produce different keys");
        assert_ne!(key1, key3, "Different users should produce different keys");
    }

    #[test]
    fn test_progress_sum_key_determinism() {
        let key1 = progress_sum_key("title1", "user1", "sig123");
        let key2 = progress_sum_key("title1", "user1", "sig123");
        assert_eq!(key1, key2, "Same inputs should produce same key");
    }

    #[test]
    fn test_progress_sum_key_signature_change() {
        let key1 = progress_sum_key("title1", "user1", "sig123");
        let key2 = progress_sum_key("title1", "user1", "sig456"); // Different signature
        assert_ne!(
            key1, key2,
            "Different entry signatures should produce different keys"
        );
    }

    #[test]
    fn test_info_json_key_determinism() {
        let path = Path::new("/path/to/manga");
        let key1 = info_json_key(path);
        let key2 = info_json_key(path);
        assert_eq!(key1, key2, "Same path should produce same key");
    }

    #[test]
    fn test_info_json_key_uniqueness() {
        let path1 = Path::new("/path/to/manga1");
        let path2 = Path::new("/path/to/manga2");
        let key1 = info_json_key(path1);
        let key2 = info_json_key(path2);
        assert_ne!(key1, key2, "Different paths should produce different keys");
    }

    #[test]
    fn test_key_prefixes() {
        let ids = vec!["id1".to_string()];
        let titles_key = sorted_titles_key("user", &ids, "name", true);
        let entries_key = sorted_entries_key("title", "user", &ids, "name", true);
        let progress_key = progress_sum_key("title", "user", "sig");
        let info_key = info_json_key(Path::new("/path"));

        assert!(
            titles_key.starts_with(SORTED_TITLES_PREFIX),
            "Titles key should have correct prefix"
        );
        assert!(
            entries_key.starts_with(SORTED_ENTRIES_PREFIX),
            "Entries key should have correct prefix"
        );
        assert!(
            progress_key.starts_with(PROGRESS_SUM_PREFIX),
            "Progress key should have correct prefix"
        );
        assert!(
            info_key.starts_with(INFO_JSON_PREFIX),
            "Info key should have correct prefix"
        );
    }
}
