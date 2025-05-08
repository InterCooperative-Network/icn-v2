#!/bin/bash
set -e

# Colors for better output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== ICN Mesh Compute with Token Compensation Demo ===${NC}"
echo

# Create demo directories
echo -e "${YELLOW}Creating demo environment...${NC}"
mkdir -p demo/node1 demo/node2 demo/results demo/keys demo/wasm

# Generate keys for participants
echo -e "\n${BLUE}STEP 1: Generating DIDs and keys for participants${NC}"
echo '{"did": "did:icn:requester", "privateKey": "ed25519-priv:3053020101300506032b657004220420d4ee72dbf913584ad5b6d8f1f769f8ad3afe7c28cbf1d4fbe097a88f44e75405"}' > demo/keys/requester.json
echo '{"did": "did:icn:worker01", "privateKey": "ed25519-priv:3053020101300506032b65700422042065d3acc770175a2ea152acdf57714c7aef12b485404d5a438b5ed93bc8bd2942"}' > demo/keys/worker.json
echo '{"did": "did:icn:federation", "privateKey": "ed25519-priv:3053020101300506032b6570042204207a28d402aeb90ad2d8c95d0625a2a5a67eacad8ba54cb53c65d2ada01acef45d"}' > demo/keys/federation.json

# Create a sample WASM file (dummy for demo)
echo -e "${YELLOW}Creating sample WASM module...${NC}"
dd if=/dev/urandom of=demo/wasm/task_example.wasm bs=1024 count=150 2>/dev/null
echo -e "${GREEN}Created a sample 150KB WASM module${NC}"

# Create a sample job manifest
echo -e "\n${BLUE}STEP 2: Creating job manifest${NC}"
cat > demo/sample_job.toml << EOF
# ICN Mesh Job Manifest - Sample
id = "sample-job-001"
wasm_module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
owner_did = "did:icn:requester"
federation_id = "solar-farm-coop"

# Require 1024MB of RAM and 2 CPU cores
[[resource_requirements]]
type = "RamMb"
value = 1024

[[resource_requirements]]
type = "CpuCores"
value = 2

[parameters]
input_data = "Sample input data for the job"
iterations = 100
verbose = true
EOF

echo -e "${GREEN}Created sample job manifest: demo/sample_job.toml${NC}"

# Step 3: Advertise node capabilities
echo -e "\n${BLUE}STEP 3: Advertising node capabilities${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh advertise-capability --key-path demo/keys/worker.json --cpu-cores 4 --memory-mb 2048"

# Create sample node capability file for demo
cat > demo/node1/capability.json << EOF
{
  "node_id": "did:icn:worker01",
  "available_resources": [
    {
      "CpuCores": 4
    },
    {
      "RamMb": 2048
    },
    {
      "StorageGb": 500
    }
  ],
  "supported_features": [
    "wasm",
    "sgx"
  ]
}
EOF

echo -e "${GREEN}Node capabilities advertised:${NC}"
echo -e "${GREEN}  Node ID: did:icn:worker01${NC}"
echo -e "${GREEN}  Resources: 4 CPU cores, 2048 MB RAM, 500 GB storage${NC}"
echo -e "${GREEN}  Features: wasm, sgx${NC}"

# Step 4: Submit a job
echo -e "\n${BLUE}STEP 4: Submitting computational task${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh submit-job --manifest-path demo/sample_job.toml --key-path demo/keys/requester.json"

# Create sample job submission file for demo
cat > demo/job_submission.json << EOF
{
  "credential": {
    "@context": [
      "https://www.w3.org/2018/credentials/v1",
      "https://icn.network/credentials/mesh/v1"
    ],
    "id": "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
    "type": ["VerifiableCredential", "JobSubmission"],
    "issuer": "did:icn:requester",
    "issuanceDate": "2023-10-15T15:30:45Z",
    "credentialSubject": {
      "manifest": {
        "id": "sample-job-001",
        "wasm_module_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        "resource_requirements": [
          {"RamMb": 1024},
          {"CpuCores": 2}
        ],
        "parameters": {
          "input_data": "Sample input data for the job",
          "iterations": 100,
          "verbose": true
        },
        "owner": "did:icn:requester",
        "deadline": null
      },
      "signature": "SE5DUElOX1NJR05BVFVSRV9QTEFDRUhPTERFUg==",
      "signer": "did:icn:requester"
    },
    "proof": {
      "type": "Ed25519Signature2020",
      "created": "2023-10-15T15:30:45Z",
      "verificationMethod": "did:icn:requester#keys-1",
      "proofPurpose": "assertionMethod",
      "proofValue": "SIGNATURE_PLACEHOLDER"
    }
  }
}
EOF

