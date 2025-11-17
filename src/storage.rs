use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::{sqlite::SqlitePool, Row};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Represents a missing (unavailable) database entry
/// Used for displaying and managing items whose files are no longer on disk
#[derive(Debug, Clone, serde::Serialize)]
pub struct MissingEntry {
    pub id: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
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

        // Connect to database
        let pool = SqlitePool::connect(database_url).await?;

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

    /// Get all unavailable (missing) entries
    /// Matches original Storage#get_missing
    pub async fn get_missing_entries(&self) -> Result<Vec<MissingEntry>> {
        let rows =
            sqlx::query("SELECT id, path, type FROM ids WHERE unavailable = 1 ORDER BY type, path")
                .fetch_all(&self.pool)
                .await?;

        let entries = rows
            .into_iter()
            .map(|row| MissingEntry {
                id: row.get("id"),
                path: row.get("path"),
                entry_type: row.get("type"),
            })
            .collect();

        Ok(entries)
    }

    /// Delete a specific missing entry from database
    /// Matches original Storage#delete_missing
    pub async fn delete_missing_entry(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM ids WHERE id = ? AND unavailable = 1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        tracing::info!("Deleted missing entry: {}", id);
        Ok(())
    }

    /// Delete all missing entries from database
    /// Matches original Storage#delete_all_missing (custom implementation)
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
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ids WHERE unavailable = 1")
            .fetch_one(&self.pool)
            .await?;

        Ok(count as usize)
    }

    /// Get database pool for advanced operations
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
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
