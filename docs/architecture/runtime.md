# ICN v2 — Runtime (CoVM)

The Cooperative Virtual Machine (CoVM) is the core execution engine.

- **WASM Host ABI**: Defines the interface between WASM guest modules and the host environment, enabling access to DAG storage, cryptographic primitives, and network services.
- **DAG Anchoring**: Transactions and state transitions are cryptographically linked into the global Directed Acyclic Graph (DAG), ensuring verifiable history and forkless updates.
- **Resource Metering**: Execution steps and storage are metered to prevent abuse and manage network resources. 