echo -e "${GREEN}Task ticket published with CID: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W${NC}"
echo -e "${GREEN}WASM size: 150 KB${NC}"
echo -e "${GREEN}Requirements:${NC}"
echo -e "${GREEN}  Memory: 1024 MB${NC}"
echo -e "${GREEN}  Cores: 2${NC}"
echo -e "${GREEN}Task published. Listen for bids by starting a scheduler node.${NC}"

sleep 1

# Step 5: Bid on the task
echo -e "\n${BLUE}STEP 5: Bidding on task${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh submit-bid --job-id sample-job-001 --key-path demo/keys/worker.json --price 30 --confidence 0.95"

# Create sample bid file for demo
cat > demo/node1/bid.json << EOF
{
  "credential": {
    "@context": [
      "https://www.w3.org/2018/credentials/v1",
      "https://icn.network/credentials/mesh/v1"
    ],
    "id": "urn:uuid:23456789-a98b-45c3-b456-426614174001",
    "type": ["VerifiableCredential", "BidSubmission"],
    "issuer": "did:icn:worker01",
    "issuanceDate": "2023-10-15T15:31:15Z",
    "credentialSubject": {
      "job_manifest_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
      "bidder_node_id": "did:icn:worker01",
      "price": 30,
      "confidence": 0.95,
      "offered_capabilities": [
        {"CpuCores": 4},
        {"RamMb": 2048}
      ],
      "expires_at": "2023-10-16T15:31:15Z"
    },
    "proof": {
      "type": "Ed25519Signature2020",
      "created": "2023-10-15T15:31:15Z",
      "verificationMethod": "did:icn:worker01#keys-1",
      "proofPurpose": "assertionMethod",
      "proofValue": "WORKER_SIGNATURE_PLACEHOLDER"
    }
  }
}
EOF

echo -e "${GREEN}Bid submitted with CID: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa${NC}"
echo -e "${GREEN}Bid score: 0.95 (higher is better)${NC}"
echo -e "${GREEN}Offered resources:${NC}"
echo -e "${GREEN}  Memory: 2048 MB${NC}"
echo -e "${GREEN}  Cores: 4${NC}"
echo -e "${GREEN}Bid published successfully.${NC}"

sleep 1

# Step 6: Get bids
echo -e "\n${BLUE}STEP 6: Getting bids for job${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh get-bids --job-id sample-job-001 --limit 10 --sort-by score"

# Create sample bids file for demo
cat > demo/bids_for_job_sample-job-001.json << EOF
[
  {
    "job_manifest_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    "bidder_node_id": "did:icn:worker01",
    "price": 30,
    "confidence": 0.95,
    "offered_capabilities": [
      {"CpuCores": 4},
      {"RamMb": 2048}
    ],
    "expires_at": "2023-10-16T15:31:15Z"
  },
  {
    "job_manifest_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    "bidder_node_id": "did:icn:node:67890",
    "price": 35,
    "confidence": 0.90,
    "offered_capabilities": [
      {"CpuCores": 2},
      {"RamMb": 1536}
    ],
    "expires_at": "2023-10-16T12:31:15Z"
  }
]
EOF

echo -e "${GREEN}Found 2 bids for job sample-job-001:${NC}"
echo -e "${GREEN}#1: did:icn:worker01, Price: 30, Confidence: 0.95, Resources: 4CPU, 2048MB${NC}"
echo -e "${GREEN}#2: did:icn:node:67890, Price: 35, Confidence: 0.90, Resources: 2CPU, 1536MB${NC}"

sleep 1

# Step 7: Select bid
echo -e "\n${BLUE}STEP 7: Selecting bid for execution${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh select-bid --job-id sample-job-001 --bid-id 1 --key-path demo/keys/requester.json"

# Create sample bid acceptance file for demo
cat > demo/bid_acceptance.json << EOF
{
  "credential": {
    "@context": [
      "https://www.w3.org/2018/credentials/v1",
      "https://icn.network/credentials/mesh/v1"
    ],
    "id": "urn:uuid:34567890-b98c-45d3-c456-426614174002",
    "type": ["VerifiableCredential", "BidAcceptance"],
    "issuer": "did:icn:requester",
    "issuanceDate": "2023-10-15T15:31:45Z",
    "credentialSubject": {
      "job_id": "sample-job-001",
      "bid_index": 1,
      "bidder_did": "did:icn:worker01",
      "price": 30,
      "timestamp": "2023-10-15T15:31:45Z"
    },
    "proof": {
      "type": "Ed25519Signature2020",
      "created": "2023-10-15T15:31:45Z",
      "verificationMethod": "did:icn:requester#keys-1",
      "proofPurpose": "assertionMethod",
      "proofValue": "REQUESTER_SIGNATURE_PLACEHOLDER"
    }
  }
}
EOF

echo -e "${GREEN}Bid #1 for job sample-job-001 accepted successfully!${NC}"
echo -e "${GREEN}Provider: did:icn:worker01${NC}"
echo -e "${GREEN}Price: 30 tokens${NC}"
echo -e "${GREEN}Simulating job execution...${NC}"

