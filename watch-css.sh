#!/bin/bash
# Watch LESS files and auto-recompile on changes

echo "Watching LESS files for changes..."
echo "Press Ctrl+C to stop"

mkdir -p static/dist/css

lessc \
  static/src/css/main.less \
  static/dist/css/main.css \
  --compress \
  --source-map \
  --watch
