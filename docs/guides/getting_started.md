# Getting Started with ICN v2

```bash
# Clone & build
 git clone https://github.com/InterCooperative-Network/icn-v2.git
 cd icn-v2 && cargo build --workspace

# Start devnet (scripts/devnet)
 ./scripts/devnet/start.sh

# Issue a demo credential
 icn-cli credential issue --to did:key:z123… --type DemoCredential
```

This guide walks you through running a local federation, issuing a credential, and verifying it. 