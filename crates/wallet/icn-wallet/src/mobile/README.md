# ICN Wallet Mobile App

A React Native mobile application for verifying ICN dispatch credentials and managing trust policies.

## Features

- ✅ **Import Credentials** via QR code, file, or deep links
- ✅ **Verify Credentials** using the Rust verification engine
- ✅ **View Verification Status** with detailed reports
- ✅ **Share Credentials** via deep links
- ✅ **Manage Trust Policies** (view and select active policies)

## Setup

### Prerequisites

- Node.js 14+
- React Native development environment
- Rust toolchain
- UniFFI tools

### Building the Rust Library

First, build the Rust library with UniFFI bindings:

```bash
cd crates/wallet/icn-wallet
cargo build --features uniffi-bindings
```

### Generate Mobile Bindings

After building the Rust library with UniFFI bindings, generate the platform-specific bindings:

```bash
# Generate Kotlin bindings
cd crates/wallet/icn-wallet
uniffi-bindgen generate src/icn-wallet.udl --language kotlin --out-dir mobile-bindings/kotlin

# Generate Swift bindings
uniffi-bindgen generate src/icn-wallet.udl --language swift --out-dir mobile-bindings/swift
```

### Install JavaScript Dependencies

```bash
cd crates/wallet/icn-wallet/src/mobile
npm install
# or 
yarn install
```

### Running the App

```bash
# iOS
npx react-native run-ios

# Android
npx react-native run-android
```

## Deep Link Support

The app supports deep links in the format:

```
icn://dispatch?credential=<base64-encoded-credential-json>
```

## App Structure

- `App.tsx` - Main application entry point
- `wallet_screen.tsx` - Main wallet UI with tabs
- `verification_card.tsx` - Credential verification display
- `credential_import.tsx` - QR/file/link import functionality
- `bindings/` - FFI bindings to Rust verification library

## Integration with Core Rust SDK

The app uses the UniFFI-generated bindings to call the Rust verification functions from `icn-wallet`. This provides the same cryptographic verification on mobile as is available in the CLI and server applications.

## Screenshots

[Screenshots will be added here]

## License

MIT 