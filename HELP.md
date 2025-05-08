# ICN Help Guide

This document provides answers to common questions and solutions to typical issues you might encounter when working with the ICN system.

## Table of Contents

- [General Questions](#general-questions)
- [Installation Issues](#installation-issues)
- [Key Management](#key-management)
- [Federation Commands](#federation-commands)
- [Mesh Network](#mesh-network)
- [Debugging Tips](#debugging-tips)

## General Questions

### What is ICN?

ICN (Interoperable Compute Network) is a decentralized federation system for distributed computing resources. It enables seamless sharing and utilization of computing power across different networks with built-in governance, trust, and accountability.

### How do I get started?

1. Make sure you have installed all [prerequisites](#installation-issues)
2. Run the demo script: `./icn_demo.sh`
3. Explore the CLI commands in the [README](README.md)

## Installation Issues

### Required Dependencies

- **Rust/Cargo**: Install from [rust-lang.org](https://www.rust-lang.org/tools/install)
- **Docker & Docker Compose**: Follow instructions at [docker.com](https://www.docker.com/get-started)
- **Build Tools**: Depending on your OS:
  - Ubuntu/Debian: `apt install build-essential pkg-config libssl-dev`
  - Fedora/RHEL: `dnf install gcc gcc-c++ openssl-devel`
  - macOS: `xcode-select --install`

### Common Installation Errors

#### "Could not find libssl"

```
error: failed to run custom build command for `openssl-sys`
```

Install OpenSSL development packages for your operating system:
- Ubuntu/Debian: `apt install libssl-dev`
- Fedora/RHEL: `dnf install openssl-devel`
- macOS: `brew install openssl@1.1`

#### Docker Issues

If Docker containers fail to start:

1. Ensure Docker service is running: `systemctl status docker`
2. Check for port conflicts: `netstat -tuln | grep 500`
3. Increase Docker memory limits in Docker Desktop settings

## Key Management

### How do DIDs work in ICN?

DIDs (Decentralized Identifiers) are the foundation of identity in ICN. Each node or participant has a DID derived from a public key, allowing for secure identification and cryptographic verification.

### I've lost my key file, what can I do?

Unfortunately, if you've lost your key file, you cannot recover it. You'll need to:
1. Generate a new key with `cargo run -p icn-cli -- key-gen --output new-key.json`
2. Use the new key for future operations
3. If you were part of a federation, you'll need to request re-admission with your new DID

### Key format errors

If you encounter key format errors, ensure you're using the correct format. ICN uses a specific key format with the prefix `ed25519-priv:` for private keys.

## Federation Commands

### Federation proposal submission fails

If your proposal submission fails:

1. Check node connectivity: `curl -sf http://localhost:5001/health`
2. Verify your key file path is correct
3. Ensure the proposal TOML file is properly formatted

### Voting errors

Common issues with voting:
- Invalid proposal ID: Double-check the proposal ID
- Authentication errors: Ensure you're using the correct key file
- Node connectivity: Verify the federation node is running

### Federation node doesn't respond

If a federation node is unresponsive:
1. Check Docker container status: `docker ps | grep federation`
2. Look at container logs: `docker logs <container-id>`
3. Restart the container if needed: `docker restart <container-id>`

## Mesh Network

### Job submission issues

If job submission fails:
- Verify the manifest file format is correct
- Ensure the WASM module CID exists in the network
- Check that your key has sufficient permissions

### No bids received for jobs

If you're not receiving bids:
- Verify resource requirements aren't too strict
- Ensure there are active worker nodes in the network
- Check if your job offer is competitive

## Debugging Tips

### Verbose Logging

Add the `-v` flag to get more detailed output:
```
cargo run -p icn-cli -- -v federation submit-proposal --file proposal.toml --to http://localhost:5001
```

### Check Logs

For Docker-based nodes, check logs with:
```
docker logs <container_id>
```

### Network Connectivity

Test node connectivity:
```
curl -v http://localhost:5001/health
```

### Common Error Codes

- **401**: Authentication error - check your key
- **404**: Resource not found - check IDs and paths
- **500**: Server error - check node logs

## Still Need Help?

If you're still experiencing issues:
1. Search for the error message in the project documentation
2. Check the [Known Issues](KNOWN_ISSUES.md) file
3. Read the [DEMO_SYSTEM_SUMMARY.md](DEMO_SYSTEM_SUMMARY.md) for system architecture details 