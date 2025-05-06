# ICN v2

A clean-slate refactor of the InterCooperative Network's federated infrastructure, emphasizing modular design, verifiable governance, and decentralized coordination.

## Architecture Overview

The project follows a modular architecture organized within a Rust workspace:

```
crates/
├── runtime/icn-runtime           # Core runtime logic
├── wallet/icn-wallet             # Wallet management and transaction signing
├── agoranet/agoranet-core        # Core networking and consensus protocols
├── mesh/planetary-mesh           # Peer-to-peer mesh networking layer
├── common/
│   ├── icn-types                 # Common data types and structures
│   └── icn-identity-core       # Identity management and cryptographic primitives
└── tools/icn-cli                 # Command-line interface for interacting with the network
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

## Contributing

Contributions are welcome! Please follow these general guidelines:

*   Fork the repository and create a new branch for your feature or bug fix.
*   Ensure your code adheres to the project's coding style (`cargo fmt`).
*   Write tests for new functionality.
*   Make sure all tests pass (`cargo test --workspace`).
*   Run linters (`cargo clippy --workspace -- -D warnings`).
*   Submit a pull request with a clear description of your changes. 