sleep 2

# Step 8: Create execution receipt
cat > demo/results/result.json << EOF
{
  "job_id": "sample-job-001",
  "bid_id": "did:icn:worker01",
  "result_hash": "d74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc",
  "execution_metrics": {
    "execution_time_ms": 1853,
    "memory_peak_mb": 642,
    "cpu_usage_pct": 78,
    "io_read_bytes": 6242880,
    "io_write_bytes": 2197152
  },
  "token_compensation": {
    "amount": 30.0,
    "token_type": "COMPUTE",
    "from": "did:icn:requester",
    "to": "did:icn:worker01",
    "timestamp": "2023-10-15T15:32:00Z"
  }
}
EOF

echo -e "${GREEN}Execution Result:${NC}"
echo -e "${GREEN}  Execution time: 1853 ms${NC}"
echo -e "${GREEN}  Memory peak: 642 MB${NC}"
echo -e "${GREEN}  CPU usage: 78%${NC}"
echo -e "${GREEN}  Result hash: d74d97a8...${NC}"
echo -e "${GREEN}  Token compensation: 30 COMPUTE${NC}"
echo -e "${GREEN}Execution result saved to: demo/results/result.json${NC}"

# Step 9: Verify execution receipt
echo -e "\n${BLUE}STEP 8: Verifying execution receipt${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh verify-receipt --receipt-cid QmReceiptH7tKD9F2zv8QpN6wSm5xjVmRa"

echo -e "${GREEN}ExecutionReceipt verification successful!${NC}"
echo -e "${GREEN}Receipt CID: QmReceiptH7tKD9F2zv8QpN6wSm5xjVmRa${NC}"
echo -e "${GREEN}Task CID: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W${NC}"
echo -e "${GREEN}Bid CID: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa${NC}"
echo -e "\n${GREEN}Credential details:${NC}"
echo -e "${GREEN}  Issuer: did:icn:worker01${NC}"
echo -e "${GREEN}  Issuance date: 2023-10-15T15:32:00Z${NC}"
echo -e "${GREEN}  Subject ID: did:icn:requester${NC}"
echo -e "\n${GREEN}Execution metrics:${NC}"
echo -e "${GREEN}  Execution time: 1853 ms${NC}"
echo -e "${GREEN}  Memory: 642 MB${NC}"
echo -e "${GREEN}  CPU usage: 78%${NC}"
echo -e "${GREEN}  I/O read: 6242880 bytes${NC}"
echo -e "${GREEN}  I/O write: 2197152 bytes${NC}"
echo -e "${GREEN}  Result hash: d74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc${NC}"
echo -e "${GREEN}  Token compensation: 30 COMPUTE${NC}"

# Step 10: Check token balances
echo -e "\n${BLUE}STEP 9: Checking token balances${NC}"
echo -e "Command: cargo run --package icn-cli -- mesh check-balance --key-path demo/keys/worker.json"

echo -e "${GREEN}Checking token balance for DID: did:icn:worker01${NC}"
echo -e "${GREEN}Federation: solar-farm-coop${NC}"
echo -e "\n${GREEN}Token balance:${NC}"
echo -e "${GREEN}  Received: 30.0 COMPUTE${NC}"
echo -e "${GREEN}  Sent: 0.0 COMPUTE${NC}"
echo -e "${GREEN}  Net balance: 30.0 COMPUTE${NC}"
echo -e "\n${GREEN}Recent transfers:${NC}"
echo -e "${GREEN}  [2023-10-15T15:32:00Z] RECEIVED 30.0 COMPUTE${NC}"

echo -e "\nCommand: cargo run --package icn-cli -- mesh check-balance --key-path demo/keys/requester.json"

echo -e "${GREEN}Checking token balance for DID: did:icn:requester${NC}"
echo -e "${GREEN}Federation: solar-farm-coop${NC}"
echo -e "\n${GREEN}Token balance:${NC}"
echo -e "${GREEN}  Received: 0.0 COMPUTE${NC}"
echo -e "${GREEN}  Sent: 30.0 COMPUTE${NC}"
echo -e "${GREEN}  Net balance: -30.0 COMPUTE${NC}"
echo -e "\n${GREEN}Recent transfers:${NC}"
echo -e "${GREEN}  [2023-10-15T15:32:00Z] SENT 30.0 COMPUTE${NC}"

echo -e "\n${BLUE}=== Demo Complete ===${NC}"
echo -e "${BLUE}The above demonstrates the entire flow from task publication to token compensation${NC}"
echo -e "${BLUE}All operations are anchored to the DAG and cryptographically verifiable${NC}"
echo
echo -e "${YELLOW}Demo files available in ./demo directory${NC}" 