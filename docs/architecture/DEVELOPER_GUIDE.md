# `DEVELOPER_GUIDE.md` — ICN Contributor and Development Guide

## 1. Introduction

Welcome to the InterCooperative Network (ICN) development guide!

*   **Purpose of this guide**: This document provides all necessary information for developers, contributors, and reviewers to understand the ICN codebase, set up a development environment, follow our workflows, and contribute effectively.
*   **Who it's for**: Whether you're a new contributor looking to make your first commit, an experienced developer exploring a specific module, or a reviewer ensuring code quality, this guide is for you.
*   **High-level goals**: Our development philosophy centers around:
    *   **Clarity**: Writing understandable and maintainable code.
    *   **Safety**: Prioritizing robust, secure, and fault-tolerant systems through rigorous testing and cryptographic guarantees.
    *   **Velocity**: Enabling efficient development and iteration without sacrificing quality.
    *   **Federation-readiness**: Ensuring all components are designed for interoperability and scalability within a federated ecosystem.

---

## 2. Project Structure

ICN utilizes a monorepo structure to manage its various components and facilitate integrated development.

*   **Overview of the monorepo layout**:
    *   `crates/`: Contains all Rust crates, the core of the ICN system.
        *   `crates/icn-common/`: Shared types, cryptographic utilities, error handling, and core interfaces used across multiple ICN crates.
        *   `crates/icn-identity/`: Manages DIDs, Verifiable Credentials (VCs), TrustBundles, and quorum proofs.
        *   `crates/icn-runtime/`: The core governance and execution engine (CoVM - Cooperative Virtual Machine). Handles WASM execution, DAG anchoring, and economic metering.
        *   `crates/icn-wallet-core/`: Core logic for the ICN Wallet, including key management, DAG synchronization, and FFI preparation.
        *   `crates/icn-agoranet/`: Implements the deliberation layer for proposals, discussions, and thread management.
        *   `crates/icn-mesh/`: Code for the planetary compute commons, enabling distributed task execution.
        *   `crates/icn-ccl/`: The Contract Chain Language parser, compiler, and associated tooling.
        *   `crates/icn-ffi/`: Foreign Function Interface bindings (e.g., UniFFI for mobile) for the wallet and other components.
    *   `tools/`: Contains CLI utilities and developer tools.
        *   `tools/icn-cli/`: The main command-line interface for interacting with ICN federations, managing identities, submitting jobs, and managing the compute mesh (e.g. via `icn-cli mesh ...` subcommands).
    *   `docs/`: Contains all project documentation.
        *   `docs/architecture/`: Detailed architectural documents, specifications, and design rationale.
        *   `docs/rfc/`: Requests for Comments for proposing significant changes or new features.
    *   `scripts/`: Helper scripts for common development tasks (setup, testing, building).
    *   `examples/`: Example code, configurations, and sample CCL contracts.

*   **Naming conventions**:
    *   Rust crates are prefixed with `icn-` (e.g., `icn-runtime`, `icn-wallet-core`).
    *   Binaries (CLI tools) follow a similar pattern (e.g., `icn-cli`).

---

## 3. Development Environment Setup

### Prerequisites

Ensure the following tools are installed on your system:

