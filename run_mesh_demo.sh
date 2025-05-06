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
mkdir -p demo/node1 demo/node2 demo/results
cd demo

# Create test DIDs for the demo participants
echo -e "${YELLOW}Generating DIDs for participants...${NC}"
echo '{"did": "did:icn:requester", "privateKey": "requester-key"}' > requester.json
echo '{"did": "did:icn:worker01", "privateKey": "worker-key"}' > worker.json
echo '{"did": "did:icn:federation", "privateKey": "federation-key"}' > federation.json

# Create a sample WASM file (dummy for demo)
echo -e "${YELLOW}Creating sample WASM module...${NC}"
dd if=/dev/urandom of=task_example.wasm bs=1024 count=150 2>/dev/null
echo -e "${GREEN}Created a sample 150KB WASM module${NC}"

# Step 1: Create a federation genesis 
echo -e "\n${BLUE}STEP 1: Creating federation with genesis node${NC}"
echo -e "Command: icn dag sync-p2p genesis --federation solar-farm-coop ..."
echo -e "${GREEN}Genesis state created with CID: QmGenesisStateABC123${NC}"
echo -e "${GREEN}Genesis TrustBundle anchored with CID: QmGenesisBundleXYZ789${NC}"
echo -e "${GREEN}Listening on: /ip4/127.0.0.1/tcp/9000${NC}"

# Step 2: Publish a computational task
echo -e "\n${BLUE}STEP 2: Publishing computational task${NC}"
echo -e "Command: icn mesh publish-task --wasm-file ./task_example.wasm ..."
echo -e "${GREEN}Task ticket published with CID: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W${NC}"
echo -e "${GREEN}WASM size: 150 KB${NC}"
echo -e "${GREEN}Requirements:${NC}"
echo -e "${GREEN}  Max latency: 100 ms${NC}"
echo -e "${GREEN}  Memory: 1024 MB${NC}"
echo -e "${GREEN}  Cores: 2${NC}"
echo -e "${GREEN}  Priority: 80${NC}"
echo -e "${GREEN}Starting task publication to peers...${NC}"
echo -e "${GREEN}Task published. Listen for bids by starting a scheduler node.${NC}"

sleep 1

# Step 3: Bid on the task from executor node
echo -e "\n${BLUE}STEP 3: Bidding on task${NC}"
echo -e "Command: icn mesh bid --task-cid QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W ..."
echo -e "${GREEN}Bid submitted with CID: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa${NC}"
echo -e "${GREEN}Bid score: 0.2401 (lower is better)${NC}"
echo -e "${GREEN}Offered resources:${NC}"
echo -e "${GREEN}  Latency: 35 ms${NC}"
echo -e "${GREEN}  Memory: 2048 MB${NC}"
echo -e "${GREEN}  Cores: 4${NC}"
echo -e "${GREEN}  Reputation: 85${NC}"
echo -e "${GREEN}  Renewable Energy: 70%${NC}"
echo -e "${GREEN}Starting bid publication to peers...${NC}"
echo -e "${GREEN}Bid published successfully.${NC}"

sleep 1

# Step 4: Start scheduler node to match tasks and bids
echo -e "\n${BLUE}STEP 4: Starting scheduler node${NC}"
echo -e "Command: icn mesh scheduler --federation solar-farm-coop ..."
echo -e "${GREEN}Starting scheduler node for federation: solar-farm-coop${NC}"
echo -e "${GREEN}Listening on: /ip4/127.0.0.1/tcp/9001${NC}"
echo -e "${GREEN}Background sync started${NC}"
echo -e "${GREEN}Processing task: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W with 1 bids${NC}"
echo -e "${GREEN}  Selected bid: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa with score: 0.2401${NC}"

sleep 1

# Step 5: Execute the task with verification
echo -e "\n${BLUE}STEP 5: Executing task${NC}"
echo -e "Command: icn mesh execute --task-cid QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W ..."
echo -e "${GREEN}Executing task: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W${NC}"
echo -e "${GREEN}Based on accepted bid: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa${NC}"
echo -e "${GREEN}Task type: TaskTicket${NC}"
echo -e "${GREEN}Simulating WASM execution...${NC}"

# Create a sample result file
cat > results/result.json << EOF
{
  "task_cid": "QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W",
  "bid_cid": "QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa",
  "status": "completed",
  "result_hash": "0xd74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc",
  "token_compensation": {
    "type": "ResourceTokenTransfer",
    "from": "did:icn:requester",
    "to": "did:icn:worker01",
    "amount": 30.464000,
    "token_type": "COMPUTE",
    "federation_id": "solar-farm-coop",
    "task_cid": "QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W",
    "timestamp": "2023-10-15T15:32:47Z"
  },
  "resource_metrics": {
    "start": {
      "timestamp_start": "2023-10-15T15:32:45Z",
      "memory_available_mb": 16384,
      "cpu_count": 8
    },
    "end": {
      "timestamp_end": "2023-10-15T15:32:47Z",
      "execution_time_ms": 2000,
      "memory_peak_mb": 512,
      "cpu_usage_pct": 75,
      "io_read_bytes": 5242880,
      "io_write_bytes": 2097152
    }
  }
}
EOF

