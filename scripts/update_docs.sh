#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
WORKSPACE_ROOT=$(realpath "$SCRIPT_DIR/..")

echo "Updating generated documentation..."

# Ensure we are in the workspace root for cargo and mkdocs commands
cd "$WORKSPACE_ROOT"

echo "Running gen-crate-readmes..."
cargo run --package gen-crate-readmes --quiet

echo "Running gen-crate-map..."
cargo run --package gen-crate-map --quiet > docs/generated/crate_map.md

echo "Building MkDocs site..."
mkdocs build --strict

echo "Documentation update complete." 