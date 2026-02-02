# RAR/CBR Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace `zip` crate with `compress-tools` (libarchive) to support RAR/CBR and 7z/CB7 archives.

**Architecture:** Modify the archive extraction functions in `entry.rs` to use compress-tools instead of the zip crate. Update the supported extensions constant in `util.rs`. Update Dockerfile to include libarchive.

**Tech Stack:** compress-tools 0.15 with tokio feature, libarchive system library

---

## Task 1: Update Cargo.toml Dependencies

**Files:**
- Modify: `Cargo.toml:32-33`

**Step 1: Replace zip with compress-tools**

Open `Cargo.toml` and replace:

```toml
# Archive handling (Tier 1: ZIP only)
zip = "0.6"
```

With:

```toml
# Archive handling (libarchive - supports ZIP, RAR, 7z)
compress-tools = { version = "0.15", features = ["tokio"] }
```

**Step 2: Verify it compiles (will have errors - that's expected)**

Run: `cargo check 2>&1 | head -20`

Expected: Errors about `zip::ZipArchive` not found (confirms old dependency removed)

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: replace zip crate with compress-tools for libarchive support"
```

---

## Task 2: Update Supported Extensions

**Files:**
- Modify: `src/util.rs:105`

**Step 1: Update EXTRACTABLE_ARCHIVE_EXTENSIONS**

In `src/util.rs`, change line 105 from:

```rust
pub const EXTRACTABLE_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz"];
```

To:

```rust
pub const EXTRACTABLE_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr", "7z", "cb7"];
```

**Step 2: Commit**

```bash
git add src/util.rs
git commit -m "feat: add RAR/CBR and 7z/CB7 to supported archive extensions"
```

---

## Task 3: Rewrite extract_image_list Function

**Files:**
- Modify: `src/library/entry.rs:198-225`

**Step 1: Replace the extract_image_list function**

In `src/library/entry.rs`, replace the entire `extract_image_list` function (lines 198-225) with:

```rust
/// Extract list of image filenames from an archive (ZIP, RAR, 7z)
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_list(archive_path: &Path) -> Result<Vec<String>> {
    let path = archive_path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let files = compress_tools::list_archive_files(file)
            .map_err(|e| crate::error::Error::Internal(format!("Failed to list archive: {}", e)))?;

        let mut images: Vec<String> = files
            .into_iter()
            .filter(|name| is_image_file(name))
            .collect();

        // Sort naturally (Chapter 2 before Chapter 10)
        images.sort_by(|a, b| natord::compare(a, b));

        Ok(images)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}
```

**Step 2: Commit**

```bash
git add src/library/entry.rs
git commit -m "refactor: use compress-tools for extract_image_list"
```

---

## Task 4: Rewrite extract_image_from_archive Function

**Files:**
- Modify: `src/library/entry.rs:227-247`

**Step 1: Replace the extract_image_from_archive function**

In `src/library/entry.rs`, replace the entire `extract_image_from_archive` function (lines 227-247, now shifted due to previous edit) with:

```rust
/// Extract a single image from archive (ZIP, RAR, 7z)
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_from_archive(archive_path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let path = archive_path.to_path_buf();
    let name = image_name.to_string();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut buffer = Vec::new();

        compress_tools::uncompress_archive_file(file, &mut buffer, &name)
            .map_err(|e| crate::error::Error::Internal(format!("Failed to extract {}: {}", name, e)))?;

        Ok(buffer)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}
```

**Step 2: Remove old zip import if present**

At the top of `entry.rs`, there should be no `use zip` statement. If there is, remove it.

**Step 3: Verify compilation**

Run: `cargo check`

Expected: Should compile successfully (if libarchive is installed)

**Step 4: Commit**

```bash
git add src/library/entry.rs
git commit -m "refactor: use compress-tools for extract_image_from_archive"
```

---

## Task 5: Update Dockerfile for libarchive

**Files:**
- Modify: `Dockerfile:5` and add runtime dependency

**Step 1: Update build stage dependencies**

In `Dockerfile`, change line 5 from:

```dockerfile
RUN apk add --no-cache musl-dev sqlite-dev sqlite-static nodejs npm
```

To:

```dockerfile
RUN apk add --no-cache musl-dev sqlite-dev sqlite-static nodejs npm libarchive-dev pkgconfig
```

**Step 2: Add runtime dependency**

After line 33 (`FROM alpine:latest`), add:

```dockerfile
# Install libarchive runtime library for archive support
RUN apk add --no-cache libarchive
```

So lines 33-35 become:

```dockerfile
FROM alpine:latest

# Install libarchive runtime library for archive support
RUN apk add --no-cache libarchive

WORKDIR /app
```

**Step 3: Commit**

```bash
git add Dockerfile
git commit -m "build: add libarchive dependency to Dockerfile"
```

---

## Task 6: Create Test Fixtures

**Files:**
- Create: `tests/fixtures/library/test-rar/test.cbr`
- Create: `tests/fixtures/library/test-7z/test.cb7`

**Step 1: Create RAR test fixture**

```bash
# Create directory
mkdir -p tests/fixtures/library/test-rar

# Create test images (simple colored rectangles)
cd tests/fixtures/library/test-rar

# Use ImageMagick to create test images (or copy existing test images)
# If ImageMagick not available, copy from existing test fixture
convert -size 100x100 xc:red page1.jpg 2>/dev/null || cp ../Test\ Title/Test\ Entry.cbz.tmp/1.jpg page1.jpg 2>/dev/null || echo "Need to create test images manually"
convert -size 100x100 xc:green page2.jpg 2>/dev/null || echo "Skipping page2"
convert -size 100x100 xc:blue page3.jpg 2>/dev/null || echo "Skipping page3"

# Create RAR archive (requires rar command)
# If rar not available, download a sample CBR for testing
rar a test.cbr page1.jpg page2.jpg page3.jpg 2>/dev/null || echo "RAR command not available - will need manual fixture"

# Clean up loose images
rm -f page1.jpg page2.jpg page3.jpg

cd /home/philip/workspace/Mango-Rust
```

**Note:** If `rar` command is not available, you can:
1. Download a sample CBR file from the internet for testing
2. Or create the fixture on a system with WinRAR/rar installed

**Step 2: Create 7z test fixture**

```bash
mkdir -p tests/fixtures/library/test-7z

cd tests/fixtures/library/test-7z

# Create test images
convert -size 100x100 xc:yellow page1.jpg 2>/dev/null || echo "Need ImageMagick"
convert -size 100x100 xc:cyan page2.jpg 2>/dev/null || echo "Skipping"
convert -size 100x100 xc:magenta page3.jpg 2>/dev/null || echo "Skipping"

# Create 7z archive
7z a test.cb7 page1.jpg page2.jpg page3.jpg 2>/dev/null || echo "7z command not available"

rm -f page1.jpg page2.jpg page3.jpg

cd /home/philip/workspace/Mango-Rust
```

**Step 3: Commit fixtures (if created)**

```bash
git add tests/fixtures/library/test-rar tests/fixtures/library/test-7z
git commit -m "test: add RAR and 7z test fixtures"
```

---

## Task 7: Run Integration Tests

**Files:**
- None (testing only)

**Step 1: Install libarchive locally (if not already)**

Ubuntu/Debian:
```bash
sudo apt install libarchive-dev pkg-config
```

macOS:
```bash
brew install libarchive pkg-config
```

**Step 2: Build and run tests**

```bash
cargo build
npm --prefix tests test
```

Expected: All existing tests should pass (ZIP still works via libarchive)

**Step 3: Manual verification with RAR file (if fixture available)**

Start server and verify RAR entry appears in library and pages render correctly.

---

## Task 8: Update README

**Files:**
- Modify: `README.md`

**Step 1: Update supported formats section**

Add or update a "Supported Formats" section in README.md:

```markdown
## Supported Archive Formats

Mango-Rust supports the following comic archive formats:

| Extension | Format | Notes |
|-----------|--------|-------|
| `.zip`, `.cbz` | ZIP | Standard comic book archive |
| `.rar`, `.cbr` | RAR | Including RAR5 and multipart |
| `.7z`, `.cb7` | 7-Zip | High compression format |

All formats are handled via libarchive for consistent behavior.
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document supported archive formats"
```

---

## Task 9: Final Verification and Tag

**Files:**
- None

**Step 1: Run full test suite**

```bash
cargo test
npm --prefix tests test
```

**Step 2: Build Docker image locally**

```bash
docker build -t mango-rust:v1.0.0-test .
```

**Step 3: Test Docker image**

```bash
docker run -d -p 9000:9000 -v ~/manga:/root/mango/library mango-rust:v1.0.0-test
# Verify library loads, pages render (including any RAR files if present)
```

**Step 4: Create release commit**

```bash
git add -A
git commit -m "release: v1.0.0 - add RAR/CBR and 7z support"
```

---

## Summary

After completing all tasks:

1. **ZIP/CBZ** - Continue to work (via libarchive now)
2. **RAR/CBR** - Now supported
3. **7z/CB7** - Now supported
4. **Docker** - Includes libarchive runtime
5. **Tests** - All passing

Ready to trigger GitHub Actions workflow with `v1.0.0` version for release.
