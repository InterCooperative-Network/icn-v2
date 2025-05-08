#!/bin/bash

# Run the storage tests with full output
cd "$(dirname "$0")/.."
echo "Running storage tests..."
cargo test -p icn-storage -- --nocapture 