#!/bin/bash
# Create minimal test manga library for CI/testing
# This creates ZIP files containing manga pages

set -e

TEST_LIBRARY_DIR="${HOME}/test-manga-library"

echo "📚 Setting up test manga library at: $TEST_LIBRARY_DIR"

# Clean up any existing test library
rm -rf "$TEST_LIBRARY_DIR"
mkdir -p "$TEST_LIBRARY_DIR"

# Create 7 test manga titles with ZIP entries (matches test expectations)
declare -a titles=(
  "Test Manga Alpha"
  "Test Manga Beta"
  "Test Manga Charlie"
  "Test Manga Delta"
  "Test Manga Echo"
  "Test Manga Foxtrot"
  "Test Manga Golf"
)

# Create a temporary directory for building ZIPs
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

for title in "${titles[@]}"; do
  title_dir="$TEST_LIBRARY_DIR/$title"
  mkdir -p "$title_dir"

  # Create 5 chapters/volumes for each title
  for chapter in {1..5}; do
    chapter_name="Vol.$(printf "%02d" $chapter)"
    work_dir="$TEMP_DIR/$chapter_name"
    mkdir -p "$work_dir"

    # Create 10 minimal images per chapter (1x1 PNG)
    for page in {1..10}; do
      # Create a minimal 1x1 transparent PNG (67 bytes)
      printf '\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0a\x49\x44\x41\x54\x78\x9c\x63\x00\x01\x00\x00\x05\x00\x01\x0d\x0a\x2d\xb4\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82' > "$work_dir/$(printf "%03d" $page).png"
    done

    # Create ZIP file
    zip_file="$title_dir/$title $chapter_name.zip"
    (cd "$work_dir" && zip -q -r "$zip_file" .)

    # Clean up work directory
    rm -rf "$work_dir"
  done
done

echo "✓ Created ${#titles[@]} manga titles"
echo "✓ Each with 5 volumes (ZIP files) and 10 pages per volume"
echo "✓ Total: $((${#titles[@]} * 5)) ZIP files, $((${#titles[@]} * 5 * 10)) pages"
echo "✓ Test library ready at: $TEST_LIBRARY_DIR"
