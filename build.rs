fn main() {
    // Enable SQLx offline mode by default
    // This uses cached query data from .sqlx/ directory
    // Set SQLX_OFFLINE=false to use live database verification
    if std::env::var("SQLX_OFFLINE").is_err() {
        println!("cargo:rustc-env=SQLX_OFFLINE=true");
    }
}
