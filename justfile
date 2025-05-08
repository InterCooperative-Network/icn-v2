# ICN v2 Developer Automation - justfile

# Build the full workspace
build:
    cargo build --release --workspace

# Run all tests
check:
    cargo check --workspace

test:
    cargo test --workspace

# Clean build artifacts
clean:
    cargo clean

# Bootstrap federation (requires Docker)
bootstrap-federation:
    cd demo/federation && ./init_federation.sh

# Start federation nodes (Docker Compose)
run-federation:
    cd demo/federation && docker-compose up -d

# Stop federation nodes
stop-federation:
    cd demo/federation && docker-compose down

# Check health of all federation nodes
health:
    curl -sf http://localhost:5001/health && \
    curl -sf http://localhost:5002/health && \
    curl -sf http://localhost:5003/health

# Submit a sample proposal (edit path as needed)
submit-proposal:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation submit-proposal \
      --to http://localhost:5001 \
      --file examples/sample_proposal.toml

# Vote on a proposal (edit <id> as needed)
vote:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation vote --proposal-id <id>

# Execute a proposal (edit <id> as needed)
execute:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- federation execute --proposal-id <id>

# Submit a mesh job (edit path as needed)
submit-job:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- mesh submit-job \
      --manifest examples/sample_job.toml \
      --to http://localhost:5001

# Check mesh job status (edit <id> as needed)
job-status:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- mesh job-status --job-id <id>

# Verify an execution receipt (edit <cid> as needed)
verify-receipt:
    cargo run --manifest-path crates/tools/icn-cli/Cargo.toml -- wallet verify-receipt --id <cid>

gen-cli-docs:
    cargo run --package icn-cli --bin gen_clap_docs

propose-ccl FILE SCOPE TITLE:
    cargo run -p icn-cli -- dag propose-ccl {{FILE}} --scope {{SCOPE}} --title "{{TITLE}}" 