#!/bin/bash

# Simple health server for demo purposes
# Usage: ./health_server.sh PORT

PORT=${1:-5001}

# Listen on the specified port and respond to health checks
while true; do
  echo -e "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{\"status\":\"healthy\"}" | nc -l -p $PORT
  echo "Health check served on port $PORT"
done 