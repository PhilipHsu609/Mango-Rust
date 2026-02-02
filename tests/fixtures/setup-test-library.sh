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

echo "✓ Created ${#titles[@]} manga titles (ZIP format)"

# Create a 7z test title (for testing 7z/cb7 support)
if command -v 7z &> /dev/null; then
  echo "📦 Creating 7z test title..."
  title_7z="Test Manga 7z Format"
  title_dir_7z="$TEST_LIBRARY_DIR/$title_7z"
  mkdir -p "$title_dir_7z"

  for chapter in {1..3}; do
    chapter_name="Vol.$(printf "%02d" $chapter)"
    work_dir="$TEMP_DIR/$chapter_name"
    mkdir -p "$work_dir"

    # Create 5 minimal images
    for page in {1..5}; do
      printf '\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0a\x49\x44\x41\x54\x78\x9c\x63\x00\x01\x00\x00\x05\x00\x01\x0d\x0a\x2d\xb4\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82' > "$work_dir/$(printf "%03d" $page).png"
    done

    # Create 7z file (using cb7 extension for comic book format)
    cb7_file="$title_dir_7z/$title_7z $chapter_name.cb7"
    (cd "$work_dir" && 7z a -t7z "$cb7_file" . > /dev/null)

    rm -rf "$work_dir"
  done
  echo "✓ Created 7z test title with 3 volumes"
else
  echo "⚠ 7z not available - skipping 7z test title"
fi

# Note: RAR/CBR test fixtures require the proprietary 'rar' command
# which is not commonly available. RAR reading is tested manually or
# by downloading sample CBR files.

echo ""
echo "📊 Summary:"
echo "✓ ${#titles[@]} ZIP titles with 5 volumes each (10 pages per volume)"
echo "✓ 1 7z/CB7 title with 3 volumes (5 pages per volume) - if 7z available"
echo "✓ Total ZIP files: $((${#titles[@]} * 5))"
echo "✓ Test library ready at: $TEST_LIBRARY_DIR"
