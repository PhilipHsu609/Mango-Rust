#!/usr/bin/env bash
set -euo pipefail

# The caller owns this fresh destination. Never replace an existing library.
destination="${1:?Usage: setup-test-library.sh NEW_LIBRARY_PATH}"
mkdir -- "$destination"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Valid 2x2 RGB PNG, also suitable for thumbnail and dimension checks.
pixel='iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4z8AARAwQCgAf7gP9i18U1AAAAABJRU5ErkJggg=='
for page in {1..10}; do
  printf '%s' "$pixel" | base64 --decode > "$work/$(printf '%03d' "$page").png"
done

for name in Alpha Beta Charlie Delta Echo Foxtrot Golf; do
  title="Test Manga $name"
  mkdir -- "$destination/$title"
  for number in {1..5}; do
    volume="$(printf 'Vol.%02d' "$number")"
    zip -q -j "$destination/$title/$title $volume.zip" "$work"/*.png
  done
done
