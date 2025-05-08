#!/bin/bash
set -e

# ICN v2 Demo Flow: End-to-End Developer Test

echo "\n🚀 [1/8] Building the ICN workspace..."
cargo build --release --workspace

echo "\n🧹 [2/8] Cleaning up any previous federation..."
cd demo/federation
./init_federation.sh

echo "\n🐳 [3/8] Starting federation nodes (Docker Compose)..."
docker-compose up -d
sleep 5

# Health check
for port in 5001 5002 5003; do
  echo -n "Checking node on port $port... "
  if curl -sf http://localhost:$port/health > /dev/null; then
    echo "🟢 healthy"
  else
    echo "🔴 failed"; exit 1
  fi
done
cd ../../

echo "\n📤 [4/8] Submitting a sample proposal to node 1..."
cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation submit-proposal \
  --to http://localhost:5001 \
  --file examples/sample_proposal.toml

# Extract proposal ID (simulate or parse from output if possible)
PROPOSAL_ID="demo-proposal-id" # TODO: parse real ID from CLI output

echo "\n🗳️  [5/8] Voting on the proposal..."
cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation vote --proposal-id $PROPOSAL_ID

echo "\n⚙️  [6/8] Executing the proposal..."
cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation execute --proposal-id $PROPOSAL_ID

# Extract receipt CID (simulate or parse from output if possible)
RECEIPT_CID="demo-receipt-cid" # TODO: parse real CID from CLI output

echo "\n🧬 [7/8] Submitting a mesh job..."
cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- mesh submit-job \
  --manifest examples/sample_job.toml \
  --to http://localhost:5001

# Extract job ID (simulate or parse from output if possible)
JOB_ID="demo-job-id" # TODO: parse real job ID from CLI output

echo "\n🔍 [8/8] Verifying execution receipt..."
cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- wallet verify-receipt --id $RECEIPT_CID

echo "\n✅ Demo flow complete! Check logs and DAG for anchored results." 