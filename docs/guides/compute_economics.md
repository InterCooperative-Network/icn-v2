# Compute Economics & Credentials Guide

This guide explains the economic layer of ICN's mesh computation system, focusing on verifiable credential issuance and resource token compensation.

## Overview

The ICN mesh computation system now includes a complete economic layer that:

1. Issues verifiable credentials as proof of computation
2. Calculates and transfers resource tokens between nodes
3. Tracks token balances in federation-specific ledgers
4. Verifies the entire computation and compensation history using the DAG

## Verifiable Computation Credentials

### Credential Structure

Each successful computation produces an `ExecutionReceipt` with a W3C Verifiable Credential structure:

```json
{
  "credential": {
    "@context": [
      "https://www.w3.org/2018/credentials/v1",
      "https://icn.network/credentials/compute/v1"
    ],
    "id": "urn:icn:receipt:123e4567-e89b-12d3-a456-426614174000",
    "type": ["VerifiableCredential", "ExecutionReceipt"],
    "issuer": "did:icn:executor",
    "issuanceDate": "2023-10-15T12:00:00Z",
    "credentialSubject": {
      "id": "did:icn:requester",
      "taskCid": "QmTaskCid123",
      "bidCid": "QmBidCid456",
      "executionTime": 1500,
      "resourceUsage": {
        "memoryMb": 512,
        "cpuCores": 4,
        "cpuUsagePercent": 75,
        "ioReadBytes": 5242880,
        "ioWriteBytes": 2097152
      },
      "resultHash": "0x1234567890abcdef",
      "tokenCompensation": 45.123456
    }
  }
}
```

### Credential Verification

Credentials are cryptographically signed and anchored to the DAG, allowing any party to verify:

1. That the computation was performed by the claimed executor
2. That the resources claimed were actually used
3. That the result hash matches the expected output
4. That fair compensation was provided based on agreed terms

To verify a receipt:

```bash
icn mesh verify-receipt --receipt-cid QmReceiptCid123 --dag-dir ./dag-data
```

## Resource Token System

### Token Calculation

Token compensation is calculated based on the following formula:

```
tokens = (execution_time_sec) * (memory_mb + cpu_cores) * (reputation_factor)
```

Where:
- `execution_time_sec` is the actual execution time in seconds
- `memory_mb` and `cpu_cores` are the resources used
- `reputation_factor` is derived from the node's reputation score (higher = more tokens)

This formula rewards:
- More efficient execution (faster = fewer tokens spent)
- Higher reputation nodes (quality service = better compensation)
- Appropriate resource usage (prevents over-provisioning)

### Token Transfer

Every execution automatically creates a token transfer between:
- **From**: The task creator (requester)
- **To**: The winning bidder (executor)

The transfer is recorded in the DAG as a `ResourceTokenTransfer` node linked to the receipt:

```json
{
  "type": "ResourceTokenTransfer",
  "from": "did:icn:requester",
  "to": "did:icn:executor",
  "amount": 45.123456,
  "token_type": "COMPUTE",
  "federation_id": "my-federation",
  "task_cid": "QmTaskCid123",
  "timestamp": "2023-10-15T12:05:00Z"
}
```

### Checking Balances

To check a node's token balance:

```bash
icn mesh check-balance --key ./my-key.json --federation my-federation --dag-dir ./dag-data
```

This shows:
- Total tokens received
- Total tokens sent
- Net balance
- Recent transaction history

## Federation-Specific Token Economies

Each federation maintains its own token economy, with balances specific to that federation:

- Tokens can only be spent within the federation that issued them
- Federations can set their own token policies (inflation, decay, etc.)
- Cross-federation token bridges can be implemented through federation governance

## Example Workflow

Here's a complete example of the token compensation process:

```bash
# 1. Requester publishes a task with token allocation
icn mesh publish-task --wasm-file ./my-task.wasm --federation "compute-fed" --key ./requester.json --dag-dir ./data

# 2. Executor bids on the task
icn mesh bid --task-cid QmTask123 --latency 25 --memory 4096 --cores 8 --key ./executor.json --dag-dir ./data

# 3. Scheduler matches task and bid
icn mesh scheduler --federation "compute-fed" --key ./scheduler.json --dag-dir ./data

# 4. Executor runs the task and receives compensation
icn mesh execute --task-cid QmTask123 --bid-cid QmBid456 --key ./executor.json --dag-dir ./data --output-dir ./results

# 5. Both parties can verify the receipt and token transfer
icn mesh verify-receipt --receipt-cid QmReceipt789 --dag-dir ./data

# 6. Both parties can check their token balances
icn mesh check-balance --key ./requester.json --federation "compute-fed" --dag-dir ./data
icn mesh check-balance --key ./executor.json --federation "compute-fed" --dag-dir ./data
```

## Reputation Impact

Successful execution with verified receipts improves a node's reputation score over time, which:

1. Makes its bids more likely to be selected
2. Increases its token compensation rate
3. Builds its verifiable credential history in the DAG

Reputation is computed based on:
- Execution success rate
- Resource usage accuracy
- Latency claims vs. actual performance
- Longevity in the federation

## Advanced Applications

### Staking and Bonding

Future enhancements will include:
- Task creators staking tokens to guarantee payment
- Executors bonding tokens to guarantee performance
- Automatic slashing for non-performance or invalid results

### Credential-Based Access Control

Nodes that have earned specific credentials can gain access to:
- Premium computation tasks
- Sensitive or proprietary datasets
- Higher priority in the bid selection process

### Token-Based Governance

Token holders can participate in federation governance:
- Voting on economic policy changes
- Setting minimum bid requirements
- Adjusting validation parameters
- Electing scheduling authorities

## Next Steps

- Create custom token logic for your federation
- Implement reputation scoring for your specific use case
- Set up automated task pipelines with token budgets 