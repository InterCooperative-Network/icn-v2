#!/bin/bash
# Clean up any existing containers to start fresh
echo "Cleaning up any existing federation nodes..."
docker-compose down -v 2>/dev/null || true
rm -rf node_data_* 2>/dev/null || true
echo "Environment reset complete"
