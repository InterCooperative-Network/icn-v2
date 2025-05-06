# ICN v2

A clean-slate refactor of the InterCooperative Network's federated infrastructure, emphasizing modular design, verifiable governance, and decentralized coordination.

## Key Features

- **DAG-Based Federated Ledger**: Constitutional governance using a Directed Acyclic Graph (DAG)
- **P2P DAG Synchronization**: Fully decentralized federation state synchronization over libp2p
- **TrustBundle Verification**: Quorum-based validation with cryptographic proofs
- **Multi-Node Federation**: Create, join, and participate in federated governance
- **Mesh Computation**: Latency-aware distributed compute with resource bidding

## Architecture Overview

The project follows a modular architecture organized within a Rust workspace:

```
crates/
├── common/
│   ├── icn-core-types/         # Core types used throughout ICN
│   ├── icn-identity-core/      # Identity management (DIDs, VCs) and cryptographic primitives
│   └── icn-types/              # Common data types and structures (DAG, TrustBundle, etc.)
├── runtime/
│   └── icn-runtime/            # Core runtime logic (CoVM)
├── wallet/
│   └── icn-wallet/             # Wallet management and transaction signing
├── agoranet/
│   └── agoranet-core/          # Deliberation layer, networking and consensus protocols
├── mesh/
│   └── planetary-mesh/         # Peer-to-peer mesh networking layer for distributed compute
└── tools/
    └── icn-cli/                # Command-line interface for interacting with the network

# Note: Additional crates like icn-ccl (Contract Chain Language) and icn-ffi (Foreign Function Interface)
# are planned. Some utility/experimental crates may exist but are not yet part of the main workspace.
# See DEVELOPER_GUIDE.md in docs/architecture/ for the most detailed structure.
```

## Getting Started

To get started with ICN v2 development:

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-username/icn-v2.git
    cd icn-v2
    ```
2.  **Install Rust:** Follow the instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
3.  **Build the workspace:**
    ```bash
    cargo build --release
    ```
4.  **Run tests:**
    ```bash
    cargo test --workspace
    ```

## Federation Genesis Example

Create a new federation with a Genesis DAG and start a node:

```bash
# Generate a DID key
icn key-gen --output founder.json

# Create a federation and start a node
icn dag sync-p2p genesis \
  --federation "my-federation" \
  --dag-dir ./data \
  --key ./founder.json \
  --policy-id "gov.icn.v1" \
  --founding-dids did:icn:founder1,did:icn:founder2 \
  --listen-addr "/ip4/0.0.0.0/tcp/9000"
```

See the [Federation Sync Guide](docs/guides/federation_sync.md) for more examples.

## Distributed Mesh Computation

Publish and execute tasks across federation nodes using latency-aware resource bidding:

```bash
# Publish a computational task
icn mesh publish-task \
  --wasm-file ./my-task.wasm \
  --input "s3://data/input1" \
  --federation "my-federation" \
  --key ./key.json \
  --dag-dir ./data

# Start a scheduler node
icn mesh scheduler \
  --federation "my-federation" \
  --key ./key.json \
  --dag-dir ./data
```

See the [Mesh Computation Guide](docs/guides/mesh_compute.md) for details.

## Contributing

Contributions are welcome! Please follow these general guidelines:

*   Fork the repository and create a new branch for your feature or bug fix.
*   Ensure your code adheres to the project's coding style (`cargo fmt`).
*   Write tests for new functionality.
*   Make sure all tests pass (`cargo test --workspace`).
*   Run linters (`cargo clippy --workspace -- -D warnings`).
*   Submit a pull request with a clear description of your changes.

## 🚧 Known Issues

- `ed25519_dalek::PublicKey` import fails during `cargo check`, even with correct dependency.
- Suspected IDE or toolchain cache issue. Reboot or `cargo clean` may not be sufficient.
- Proceeding with implementation assuming proper availability of the `PublicKey` type. 