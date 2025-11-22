#!/bin/bash
# Build CSS from LESS sources for production

set -e

echo "Building CSS..."
mkdir -p static/dist/css

lessc \
  static/src/css/main.less \
  static/dist/css/main.css \
  --compress \
  --source-map

echo "✓ CSS compiled to static/dist/css/main.css"
echo "✓ CSS minified and source map generated"
