# ICN v2 Documentation Index

| Section        | Description |
| -------------- | ----------- |
| Architecture   | High‑level design & subsystems |
| Guides         | How‑to docs for devs & operators |
| Specs          | Formal schemas & protocol details |

## Key Resources

- [Developer Journey](guides/DEVELOPER_JOURNEY.md) - Complete end-to-end walkthrough of ICN capabilities
- [Architecture Overview](architecture/ARCHITECTURE.md) - High-level system design
- [Mesh Computation](guides/mesh_compute.md) - Distributed execution guide

## Understanding ICN's Core Layers

The InterCooperative Network is built upon a modular, three-layer architecture designed for verifiable governance and decentralized coordination:

1.  **🪙 Wallet Layer**: The user's personal agent within the ICN. It manages Decentralized Identifiers (DIDs), Verifiable Credentials (VCs), and facilitates secure, offline-first interactions with federations. 
    *Learn more in the [Architecture Overview](architecture/ARCHITECTURE.md#21-wallet-layer).*

2.  **🗣️ Deliberation Layer (AgoraNet)**: Provides the infrastructure for structured, authenticated discussions and proposal development. This layer ensures that governance processes are transparent and participatory. (Note: AgoraNet is currently in design phase).
    *Learn more in the [Architecture Overview](architecture/ARCHITECTURE.md#22-deliberation-layer-agoranet).*

3.  **⚙️ Runtime Layer (CoVM)**: The Cooperative Virtual Machine is a WASM-based execution environment. It runs compiled Contract Chain Language (CCL) modules, enforces governance logic, anchors outcomes to the federation DAG, and manages economic metering.
    *Learn more in the [Architecture Overview](architecture/ARCHITECTURE.md#23-runtime-layer-covm).*

These layers work in concert to provide a comprehensive platform for decentralized cooperation. Explore the [full Architecture Overview](architecture/ARCHITECTURE.md) for more details. 