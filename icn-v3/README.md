# Inter-Cooperative Network (ICN) v3

A planetary-scale governance, identity, and compute stack designed to replace extractive capitalist institutions with verifiable, DAG-anchored, cooperatively owned infrastructure.

## Architecture

ICN v3 is built on these core principles:

- **Scoped Authority**: All actions are performed within explicit scopes (cooperatives, federations, communities)
- **DAG-Anchored Verification**: Every operation is cryptographically verifiable in a directed acyclic graph
- **Federated Governance**: Cooperatives and communities form federations with democratic quorum voting
- **Resource Metering**: All compute, storage, and network usage is explicitly measured and allocated
- **WASM Execution**: Sandboxed, portable computation with deterministic resource usage

## Codebase Structure

- **common**: Core types, traits, and utilities shared across all components
- **ccl**: Cooperative Control Logic - governance primitives and operations
- **p2p**: Libp2p-based peer-to-peer networking
- **runtime**: WASM execution environment
- **services**: System services (DAG maintenance, credential issuance)
- **wallet**: Identity management and credential storage
- **tools**: Development utilities

## Current Status

The project is in the initial scaffolding phase. Core types and interfaces have been defined, including:

- DAG structure for verifiable operations
- Identity and credential system with scoped authority
- Resource allocation and metering primitives

## Next Steps

1. Implement basic DAG storage and verification
2. Build cooperative and federation creation flows
3. Develop runtime executor with resource metering

## Development

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Run examples
cargo run --example federation_demo
```

## License

AGPL-3.0 