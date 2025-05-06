# ICN Crate Index

This document provides a quick reference to the primary crates within the ICN monorepo.
For the most detailed project structure, please refer to the [`DEVELOPER_GUIDE.md`](./DEVELOPER_GUIDE.md).

| Crate Name (Conceptual) | Path in Workspace                | Description                                                                  | Status                                       |
|-------------------------|----------------------------------|------------------------------------------------------------------------------|----------------------------------------------|
| Runtime (CoVM)          | `crates/runtime/icn-runtime`     | Core governance and execution engine (CoVM)                                  | ✅ Active                                    |
| Wallet                  | `crates/wallet/icn-wallet`       | Core logic for ICN Wallet (key management, DAG sync)                         | ✅ Active                                    |
| AgoraNet                | `crates/agoranet/agoranet-core`  | Deliberation layer (proposals, discussions)                                  | ✅ Active                                    |
| Mesh Compute            | `crates/mesh/planetary-mesh`     | Planetary compute commons for distributed task execution                     | ✅ Active                                    |
| Types                   | `crates/common/icn-types`        | Common data types and structures (DAGs, TrustBundles)                        | ✅ Active                                    |
| Identity Core           | `crates/common/icn-identity-core`| DID, VC, TrustBundle management, cryptographic utilities                   | ✅ Active                                    |
| Core Types              | `crates/common/icn-core-types`   | Fundamental types used across the ICN system                                 | ✅ Active                                    |
| CLI Tool                | `crates/tools/icn-cli`           | Main command-line interface for ICN                                          | ✅ Active                                    |
|                         |                                  |                                                                              |                                              |
| **Conceptual/Planned**  |                                  |                                                                              |                                              |
| CCL Compiler            | `icn-ccl` (TBD)                  | Contract Chain Language parser, compiler, and tooling                        | ⚠️ Planned/TBD                            |
| FFI Bindings            | `icn-ffi` (TBD)                  | Foreign Function Interface bindings (e.g., UniFFI for mobile)                | ⚠️ Planned/TBD                            |
| Mesh Control CLI        | `meshctl` (TBD)                  | Dedicated CLI tool for mesh network control                                  | ❓ Status unclear (possibly merged/TBD)      |
|                         |                                  |                                                                              |                                              |
| **Other Found Crates**  |                                  |                                                                              |                                              |
| Standalone Mesh Crate   | `crates/planetary-mesh/`         | Standalone mesh-related crate.                                               | ❔ Not in workspace, purpose to be clarified |
| Generic CLI Crate       | `crates/cli/`                    | Generic CLI-related crate.                                                   | ❔ Not in workspace, purpose to be clarified |

**Status Key:**
*   ✅ **Active**: Core component, actively developed and part of the main workspace.
*   ⚠️ **Planned/TBD**: Conceptual component, planned for future implementation or status to be determined.
*   ❓ **Status Unclear**: Component's role or future is not clearly defined.
*   ❔ **Not in Workspace**: Crate exists but is not part of the main `Cargo.toml` workspace members.

--- 