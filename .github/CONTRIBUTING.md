# Contributing to ICN

We welcome PRs, bug reports, and new ideas!

## Setup

1. Install Rust (stable): https://rustup.rs
2. Clone the repo and run:
   ```bash
   cargo check --all
   cargo test --all
   just bootstrap
   ```

## Submitting Changes

*   Use feature branches (e.g., `feat/new-token`, `fix/dag-sync-bug`, `rfc/my-proposal`, `docs/improve-wallet-guide`).
*   Follow [Conventional Commits](https://www.conventionalcommits.org/).
*   Run all tests and linters before submitting a PR:
    ```bash
    cargo check --workspace
    cargo test --workspace
    cargo clippy --all-targets -- -D warnings
    cargo fmt --all -- --check 
    ```
*   If you changed CLI commands or options, regenerate the command reference:
    ```bash
    just gen-cli-docs
    ```
*   Update documentation where relevant or add new guides if applicable.

## RFCs

Major architectural, economic, or governance proposals should be submitted as **RFCs (Requests for Comments)**.
Use the "🧠 RFC Proposal" issue template to get started. This allows for community discussion and design iteration before significant implementation effort.

## Community

We're building infrastructure for federated governance and cooperative economics. Join us with care, curiosity, and a commitment to building empowering tools.

(Link to more detailed developer guide: `docs/architecture/DEVELOPER_GUIDE.md`) 