*   **Rust**:
    *   Install via `rustup` (https://rustup.rs/).
    *   The project uses the latest stable Rust toolchain. Ensure it's up-to-date: `rustup update stable`.
*   **Wasmtime CLI**:
    *   Required for testing and interacting with WASM modules directly.
    *   Installation instructions: https://docs.wasmtime.dev/cli-installation.html
*   **Node.js and Yarn** (Optional, for mobile wallet frontend development):
    *   If you plan to work on mobile wallet UI components that might use JavaScript frameworks.
    *   Node.js: https://nodejs.org/
    *   Yarn: `npm install --global yarn`
*   **Docker & Docker Compose** (Recommended for federation testing):
    *   Useful for setting up multi-node ICN federations locally for testing.
    *   Docker: https://www.docker.com/get-started
    *   Docker Compose: Usually included with Docker Desktop.

### Installation Steps

1.  **Clone the repository**:
    ```bash
    git clone https://github.com/your-org/icn-v2.git # Replace with actual repo URL
    cd icn-v2
    ```

2.  **Run setup scripts** (if available):
    *   Check for a general setup script that might install dependencies or configure hooks:
        ```bash
        # Example:
        # sh ./scripts/setup.sh
        ```
    *   This step might vary; refer to any specific setup instructions in the root `README.md`.

3.  **Initial build and test pass**:
    *   Verify that the project builds and core tests pass:
        ```bash
        cargo check --all
        cargo test --all
        ```
    *   This may take some time on the first run as dependencies are downloaded and compiled.

---

## 4. Core Development Workflows

*   **Building individual crates**:
    *   Navigate to a specific crate's directory: `cd crates/icn-common`
    *   Build: `cargo build`
    *   Test: `cargo test`
    *   Check: `cargo check`

*   **Building and testing the full workspace**:
    *   From the repository root:
        *   `cargo build --all-targets` (Builds all crates and targets)
        *   `cargo test --all-targets` (Runs all tests in the workspace)
        *   `cargo clippy --all-targets -- -D warnings` (Run linter, treat all warnings as errors)
        *   `cargo fmt --all -- --check` (Check formatting)

*   **Editing and testing WASM modules**:
    *   WASM modules for governance contracts are typically written in Rust and compiled to the `wasm32-unknown-unknown` target.
    *   The CoVM (Runtime) executes these modules. Test them by deploying to a local test federation or using specialized WASM testing tools.
    *   You can often test WASM contract logic independently before integration.

*   **Writing and compiling CCL → WASM**:
    *   The `icn-ccl` crate and associated tools (e.g., a potential `ccl-compiler` binary) are used for this.
    *   CCL files (`.ccl`) are compiled into WASM modules (`.wasm`).
    *   Example workflow:
        ```bash
        # Fictional ccl-compiler usage
        # ccl-compiler compile my_contract.ccl -o my_contract.wasm
        ```
    *   Refer to `crates/icn-ccl/README.md` for specific instructions.

*   **Using the CLI tools**:
    *   Build CLI tools (e.g., `icn-cli`):
        ```bash
        cargo build --release -p icn-cli
        ```
    *   Run them from `target/release/` or install them using `cargo install --path tools/icn-cli`.
    *   Example: `icn-cli mesh list-nodes`

---

## 5. Testing and Validation

Comprehensive testing is crucial for ICN's reliability and security.

*   **Unit tests (`#[test]`)**:
    *   Each module should have thorough unit tests for its core logic.
    *   Located within the same file or in a `tests` submodule (`mod tests { ... }`).
    *   Run with `cargo test` within a crate or `cargo test -p <crate_name>`.

*   **Integration tests**:
    *   Test interactions between different modules or components.
    *   Typically located in a `tests/` directory at the crate root (e.g., `crates/icn-runtime/tests/federation_bootstrap.rs`).
    *   These tests often involve setting up more complex scenarios.

*   **Fuzzing and runtime replay validation**:
    *   Explore `cargo-fuzz` for fuzz testing critical components, especially parsers and state machines.
    *   The runtime should support replaying DAG histories to validate state consistency (`DAGAuditVerifier` mentioned in `ARCHITECTURE.md`).

*   **DAG and TrustBundle replay checks**:
    *   Develop tools and tests to specifically verify the integrity and correct processing of DAG structures and TrustBundles.

*   **CI/CD workflows and GitHub Actions**:
    *   The project uses GitHub Actions (see `.github/workflows/`) for continuous integration.
    *   CI runs checks, lints, builds, and tests on every push and pull request.
    *   Ensure your changes pass CI before merging.

---

## 6. Contribution Guidelines

*   **Branching strategy**:
    *   Develop features or fixes in branches off `main` (or a `develop` branch if used).
    *   Name branches descriptively (e.g., `feat/new-token-logic`, `fix/dag-sync-error`).
*   **Commit format**:
    *   Follow Conventional Commits (https://www.conventionalcommits.org/).
    *   Example: `feat(runtime): implement fuel metering for WASM calls`
    *   Example: `fix(wallet): correct VC parsing for v2 credentials`
*   **Code review process**:
    *   Submit Pull Requests (PRs) to the main development branch.
    *   Ensure your PR description clearly explains the changes and their rationale.
    *   At least one approval from a core maintainer is typically required.
    *   Address review comments promptly.
*   **Linting, formatting, and Clippy**:
    *   Run `cargo fmt --all` to format your code.
    *   Run `cargo clippy --all-targets -- -D warnings` to catch common mistakes and style issues.
    *   Ensure these checks pass before submitting a PR.
*   **Submitting issues and RFCs**:
    *   Use GitHub Issues for bug reports, feature requests, and discussions.
    *   For significant changes, new features, or architectural decisions, consider submitting an RFC (Request for Comments) in the `docs/rfc/` directory. Follow the existing RFC template.
*   **Where to ask questions**:
    *   (To be filled by the project team) e.g., Matrix channels, Discord server, dedicated forums.

---

## 7. Developer Tooling

*   **Custom scripts (`scripts/`, `justfile`)**:
    *   The `scripts/` directory may contain shell scripts for common tasks.
    *   If a `justfile` is present, install `just` (https://github.com/casey/just) and run tasks like `just build`, `just test`.
*   **Diagnostic tools**:
    *   The project may include tools for:
        *   `DAG explorer`: Visualizing or inspecting DAG structures.
        *   `VC validator`: Checking the validity and signature of Verifiable Credentials.
    *   These might be part of `icn-cli` or separate utilities.
*   **Docker dev environments and federation testing harness**:
    *   Use Docker Compose files (e.g., `docker-compose.dev.yml`) to spin up local multi-node federations for testing end-to-end scenarios.
*   **Debug logging and tracing**:
    *   The codebase likely uses the `log` crate with an environment logger like `env_logger` or `tracing`.
    *   Set the `RUST_LOG` environment variable to control log verbosity (e.g., `RUST_LOG=icn_runtime=debug,icn_wallet_core=info`).

---

## 8. Common Tasks and Recipes

This section provides step-by-step guides for frequent development activities.

*   **How to write and test a new governance proposal (CCL → WASM)**:
    1.  Define your proposal logic in a `.ccl` file.
    2.  Use the `ccl-compiler` (or equivalent tool) to compile it to a `.wasm` module.
    3.  Write an integration test that:
        *   Sets up a test federation.
        *   Submits the WASM proposal via `icn-cli` or direct API calls.
        *   Simulates voting and quorum achievement.
        *   Verifies the proposal's execution and its effects on the DAG and state.

*   **How to anchor data to the DAG from WASM**:
    1.  Within your WASM contract (written in Rust), use the host ABI function provided by CoVM (e.g., `host_anchor_to_dag(data: &[u8]) -> Result<Cid, Error>`).
    2.  Ensure your WASM module has the necessary permissions/capabilities to perform anchoring.
    3.  The CoVM will handle the actual Merkleization and DAG update.

*   **How to issue and verify Verifiable Credentials**:
    1.  **Issuance**: After a successful governance action or specific event, the runtime (or a privileged module) can construct a VC. Use `icn-identity` crate's types. The VC is then signed by the issuer's DID.
    2.  **Verification**:
        *   Parse the VC.
        *   Verify the issuer's signature against their DID.
        *   Check the VC's schema, expiration, and any DAG-anchored proofs.
        *   Use functions from `icn-identity` for these steps.

*   **How to build a mobile FFI binding (UniFFI)**:
    1.  Define the interface in a UDL (`.udl`) file (e.g., in `crates/icn-ffi/src/`).
    2.  Implement the Rust side of the FFI functions, often wrapping functionality from `icn-wallet-core`.
    3.  Use `uniffi-bindgen` (via `cargo uniffi_bindgen generate_scaffolding` or similar build script integration) to generate the foreign language bindings (Swift, Kotlin, Python).
    4.  Integrate these bindings into the respective mobile application projects.

*   **How to run a multi-node federation with Docker**:
    1.  Ensure Docker and Docker Compose are installed.
    2.  Look for a `docker-compose.yml` or similar file in the root or a `deployment/` directory.
    3.  This file should define services for multiple ICN nodes, potentially a bootstrapper, and any necessary networking.
    4.  Run `docker-compose up`.
    5.  Use `icn-cli` to interact with the nodes in the Docker network.

---

## 9. Known Pitfalls and Troubleshooting

*   **Common build issues**:
    *   **`cid`/`multihash` feature resolution**: These crates have many features; ensure consistent feature flags across the workspace. A `[patch.crates-io]` section in the root `Cargo.toml` might be used to enforce versions or features.
    *   **Protobuf/gRPC compilation**: If used, ensure `protoc` and relevant toolchains are installed.
*   **WASM execution errors and debugging tips**:
    *   Ensure WASM modules are compiled with the correct target (`wasm32-unknown-unknown`).
    *   Check for sufficient fuel and correct host ABI calls.
    *   Use extensive logging within WASM contracts (via host ABI logging functions).
    *   Wasmtime CLI can sometimes be used to run and debug WASM modules locally if they don't have complex host dependencies.
*   **Credential validation and quorum signature errors**:
    *   Double-check DID keys and signature algorithms.
    *   Ensure TrustBundles are correctly formed and all required signatures for a quorum are present and valid.
    *   Verify timestamps and nonces if replay protection is involved.
*   **Platform-specific build quirks**:
    *   **macOS**: May require specific versions of XCode command-line tools or OpenSSL configurations.
    *   **WSL2**: Ensure file paths and permissions are handled correctly, especially when interacting with Docker.
    *   **Mobile targets**: Cross-compilation setups (e.g., for `aarch64-linux-android`, `armv7-linux-androideabi`, `aarch64-apple-ios`) require correct NDK/SDK paths and linker settings.

---

## 10. Glossary / Quick Reference

*   **CoVM**: Cooperative Virtual Machine (the ICN runtime).
*   **CCL**: Contract Chain Language (for writing governance proposals).
*   **DAG**: Directed Acyclic Graph (core data structure for history).
*   **DID**: Decentralized Identifier.
*   **VC**: Verifiable Credential.
*   **TrustBundle**: A quorum-signed package of governance data.
*   **Key Paths**:
    *   `crates/`: Location of all Rust source code.
    *   `docs/architecture/`: For detailed design documents.
    *   `tools/icn-cli/`: Main CLI tool.
*   **Common Commands**:
    *   `cargo check --all`: Quick compilation check.
    *   `cargo test --all`: Run all tests.
    *   `cargo clippy --all-targets -- -D warnings`: Linting.
    *   `cargo fmt --all`: Code formatting.
*   **Companion Docs**:
    *   [`ARCHITECTURE.md`](./ARCHITECTURE.md)
    *   [`SECURITY.md`](./SECURITY.md) (To be created)
    *   [`ECONOMICS.md`](./ECONOMICS.md) (To be created)

---
This guide is a living document. Please contribute to its improvement by submitting PRs for corrections, additions, or clarifications. 