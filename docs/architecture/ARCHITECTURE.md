# ICN System Architecture

## 1. Introduction

The InterCooperative Network (ICN) is an infrastructure stack for democratic, federated, verifiable governance. This document provides a comprehensive technical overview of the ICN system architecture, focusing on how its components interrelate to support a new paradigm of cooperative coordination. It is intended as a companion to [`OVERVIEW.md`](../index.md), offering deeper insight into the inner mechanics of the system.

At its core, ICN is built on principles of **modularity**, **cryptographic verifiability**, and **trust-first execution**. It fuses identity, deliberation, execution, and economic metering into a unified system governed by decentralized append-only DAGs.

---

## 2. System Layers

### 2.1 Wallet Layer

The **ICN Wallet** serves as the personal agent of every participant. It handles:

* Decentralized Identifier (DID) generation and key management
* Verifiable Credential (VC) storage, issuance, and selective disclosure
* Offline-first interaction with federations via light-client DAG sync
* Task queueing (e.g., proposals, votes) for asynchronous execution

Security is paramount. The wallet encrypts all local state and leverages platform-native keystores or secure enclaves (e.g., Android Keystore, Apple Secure Enclave) to protect private keys.

### 2.2 Deliberation Layer (AgoraNet)

**AgoraNet** provides structured, authenticated deliberation:

* DID-authenticated threads and messages
* Federation-scoped debate spaces (e.g., for cooperatives or communities)
* Thread-linked proposals that feed directly into governance votes
* Integration with the Wallet for identity and credential verification

This layer models democratic discourse as a first-class infrastructure concern.

### 2.3 Runtime Layer (CoVM)

The **Cooperative Virtual Machine (CoVM)** is a WASM-based execution environment that enforces governance logic:

* Executes compiled CCL (Contract Chain Language) modules
* Interfaces with a structured host ABI for logging, identity, storage, economics, and anchoring
* Anchors all execution outcomes into the federation DAG
* Tracks resource consumption using a fuel and token metering system

Execution is deterministic, scoped, and cryptographically attestable.

---

## 3. Data Flow and Interactions

1. **Deliberation** occurs in AgoraNet, authenticated by wallet-signed DIDs.
2. **Proposals** are compiled from CCL into WASM via the governance kernel.
3. **Votes** and signatures are gathered via TrustBundles with quorum proofs.
4. **Execution** happens in the Runtime with WASM, verifying identity, tokens, and policy.
5. **Anchoring** writes results into the DAG with Merkle proofs.
6. **Receipts** and VCs are issued and stored in the wallet, sharable across federations.

Every layer is cryptographically bound to its neighbors. Nothing proceeds without quorum trust and identity validation.

---

## 4. Governance Model

Each entity—whether an individual, cooperative, community, or federation—maintains its own append-only **DAG thread**. These threads:

* Begin with a unique `GenesisEvent`
* Include only verifiable, quorum-signed proposals and results
* Are immune to forking (structurally forbidden)
* Can **merge**, **split**, or **amend** via lineage attestations

**TrustBundles** act as signed checkpoints containing:

* Proposals
* Votes and receipts
* DAG anchors
* Credential hashes

Federations can verify each other's bundles, recursively forming a trust mesh.

---

## 5. Identity and Trust Infrastructure

ICN supports:

* `did:key` for self-sovereign identity
* `did:web` for institutional federation identities
* Scoped identity types (Individual, Cooperative, Community, Federation, Node)

Verifiable Credentials (VCs) are:

* Issued post-execution (e.g., `ExecutionReceipt`)
* Anchored into the DAG with CID hashes
* Selectively disclosed by wallets
* Validated via quorum proofs (`QuorumProof`), which support:

  * Majority
  * Threshold
  * Weighted signature schemes

**LineageAttestations** allow for federation splits, merges, and continuity tracking.

---

## 6. Economic System

Computation and coordination are enforced economically, not through arbitrary gas fees, but through **purpose-scoped tokens**.

Key primitives:

* `ScopedResourceToken`: credits for usage of a specific action (e.g., `storage.write`)
* `ResourceAuthorization`: policy-bound rules for who may use what, when, and how
* **Metering**: runtime tracks fuel use and token balances during execution

Policies are federation-defined. Economics are enforceable, auditable, and de-speculative.

---

## 7. Execution Environment

ICN uses the [Wasmtime](https://github.com/bytecodealliance/wasmtime) engine, with custom runtime integrations:

* **Host ABI**: WASM contracts import functions like `host_log_message`, `host_check_resource_authorization`, `host_anchor_to_dag`, etc.
* **Fuel metering**: Every execution is bounded
* **Memory safety**: Read/write access between WASM and host is strictly managed
* **Entrypoints**: `_start`, `main`, or `run` functions initiate logic
* **Audit layer**: Replay verification (`DAGAuditVerifier`) replays past executions and compares receipts

Execution is constitutional—bounded by declared intent, scoped policy, and verifiable proofs.

---

## 8. Federation Infrastructure

Federations are autonomous, inter-verifiable collectives. They:

* Bootstrap via `GenesisEvent` and TrustBundle quorum
* Validate all incoming DAG threads and credentials
* Anchor critical events via Merkle DAG proofs
* Support **merge**, **split**, **recovery**, and **key rotation** flows
* Sync DAGs and credentials via federation-aware APIs or libp2p mesh

Federation nodes run ICN Runtime instances and optionally serve light-client endpoints for wallet sync.

---

## 9. Security Architecture

ICN security is **multi-layered and cryptographically grounded**:

* **Identity**: Every action is signed by a DID
* **Execution**: WASM contracts run in hardened sandboxes
* **Anchoring**: All data is CID-addressed and Merkle-rooted
* **TrustBundles**: Require valid quorum proofs for acceptance
* **Replay**: Every receipt can be fully re-executed and validated
* **Recovery**: Key loss, federation failure, or succession events are all formally modeled and verifiable

Security is not an afterthought—it is built into the DAG, the execution model, and the economic logic.

---

## 10. Extensibility and Future Work

The ICN architecture is modular by design. Key upcoming directions include:

* **GPU task execution** via WASM-GPU integration
* **Planetary Mesh Compute Commons**, turning ICN into a shared execution substrate
* **Public goods funding models**, enforcing purpose-bound token flows for collective projects
* **Federation-as-a-service**, allowing communities to instantiate federations with minimal overhead
* **Cross-domain DID integration**, unifying legacy DNS and modern DID infrastructure

Everything is scoped, signed, and replayable—making ICN a flexible yet principled governance substrate.

---

## 11. Glossary / Key Concepts

* **DID**: Decentralized Identifier
* **VC**: Verifiable Credential
* **DAG**: Directed Acyclic Graph (append-only, Merkle-anchored)
* **TrustBundle**: A quorum-signed package of governance data
* **ScopedResourceToken**: A purpose-limited economic token
* **Federation**: A verifiable collective anchored in shared DAG and credential trust
* **CCL**: Contract Chain Language, a human-readable governance DSL
* **CoVM**: Cooperative Virtual Machine, the ICN runtime environment
* **AgoraNet**: Deliberation and proposal system

--- 