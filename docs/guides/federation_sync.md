# Federation Genesis and DAG Synchronization Guide

This guide shows how to create a Genesis state for a new federation and synchronize it across multiple nodes using the DAG synchronization over libp2p.

## Prerequisites

- ICN CLI installed
- Understanding of DIDs and federation concepts
- Multiple machines (or local ports) to run ICN nodes

## Creating a Federation Genesis Node

First, generate a DID key for the founding member:

```bash
icn key-gen --output founder1.json
```

Then create a Genesis state and start the node:

```bash
icn dag sync-p2p genesis \
  --federation "my-test-federation" \
  --dag-dir ./dag-data \
  --key ./founder1.json \
  --policy-id "gov.test.v1" \
  --founding-dids did:example:founder1,did:example:founder2,did:example:founder3 \
  --listen-addr "/ip4/0.0.0.0/tcp/9000" \
  --start-node true
```

This command:
1. Creates a genesis state for the federation
2. Signs it with the founder's key
3. Creates a TrustBundle anchoring the state
4. Starts a node listening for connections

## Joining an Existing Federation

On another machine or port, generate a new DID key:

```bash
icn key-gen --output member1.json
```

Connect to the Genesis node and start syncing:

```bash
icn dag sync-p2p connect \
  --peer "/ip4/192.168.1.100/tcp/9000/p2p/QmFounderPeerId" \
  --federation "my-test-federation" \
  --dag-dir ./dag-data-member
```

Alternatively, use auto-discovery to find and connect to federation peers:

```bash
icn dag sync-p2p auto-sync \
  --federation "my-test-federation" \
  --dag-dir ./dag-data-member \
  --bootstrap-peers "/ip4/192.168.1.100/tcp/9000/p2p/QmFounderPeerId" \
  --authorized-dids did:example:founder1,did:example:founder2,did:example:founder3
```

## Visualizing the DAG

Generate a visual representation of the federation's DAG:

```bash
icn dag visualize \
  --dag-dir ./dag-data \
  --output federation-dag.dot \
  --max-nodes 50
```

Convert the DOT file to a PNG image:

```bash
dot -Tpng federation-dag.dot -o federation-dag.png
```

## Verifying Federation State

Check the DAG tips (latest updates):

```bash
# Get the latest state and verify it
icn dag replay --cid QmLatestBundleCid --dag-dir ./dag-data
```

Verify a specific TrustBundle:

```bash
icn dag verify-bundle --cid QmBundleCid --dag-dir ./dag-data
```

## Federation Example Use Cases

### Constitutional Updates

You can update the federation's governance rules by:

1. Creating a new state document with the updated rules
2. Creating a TrustBundle with the required quorum signatures 
3. Anchoring the bundle to the DAG
4. Automatically syncing to all connected nodes

### Decentralized Voting

Implement voting protocols where:

1. Votes are submitted as DAG nodes
2. TrustBundles aggregate and validate votes 
3. Results are anchored to the DAG
4. The planetary mesh ensures consistent state

## Troubleshooting

- **Sync Issues**: Check connectivity between nodes with `ping` and verify libp2p addresses
- **Verification Failures**: Ensure all nodes have the same federation ID and policy configuration
- **Node Discovery Problems**: Use explicit peer addresses when mDNS discovery isn't working

## Next Steps

- Add custom sync policies
- Implement credential verification for nodes
- Set up periodic verification checks 