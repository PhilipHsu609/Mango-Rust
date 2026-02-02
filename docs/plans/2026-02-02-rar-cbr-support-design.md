# RAR/CBR Archive Support Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add RAR/CBR (and 7z/CB7) archive support using libarchive, matching original Mango's backend.

**Architecture:** Replace the `zip` crate with `compress-tools` (libarchive bindings) to provide unified archive handling for all formats. This matches original Mango's approach and gives us RAR, 7z, and other formats with a single code path.

**Tech Stack:** compress-tools crate with tokio feature, libarchive system library

---

## Overview

### Supported Formats After Implementation

| Extension | Format | Status |
|-----------|--------|--------|
| `.zip`, `.cbz` | ZIP | Existing (migrated to libarchive) |
| `.rar`, `.cbr` | RAR | New |
| `.7z`, `.cb7` | 7-Zip | New (bonus) |

### Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Replace `zip` with `compress-tools` |
| `src/library/entry.rs` | Rewrite archive functions |
| `src/library/title.rs` | Update extension filtering |
| `Dockerfile` | Add libarchive dependency |
| `.github/workflows/docker-publish.yml` | Add libarchive to CI |

---

## Implementation Details

### 1. Cargo.toml Changes

```toml
# Remove:
zip = "0.6"

# Add:
compress-tools = { version = "0.15", features = ["tokio"] }
```

### 2. Archive Extension Detection

Update filtering to include all supported formats:

```rust
const ARCHIVE_EXTENSIONS: &[&str] = &[
    ".zip", ".cbz",  // ZIP-based
    ".rar", ".cbr",  // RAR-based
    ".7z", ".cb7",   // 7-Zip based
];

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let ext = format!(".{}", e.to_lowercase());
            ARCHIVE_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}
```

### 3. Entry.rs Function Rewrites

**`extract_image_list()`** - List images in archive:

```rust
use compress_tools::*;
use std::io::Cursor;

async fn extract_image_list(archive_path: &Path) -> Result<Vec<String>> {
    let path = archive_path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let files = list_archive_files(file)?;

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

**`extract_image_from_archive()`** - Extract single image:

```rust
async fn extract_image_from_archive(archive_path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let path = archive_path.to_path_buf();
    let name = image_name.to_string();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut buffer = Vec::new();

        uncompress_archive_file(file, &mut buffer, &name)?;

        Ok(buffer)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}
```

### 4. Dockerfile Changes

```dockerfile
# Build stage
FROM rust:1.75-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    libarchive-dev \
    pkgconfig

# ... rest of build ...

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache \
    libarchive \
    ca-certificates

# ... rest of runtime ...
```

### 5. GitHub Actions CI

Add to `.github/workflows/docker-publish.yml`:

```yaml
- name: Install build dependencies
  run: |
    apk add --no-cache libarchive-dev pkgconfig
```

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Corrupted archive | Log warning, skip entry |
| Password-protected RAR | Return error "Password-protected archives not supported" |
| Multipart RAR (.part01.rar) | libarchive handles automatically |
| Empty archive | Skip entry, log warning |
| Unsupported format | Let libarchive error propagate |

No new error types needed - existing `Error::Internal` and `Error::NotFound` suffice.

---

## Testing

### Test Fixtures

Add to `tests/fixtures/library/`:
- `test-rar/test.cbr` - Simple RAR with 3 test images
- `test-7z/test.cb7` - Simple 7z with 3 test images

### Unit Tests

```rust
#[tokio::test]
async fn test_extract_from_rar() {
    let entry = Entry::from_archive("fixtures/test.cbr".into()).await.unwrap();
    assert_eq!(entry.pages, 3);

    let page = entry.get_page(0).await.unwrap();
    assert!(!page.is_empty());
}

#[tokio::test]
async fn test_extract_from_7z() {
    let entry = Entry::from_archive("fixtures/test.cb7".into()).await.unwrap();
    assert_eq!(entry.pages, 3);
}
```

### Integration Tests

Extend Playwright tests to verify RAR entries render correctly in the reader.

---

## Local Development Setup

| OS | Install command |
|----|-----------------|
| Ubuntu/Debian | `sudo apt install libarchive-dev pkg-config` |
| macOS | `brew install libarchive pkg-config` |
| Arch | `sudo pacman -S libarchive` |
| Alpine | `apk add libarchive-dev pkgconfig` |

---

## Migration

**For existing users:** None required.
- ZIP/CBZ files continue working identically
- RAR/CBR files in library picked up on next scan
- No database changes

---

## v1.0.0 Release Checklist

- [ ] Implement archive abstraction with compress-tools
- [ ] Update Dockerfile with libarchive
- [ ] Update CI workflow
- [ ] Add RAR/7z test fixtures
- [ ] Run full integration test suite
- [ ] Update README with supported formats
- [ ] Manual testing with real-world RAR files
- [ ] Tag v1.0.0 release

---

## Out of Scope (v2.0+)

- Plugin system
- Download queue
- Subscriptions
- OpenAPI documentation
