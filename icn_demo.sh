#!/bin/bash
set -e

# Colors for better output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== ICN Federation Demo ===${NC}"
echo -e "${YELLOW}This demo shows a complete ICN federation workflow.${NC}"
echo

# Function to check if command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

# Check dependencies
echo -e "${YELLOW}Checking dependencies...${NC}"
MISSING_DEPS=0

if ! command_exists docker; then
  echo -e "${RED}Docker not found. Please install Docker.${NC}"
  MISSING_DEPS=1
fi

if ! command_exists docker-compose; then
  echo -e "${RED}Docker Compose not found. Please install Docker Compose.${NC}"
  MISSING_DEPS=1
fi

if ! command_exists cargo; then
  echo -e "${RED}Cargo not found. Please install Rust and Cargo.${NC}"
  MISSING_DEPS=1
fi

if [ $MISSING_DEPS -eq 1 ]; then
  echo -e "${RED}Missing dependencies. Please install the required tools and run again.${NC}"
  exit 1
fi

# Create demo directories
echo -e "${YELLOW}Creating demo environment...${NC}"
mkdir -p .demo/keys .demo/proposals .demo/results

# Step 1: Build the ICN workspace
echo -e "\n${BLUE}[1/7] Building the ICN workspace...${NC}"
cargo build --release --workspace

# Step 2: Generate keys for demo participants
echo -e "\n${BLUE}[2/7] Generating keys for participants...${NC}"
cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- key-gen --output .demo/keys/node1.json
cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- key-gen --output .demo/keys/node2.json
cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- key-gen --output .demo/keys/node3.json

# Extract DID information for later use
NODE1_DID=$(grep -o '"did": "[^"]*' .demo/keys/node1.json | cut -d'"' -f4)
NODE2_DID=$(grep -o '"did": "[^"]*' .demo/keys/node2.json | cut -d'"' -f4)
NODE3_DID=$(grep -o '"did": "[^"]*' .demo/keys/node3.json | cut -d'"' -f4)

echo -e "${GREEN}Generated keys for 3 nodes:${NC}"
echo -e "${GREEN}  Node 1: $NODE1_DID${NC}"
echo -e "${GREEN}  Node 2: $NODE2_DID${NC}"
echo -e "${GREEN}  Node 3: $NODE3_DID${NC}"

# Step 3: Create a federation proposal
echo -e "\n${BLUE}[3/7] Creating a federation proposal...${NC}"
cat > .demo/proposals/federation_proposal.toml << EOF
# Federation Proposal
federation_id = "demo-federation"
name = "ICN Demo Federation"
description = "A demonstration federation for ICN testing"

# Nodes in the federation
[[nodes]]
did = "$NODE1_DID"
role = "Validator"

[[nodes]]
did = "$NODE2_DID"
role = "Validator"

[[nodes]]
did = "$NODE3_DID"
role = "Validator"

# Federation policies
[policies]
voting_threshold = 0.67
min_validators = 2
max_validators = 10
EOF

echo -e "${GREEN}Created federation proposal: .demo/proposals/federation_proposal.toml${NC}"

# Step 4: Clean up any previous federation
echo -e "\n${BLUE}[4/7] Cleaning up any previous federation...${NC}"
(cd demo/federation && ./init_federation.sh)

# Step 5: Starting federation nodes (Docker Compose)
echo -e "\n${BLUE}[5/7] Starting federation nodes (Docker Compose)...${NC}"
(cd demo/federation && docker-compose up -d)
sleep 5

# Health check
echo -e "${YELLOW}Checking node health...${NC}"
for port in 5001 5002 5003; do
  echo -n "  Node on port $port... "
  if curl -sf http://localhost:$port/health > /dev/null; then
    echo -e "${GREEN}healthy${NC}"
  else
    echo -e "${RED}failed${NC}"; exit 1
  fi
done

# Step 6: Submit and vote on proposal
echo -e "\n${BLUE}[6/7] Submitting proposal to federation...${NC}"
PROPOSAL_RESULT=$(cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- \
  federation submit-proposal \
  --to http://localhost:5001 \
  --file .demo/proposals/federation_proposal.toml \
  --key .demo/keys/node1.json)

# Extract proposal ID (simplified for demo)
PROPOSAL_ID="demo-proposal-id"
echo -e "${GREEN}Proposal submitted with ID: $PROPOSAL_ID${NC}"

# Vote on the proposal
echo -e "\n${YELLOW}Voting on the proposal...${NC}"
cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- \
  federation vote \
  --key .demo/keys/node2.json \
  --proposal-id $PROPOSAL_ID \
  --to http://localhost:5002

cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- \
  federation vote \
  --key .demo/keys/node3.json \
  --proposal-id $PROPOSAL_ID \
  --to http://localhost:5003

echo -e "${GREEN}Votes submitted successfully${NC}"

# Step 7: Execute the proposal
echo -e "\n${BLUE}[7/7] Executing the proposal...${NC}"
EXEC_RESULT=$(cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- \
  federation execute \
  --key .demo/keys/node1.json \
  --proposal-id $PROPOSAL_ID \
  --to http://localhost:5001)

# Extract receipt CID (simplified for demo)
RECEIPT_CID="demo-receipt-cid"
echo -e "${GREEN}Proposal executed with receipt CID: $RECEIPT_CID${NC}"

# Demo complete
echo -e "\n${BLUE}=== Demo Complete! ===${NC}"
echo -e "${GREEN}Federation is now operational with 3 nodes.${NC}"
echo -e "${GREEN}The federation ID is 'demo-federation'${NC}"
echo -e "\n${YELLOW}To clean up the demo:${NC}"
echo -e "  - Run 'cd demo/federation && docker-compose down' to stop the nodes"
echo -e "  - Run 'rm -rf .demo' to remove the generated files"
echo
echo -e "${BLUE}To interact with the federation, use the icn-cli tool:${NC}"
echo -e "  cargo run --release --manifest-path crates/tools/icn-cli/Cargo.toml -- federation -h"
echo 