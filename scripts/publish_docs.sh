#!/bin/bash
set -e

echo "📄 Publishing docs to gh-pages..."

PROJECT_ROOT=$(git rev-parse --show-toplevel)
DOCS_SOURCE="$PROJECT_ROOT/docs"
README_SOURCE="$PROJECT_ROOT/README.md"

if [ ! -d "$DOCS_SOURCE" ]; then
  echo "❌ Docs directory not found at $DOCS_SOURCE"
  exit 1
fi

if [ ! -f "$README_SOURCE" ]; then
  echo "❌ README.md not found at $README_SOURCE"
  exit 1
fi

TMP_DIR=$(mktemp -d)
cp -r "$DOCS_SOURCE" "$TMP_DIR/"
cp "$README_SOURCE" "$TMP_DIR/index.md"

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
git checkout gh-pages

git rm -rf . > /dev/null 2>&1 || true
cp -r "$TMP_DIR/docs/"* .
cp "$TMP_DIR/index.md" .
touch .nojekyll

git add .
git commit -m "🚀 Publish updated docs"
git push origin gh-pages

git checkout "$CURRENT_BRANCH"
echo "✅ Docs published and returned to $CURRENT_BRANCH."
