use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::{sqlite::SqlitePool, Row};
use uuid::Uuid;

use crate::error::{Error, Result};

/// A missing title or entry record returned by the admin API.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MissingItem {
    pub id: String,
    pub path: String,
    pub signature: Option<String>,
}

/// Stored page dimension data (from database cache)
#[derive(Debug, Clone)]
pub struct StoredDimension {
    pub page_num: usize,
    pub width: u32,
    pub height: u32,
}

/// Database storage layer - handles user authentication and data persistence
/// Matches original Mango's Storage class functionality
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    /// Initialize storage and run migrations
    pub async fn new(database_url: &str) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(path) = database_url.strip_prefix("sqlite://") {
            // Handle both sqlite://path and sqlite:///path (triple slash for absolute paths)
            let path = path.trim_start_matches('/');
            let path = if !path.starts_with('/') {
                format!("/{}", path)
            } else {
                path.to_string()
            };
            if let Some(parent) = std::path::Path::new(&path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Configure connection pool for better concurrency
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(database_url)?
            .busy_timeout(std::time::Duration::from_secs(30)) // Wait up to 30s for locks
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal) // Use WAL mode for better concurrency
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal); // Balance between safety and performance

        // Connect to database with optimized pool settings
        let pool = SqlitePoolOptions::new()
            .max_connections(20) // Support up to 20 concurrent connections for parallel scanning
            .min_connections(3) // Keep 3 connections warm
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect_with(options)
            .await?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| Error::Internal(format!("Migration failed: {}", e)))?;

        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await?;

        let storage = Self { pool };

        // Initialize admin user if no users exist (matches original behavior)
        storage.init_admin_if_needed().await?;

        Ok(storage)
    }

    /// Create initial admin user with random password if no users exist
    /// Matches original Mango's init_admin macro
    async fn init_admin_if_needed(&self) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        if count == 0 {
            let random_password = generate_random_password();
            let password_hash = hash_password(&random_password)?;

            sqlx::query(
                "INSERT INTO users (username, password, token, admin) VALUES (?, ?, NULL, 1)",
            )
            .bind("admin")
            .bind(&password_hash)
            .execute(&self.pool)
            .await?;

            tracing::warn!("═══════════════════════════════════════════════════════════");
            tracing::warn!("Initial admin user created!");
            tracing::warn!("Username: admin");
            tracing::warn!("Password: {}", random_password);
            tracing::warn!("Please change this password immediately after first login!");
            tracing::warn!("═══════════════════════════════════════════════════════════");
        }

        Ok(())
    }

    /// Verify username and password, return session token on success
    /// Matches original Storage#verify_user
    pub async fn verify_user(&self, username: &str, password: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT password, token FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let password_hash: String = row.get("password");

            // Verify password
            if !verify_password(password, &password_hash)? {
                tracing::debug!("Password verification failed for user: {}", username);
                return Ok(None);
            }

            tracing::debug!("User {} verified successfully", username);

            // Return existing token or generate new one
            let token: Option<String> = row.get("token");
            if let Some(existing_token) = token {
                return Ok(Some(existing_token));
            }

            // Generate new token
            let new_token = Uuid::new_v4().to_string();
            sqlx::query("UPDATE users SET token = ? WHERE username = ?")
                .bind(&new_token)
                .bind(username)
                .execute(&self.pool)
                .await?;

            Ok(Some(new_token))
        } else {
            tracing::debug!("User not found: {}", username);
            Ok(None)
        }
    }

    /// Verify session token, return username on success
    /// Matches original Storage#verify_token
    pub async fn verify_token(&self, token: &str) -> Result<Option<String>> {
        let username: Option<String> =
            sqlx::query_scalar("SELECT username FROM users WHERE token = ?")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;

        Ok(username)
    }

    /// Check if user is admin
    /// Matches original Storage#verify_admin
    pub async fn verify_admin(&self, token: &str) -> Result<bool> {
        let admin: Option<i32> = sqlx::query_scalar("SELECT admin FROM users WHERE token = ?")
            .bind(token)
            .fetch_optional(&self.pool)
            .await?;

        Ok(admin.map(|a| a == 1).unwrap_or(false))
    }

    /// Check if username exists
    /// Matches original Storage#username_exists
    pub async fn username_exists(&self, username: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(&self.pool)
            .await?;

        Ok(count > 0)
    }

    /// Check if user is admin by username
    /// Matches original Storage#username_is_admin
    pub async fn username_is_admin(&self, username: &str) -> Result<bool> {
        let admin: Option<i32> = sqlx::query_scalar("SELECT admin FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        Ok(admin.map(|a| a == 1).unwrap_or(false))
    }

    /// Alias for username_is_admin
    pub async fn is_admin(&self, username: &str) -> Result<bool> {
        self.username_is_admin(username).await
    }

    /// Create a new user
    /// Matches original Storage#new_user
    pub async fn create_user(&self, username: &str, password: &str, is_admin: bool) -> Result<()> {
        let password_hash = hash_password(password)?;
        let admin_flag = if is_admin { 1 } else { 0 };

        sqlx::query("INSERT INTO users (username, password, token, admin) VALUES (?, ?, NULL, ?)")
            .bind(username)
            .bind(&password_hash)
            .bind(admin_flag)
            .execute(&self.pool)
            .await?;

        tracing::info!("Created user: {} (admin: {})", username, is_admin);
        Ok(())
    }

    /// Update user information
    /// Matches original Storage#update_user
    pub async fn update_user(
        &self,
        original_username: &str,
        new_username: &str,
        password: Option<&str>,
        is_admin: bool,
    ) -> Result<()> {
        let admin_flag = if is_admin { 1 } else { 0 };

        if let Some(new_password) = password {
            let password_hash = hash_password(new_password)?;
            sqlx::query(
                "UPDATE users SET username = ?, password = ?, admin = ? WHERE username = ?",
            )
            .bind(new_username)
            .bind(&password_hash)
            .bind(admin_flag)
            .bind(original_username)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("UPDATE users SET username = ?, admin = ? WHERE username = ?")
                .bind(new_username)
                .bind(admin_flag)
                .bind(original_username)
                .execute(&self.pool)
                .await?;
        }

        tracing::info!("Updated user: {} -> {}", original_username, new_username);
        Ok(())
    }

    /// Change user's password
    /// Verifies current password before allowing the change
    pub async fn change_password(
        &self,
        username: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<()> {
        // First, get the current password hash
        let row: Option<(String,)> =
            sqlx::query_as("SELECT password FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;

        let current_hash = row
            .ok_or_else(|| Error::BadRequest(format!("User not found: {}", username)))?
            .0;

        // Verify the current password
        if !verify_password(current_password, &current_hash)? {
            return Err(Error::BadRequest(
                "Current password is incorrect".to_string(),
            ));
        }

        // Hash the new password
        let new_hash = hash_password(new_password)?;

        // Update the password
        sqlx::query("UPDATE users SET password = ? WHERE username = ?")
            .bind(&new_hash)
            .bind(username)
            .execute(&self.pool)
            .await?;

        tracing::info!("Password changed for user: {}", username);
        Ok(())
    }

    /// Delete a user
    /// Matches original Storage#delete_user
    pub async fn delete_user(&self, username: &str) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE username = ?")
            .bind(username)
            .execute(&self.pool)
            .await?;

        tracing::info!("Deleted user: {}", username);
        Ok(())
    }

    /// List all users (returns username and admin status)
    /// Matches original Storage#list_users
    pub async fn list_users(&self) -> Result<Vec<(String, bool)>> {
        let rows = sqlx::query("SELECT username, admin FROM users")
            .fetch_all(&self.pool)
            .await?;

        let users = rows
            .into_iter()
            .map(|row| {
                let username: String = row.get("username");
                let admin: i32 = row.get("admin");
                (username, admin == 1)
            })
            .collect();

        Ok(users)
    }

    /// Logout user (clear session token)
    /// Matches original Storage#logout
    pub async fn logout(&self, token: &str) -> Result<()> {
        sqlx::query("UPDATE users SET token = NULL WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get titles marked unavailable because their paths no longer exist.
    pub async fn get_missing_titles(&self) -> Result<Vec<MissingItem>> {
        let rows = sqlx::query("SELECT id, path, signature FROM titles WHERE unavailable = 1")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| MissingItem {
                id: row.get("id"),
                path: row.get("path"),
                signature: row.get("signature"),
            })
            .collect())
    }

    /// Get entries marked unavailable because their paths no longer exist.
    pub async fn get_missing_entries(&self) -> Result<Vec<MissingItem>> {
        let rows = sqlx::query("SELECT id, path, signature FROM ids WHERE unavailable = 1")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| MissingItem {
                id: row.get("id"),
                path: row.get("path"),
                signature: row.get("signature"),
            })
            .collect())
    }

    /// Delete a specific unavailable entry record.
    pub async fn delete_missing_entry(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM ids WHERE id = ? AND unavailable = 1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() > 0 {
            tracing::info!("Deleted missing entry: {}", id);
        }

        Ok(())
    }

    /// Delete a specific unavailable title record.
    pub async fn delete_missing_title(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM titles WHERE id = ? AND unavailable = 1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() > 0 {
            tracing::info!("Deleted missing title: {}", id);
        }

        Ok(())
    }

    /// Delete all titles marked unavailable.
    pub async fn delete_all_missing_titles(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM titles WHERE unavailable = 1")
            .execute(&self.pool)
            .await?;

        let rows_affected = result.rows_affected();
        tracing::info!("Deleted {} missing titles", rows_affected);
        Ok(rows_affected)
    }

    /// Delete all entries marked unavailable.
    pub async fn delete_all_missing_entries(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM ids WHERE unavailable = 1")
            .execute(&self.pool)
            .await?;

        let rows_affected = result.rows_affected();
        tracing::info!("Deleted {} missing entries", rows_affected);
        Ok(rows_affected)
    }

    /// Get count of unavailable (missing) entries
    /// Used for admin dashboard
    pub async fn get_missing_count(&self) -> Result<usize> {
        let title_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM titles WHERE unavailable = 1")
                .fetch_one(&self.pool)
                .await?;

        let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ids WHERE unavailable = 1")
            .fetch_one(&self.pool)
            .await?;

        Ok((title_count + entry_count) as usize)
    }

    // ========== Tags Methods ==========

    /// Get all tags for a specific title
    /// Matches original Storage#get_title_tags
    pub async fn get_title_tags(&self, title_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT tag FROM tags WHERE id = ? ORDER BY tag")
            .bind(title_id)
            .fetch_all(&self.pool)
            .await?;

        let tags = rows.into_iter().map(|row| row.get("tag")).collect();

        Ok(tags)
    }

    /// Get all title IDs that have a specific tag
    /// Matches original Storage#get_tag_titles
    pub async fn get_tag_titles(&self, tag: &str) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT id FROM tags WHERE tag = ?")
            .bind(tag)
            .fetch_all(&self.pool)
            .await?;

        let title_ids = rows.into_iter().map(|row| row.get("id")).collect();

        Ok(title_ids)
    }

    /// List all unique tags
    /// Returns all distinct tag names sorted alphabetically
    pub async fn list_tags(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT tags.tag FROM tags \
             INNER JOIN titles ON tags.id = titles.id \
             WHERE titles.unavailable = 0 \
             ORDER BY tags.tag",
        )
        .fetch_all(&self.pool)
        .await?;

        let tags = rows.into_iter().map(|row| row.get("tag")).collect();

        Ok(tags)
    }

    /// Add a tag to a title
    /// Matches original Storage#add_tag
    pub async fn add_tag(&self, title_id: &str, tag: &str) -> Result<()> {
        sqlx::query("INSERT INTO tags (id, tag) VALUES (?, ?)")
            .bind(title_id)
            .bind(tag)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Delete a tag from a title
    /// Matches original Storage#delete_tag
    pub async fn delete_tag(&self, title_id: &str, tag: &str) -> Result<()> {
        sqlx::query("DELETE FROM tags WHERE id = ? AND tag = ?")
            .bind(title_id)
            .bind(tag)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get database pool for advanced operations
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ========== Display Name / Sort Title Methods ==========

    /// Update display name for a title
    pub async fn update_title_display_name(
        &self,
        title_id: &str,
        display_name: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE titles SET display_name = ? WHERE id = ?")
            .bind(display_name)
            .bind(title_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update display name for an entry
    pub async fn update_entry_display_name(
        &self,
        entry_id: &str,
        display_name: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE ids SET display_name = ? WHERE id = ?")
            .bind(display_name)
            .bind(entry_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update sort title for a title (None clears it)
    pub async fn update_title_sort_title(
        &self,
        title_id: &str,
        sort_title: Option<&str>,
    ) -> Result<()> {
        sqlx::query("UPDATE titles SET sort_title = ? WHERE id = ?")
            .bind(sort_title)
            .bind(title_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update sort title for an entry (None clears it)
    pub async fn update_entry_sort_title(
        &self,
        entry_id: &str,
        sort_title: Option<&str>,
    ) -> Result<()> {
        sqlx::query("UPDATE ids SET sort_title = ? WHERE id = ?")
            .bind(sort_title)
            .bind(entry_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get an optional title sort override.
    pub async fn get_title_sort_title(&self, title_id: &str) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar::<_, Option<String>>("SELECT sort_title FROM titles WHERE id = ?")
                .bind(title_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten(),
        )
    }

    /// Get an optional entry sort override.
    pub async fn get_entry_sort_title(&self, entry_id: &str) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar::<_, Option<String>>("SELECT sort_title FROM ids WHERE id = ?")
                .bind(entry_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten(),
        )
    }

    // ========== Dimensions Cache ==========

    /// Get cached dimensions for an entry
    /// Returns None if not cached (needs extraction)
    pub async fn get_dimensions(&self, entry_id: &str) -> Result<Option<Vec<StoredDimension>>> {
        let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT page_num, width, height FROM dimensions WHERE entry_id = ? ORDER BY page_num",
        )
        .bind(entry_id)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let dims = rows
            .into_iter()
            .map(|(page_num, width, height)| StoredDimension {
                page_num: page_num as usize,
                width: width as u32,
                height: height as u32,
            })
            .collect();

        Ok(Some(dims))
    }

    /// Save dimensions for an entry (replaces existing)
    /// Uses transaction to ensure atomicity
    pub async fn save_dimensions(
        &self,
        entry_id: &str,
        dimensions: &[(usize, u32, u32)],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Delete existing dimensions for this entry
        sqlx::query("DELETE FROM dimensions WHERE entry_id = ?")
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        // Insert new dimensions
        for (page_num, width, height) in dimensions {
            sqlx::query(
                "INSERT INTO dimensions (entry_id, page_num, width, height) VALUES (?, ?, ?, ?)",
            )
            .bind(entry_id)
            .bind(*page_num as i64)
            .bind(*width as i64)
            .bind(*height as i64)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Check if dimensions are cached for an entry
    pub async fn has_dimensions(&self, entry_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dimensions WHERE entry_id = ?")
            .bind(entry_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(count > 0)
    }

    /// Get dimension count for an entry (to check if cache is stale)
    pub async fn get_dimensions_count(&self, entry_id: &str) -> Result<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dimensions WHERE entry_id = ?")
            .bind(entry_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(count as usize)
    }
}

/// Hash a password using bcrypt (matches original Mango's hash_password function)
fn hash_password(password: &str) -> Result<String> {
    hash(password, DEFAULT_COST)
        .map_err(|e| Error::Internal(format!("Password hashing failed: {}", e)))
}

/// Verify a password against a hash (matches original Mango's verify_password function)
fn verify_password(password: &str, hash: &str) -> Result<bool> {
    verify(password, hash)
        .map_err(|e| Error::Internal(format!("Password verification failed: {}", e)))
}

/// Generate a random password for initial admin (matches original random_str behavior)
fn generate_random_password() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                             abcdefghijklmnopqrstuvwxyz\
                             0123456789";
    const PASSWORD_LEN: usize = 12;
    let mut rng = rand::thread_rng();

    (0..PASSWORD_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Storage;

    #[tokio::test]
    async fn missing_titles_and_entries_are_split_and_deleted_independently() {
        let dir = tempfile::tempdir().unwrap();
        let database_path = dir.path().join("test.db");
        std::fs::File::create(&database_path).unwrap();
        let database_url = format!("sqlite://{}", database_path.display());
        let storage = Storage::new(&database_url).await.unwrap();

        sqlx::query("INSERT INTO titles (id, path, signature, unavailable) VALUES (?, ?, ?, 1)")
            .bind("title-id")
            .bind("Series")
            .bind("title-signature")
            .execute(&storage.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ids (id, path, signature, unavailable) VALUES (?, ?, ?, 1)")
            .bind("entry-id")
            .bind("Series/Volume.cbz")
            .bind("entry-signature")
            .execute(&storage.pool)
            .await
            .unwrap();

        let titles = storage.get_missing_titles().await.unwrap();
        let entries = storage.get_missing_entries().await.unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].id, "title-id");
        assert_eq!(titles[0].signature.as_deref(), Some("title-signature"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "entry-id");
        assert_eq!(entries[0].signature.as_deref(), Some("entry-signature"));

        storage.delete_missing_title("title-id").await.unwrap();
        assert!(storage.get_missing_titles().await.unwrap().is_empty());
        assert_eq!(storage.get_missing_entries().await.unwrap().len(), 1);

        assert_eq!(storage.delete_all_missing_entries().await.unwrap(), 1);
        assert!(storage.get_missing_entries().await.unwrap().is_empty());
    }
}