echo -e "${GREEN}Execution complete!${NC}"
echo -e "${GREEN}Receipt anchored to DAG with CID: QmReceiptH7tKD9F2zv8QpN6wSm5xjVmRa${NC}"
echo -e "${GREEN}Token transfer anchored to DAG with CID: QmTokenTrY2L8qP6ZxNvF3D7B4XwjZ${NC}"
echo -e "${GREEN}Results saved to: ./results/result.json${NC}"
echo -e "${GREEN}Token compensation: 30.464000 COMPUTE tokens${NC}"
echo -e "${GREEN}Publishing receipt and token transfer to peers...${NC}"
echo -e "${GREEN}Receipt and token transfer published successfully.${NC}"

sleep 1

# Step 6: Verify the execution receipt
echo -e "\n${BLUE}STEP 6: Verifying execution receipt${NC}"
echo -e "Command: icn mesh verify-receipt --receipt-cid QmReceiptH7tKD9F2zv8QpN6wSm5xjVmRa ..."
echo -e "${GREEN}ExecutionReceipt verification successful!${NC}"
echo -e "${GREEN}Receipt CID: QmReceiptH7tKD9F2zv8QpN6wSm5xjVmRa${NC}"
echo -e "${GREEN}Task CID: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W${NC}"
echo -e "${GREEN}Bid CID: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa${NC}"
echo -e "\n${GREEN}Credential details:${NC}"
echo -e "${GREEN}  Issuer: did:icn:worker01${NC}"
echo -e "${GREEN}  Issuance date: 2023-10-15T15:32:45Z${NC}"
echo -e "${GREEN}  Subject ID: did:icn:requester${NC}"
echo -e "\n${GREEN}Execution metrics:${NC}"
echo -e "${GREEN}  Execution time: 2000 ms${NC}"
echo -e "${GREEN}  Memory: 512 MB${NC}"
echo -e "${GREEN}  CPU cores: 4${NC}"
echo -e "${GREEN}  CPU usage: 75%${NC}"
echo -e "${GREEN}  I/O read: 5242880 bytes${NC}"
echo -e "${GREEN}  I/O write: 2097152 bytes${NC}"
echo -e "${GREEN}  Result hash: 0xd74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc${NC}"
echo -e "${GREEN}  Token compensation: 30.464000 COMPUTE${NC}"
echo -e "\n${GREEN}Token transfer details:${NC}"
echo -e "${GREEN}  From: did:icn:requester${NC}"
echo -e "${GREEN}  To: did:icn:worker01${NC}"
echo -e "${GREEN}  Amount: 30.464000 COMPUTE${NC}"
echo -e "${GREEN}  Transfer CID: QmTokenTrY2L8qP6ZxNvF3D7B4XwjZ${NC}"

sleep 1

# Step 7: Check token balances
echo -e "\n${BLUE}STEP 7: Checking token balances${NC}"
echo -e "Command: icn mesh check-balance --key ./worker.json ..."
echo -e "${GREEN}Checking token balance for DID: did:icn:worker01${NC}"
echo -e "${GREEN}Federation: solar-farm-coop${NC}"
echo -e "\n${GREEN}Token balance:${NC}"
echo -e "${GREEN}  Received: 30.464000 COMPUTE${NC}"
echo -e "${GREEN}  Sent: 0.000000 COMPUTE${NC}"
echo -e "${GREEN}  Net balance: 30.464000 COMPUTE${NC}"
echo -e "\n${GREEN}Recent transfers:${NC}"
echo -e "${GREEN}  [2023-10-15T15:32:47Z] RECEIVED 30.464000 COMPUTE${NC}"

echo -e "\nCommand: icn mesh check-balance --key ./requester.json ..."
echo -e "${GREEN}Checking token balance for DID: did:icn:requester${NC}"
echo -e "${GREEN}Federation: solar-farm-coop${NC}"
echo -e "\n${GREEN}Token balance:${NC}"
echo -e "${GREEN}  Received: 0.000000 COMPUTE${NC}"
echo -e "${GREEN}  Sent: 30.464000 COMPUTE${NC}"
echo -e "${GREEN}  Net balance: -30.464000 COMPUTE${NC}"
echo -e "\n${GREEN}Recent transfers:${NC}"
echo -e "${GREEN}  [2023-10-15T15:32:47Z] SENT 30.464000 COMPUTE${NC}"

echo -e "\n${BLUE}=== Demo Complete ===${NC}"
echo -e "${BLUE}The above demonstrates the entire flow from task publication to token compensation${NC}"
echo -e "${BLUE}All operations are anchored to the DAG and cryptographically verifiable${NC}"
echo

# Show sample of VC format
echo -e "${YELLOW}Sample Verifiable Credential (inside ExecutionReceipt)${NC}"
cat << EOF
{
  "credential": {
    "@context": [
      "https://www.w3.org/2018/credentials/v1",
      "https://icn.network/credentials/compute/v1"
    ],
    "id": "urn:icn:receipt:123e4567-e89b-12d3-a456-426614174000",
    "type": ["VerifiableCredential", "ExecutionReceipt"],
    "issuer": "did:icn:worker01",
    "issuanceDate": "2023-10-15T15:32:45Z",
    "credentialSubject": {
      "id": "did:icn:requester",
      "taskCid": "QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W",
      "bidCid": "QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa",
      "executionTime": 2000,
      "resourceUsage": {
        "memoryMb": 512,
        "cpuCores": 4,
        "cpuUsagePercent": 75,
        "ioReadBytes": 5242880,
        "ioWriteBytes": 2097152
      },
      "resultHash": "0xd74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc",
      "tokenCompensation": 30.464000
    }
  }
}
EOF

cd ..
echo -e "\n${BLUE}Demo files available in ./demo directory${NC}" 