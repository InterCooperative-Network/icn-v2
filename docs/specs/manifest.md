# ICN Node Manifest Specification

## Overview

The Node Manifest is a self-describing, signed declaration of a node's capabilities in the InterCooperative Network (ICN) mesh. It serves as a "digital résumé" for any device participating in the planetary-scale compute fabric, enabling intelligent task scheduling, resource pricing, and trust establishment.

## Core Principles

1. **Self-sovereign identification**: Each node presents its own capabilities using a DID-anchored manifest.
2. **Cryptographic verification**: Manifests are signed by the node itself and can be countersigned by trusted parties.
3. **Capability advertising**: Manifests detail all resources a node has available for the mesh.
4. **Dynamic discovery**: Manifests are published to the mesh for discovery by schedulers and other nodes.
5. **Verifiable Credentials format**: Using the W3C VC standard for compatibility and extensibility.

## Manifest Structure

The Node Manifest contains the following core fields:

| Field | Type | Description |
|-------|------|-------------|
| `did` | `Did` | The decentralized identifier of the node |
| `arch` | `Architecture` | CPU architecture (x86_64, arm64, riscv32, etc.) |
| `cores` | `u16` | Number of logical CPU cores |
| `gpu` | `Option<GpuProfile>` | Detailed GPU information if available |
| `ram_mb` | `u32` | Available RAM in megabytes |
| `storage_bytes` | `u64` | Available storage in bytes |
| `sensors` | `Vec<SensorProfile>` | List of available sensors |
| `actuators` | `Vec<Actuator>` | List of available actuators |
| `energy_profile` | `EnergyInfo` | Energy source and consumption information |
| `trust_fw_hash` | `String` | Hash of the trusted firmware for attestation |
| `mesh_protocols` | `Vec<String>` | Supported mesh protocols (gossipsub, kademlia, etc.) |
| `last_seen` | `DateTime<Utc>` | Timestamp when the manifest was last updated |
| `signature` | `Vec<u8>` | Signature of the manifest by the node |

### GPU Profile

| Field | Type | Description |
|-------|------|-------------|
| `model` | `String` | GPU model identifier |
| `api` | `Vec<GpuApi>` | Supported APIs (CUDA, Vulkan, WebGPU, etc.) |
| `vram_mb` | `u64` | Available VRAM in megabytes |
| `cores` | `u32` | Number of GPU cores/compute units |
| `tensor_cores` | `bool` | Whether tensor operations are supported |
| `features` | `Vec<String>` | Specific features available |

### Sensor Profile

| Field | Type | Description |
|-------|------|-------------|
| `sensor_type` | `String` | Type of sensor (camera, microphone, etc.) |
| `model` | `Option<String>` | Sensor model/manufacturer |
| `capabilities` | `serde_json::Value` | Detailed sensor specifications |
| `protocol` | `String` | Access protocol (v4l2, i2c, etc.) |
| `active` | `bool` | Whether the sensor is currently active |

### Actuator Profile

| Field | Type | Description |
|-------|------|-------------|
| `actuator_type` | `String` | Type of actuator (relay, motor, etc.) |
| `model` | `Option<String>` | Actuator model/manufacturer |
| `capabilities` | `serde_json::Value` | Detailed actuator specifications |
| `protocol` | `String` | Control protocol (gpio, pwm, etc.) |
| `active` | `bool` | Whether the actuator is currently active |

### Energy Information

| Field | Type | Description |
|-------|------|-------------|
| `renewable_percentage` | `u8` | Percentage of energy from renewable sources |
| `battery_percentage` | `Option<u8>` | Battery level if applicable |
| `charging` | `Option<bool>` | Whether the device is charging |
| `power_consumption_watts` | `Option<f64>` | Power consumption in watts |
| `source` | `Vec<EnergySource>` | Energy sources (grid, solar, wind, etc.) |

## Verifiable Credential Format

The Node Manifest is represented as a W3C Verifiable Credential with the following structure:

```json
{
  "@context": [
    "https://www.w3.org/2018/credentials/v1",
    "https://icn.network/context/mesh-capability/v1"
  ],
  "type": ["VerifiableCredential", "NodeManifestCredential"],
  "issuer": "did:icn:node123",
  "issuanceDate": "2023-06-15T08:20:00Z",
  "credentialSubject": {
    "id": "did:icn:node123",
    "type": "MeshNode",
    "architecture": "x86_64",
    "cores": 8,
    "ramMb": 32768,
    "storageBytes": 536870912000,
    "gpu": {
      "model": "NVIDIA RTX 3080",
      "api": ["cuda", "vulkan"],
      "vram_mb": 10240,
      "cores": 8704,
      "tensor_cores": true,
      "features": ["ray-tracing", "dlss"]
    },
    "sensors": [
      {
        "sensor_type": "camera",
        "model": "Logitech C920",
        "capabilities": {"resolution": "1080p"},
        "protocol": "v4l2",
        "active": true
      }
    ],
    "actuators": [],
    "energyProfile": {
      "renewable_percentage": 80,
      "power_consumption_watts": 150.0,
      "source": ["grid", "solar"]
    },
    "trustFirmwareHash": "abcdef123456",
    "meshProtocols": ["gossipsub", "kademlia"],
    "lastSeen": "2023-06-15T08:20:00Z"
  },
  "proof": {
    "type": "Ed25519Signature2020",
    "verificationMethod": "did:icn:node123#keys-1",
    "created": "2023-06-15T08:20:00Z",
    "proofValue": "z3FLC2EP...AEDsiUJ"
  }
}
```

## Manifest Publishing and Discovery

1. **Generation**: A node generates its manifest on startup by detecting its available resources.
2. **Signing**: The node signs the manifest with its private key.
3. **DAG Anchoring**: The manifest is anchored to the ICN DAG for persistent storage.
4. **Gossip**: The node publishes its manifest CID to a "Capabilities" gossipsub topic.
5. **Discovery**: Schedulers subscribe to the topic, collect manifests, and index them by capability.

## Capability Selection

Task requirements are expressed as a `CapabilitySelector` that can match against node manifests:

```toml
[task.requirements]
arch = "x86_64|arm64"
cores_min = 4
ram_min = 16384  # 16 GB
storage_min = 107374182400  # 100 GB
gpu.api = ["cuda"]
gpu.vram_min = 8192  # 8 GB
gpu.cores_min = 5000
gpu.tensor_cores_required = true
gpu.features = ["ray-tracing"]
sensors = ["camera"]
energy_green_min = 75  # At least 75% renewable energy
mesh_protocols = ["gossipsub"]
```

Schedulers use these selectors to filter available nodes and match tasks to appropriate resources.

## Remote Attestation

For secure enclaves and trusted execution environments, the node manifest can include attestation evidence:

1. **Firmware Hash**: A cryptographic hash of the node's firmware or secure boot state.
2. **TEE Evidence**: For platforms with hardware security (TPM, SGX, TrustZone), attestation reports.
3. **Countersignatures**: Trusted third parties can countersign the manifest to vouch for its authenticity.

## Manifest Revocation

Manifests can be revoked or updated:

1. **Expiration**: Manifests include a timestamp and can be considered stale after a certain period.
2. **Explicit Revocation**: Nodes can publish a revocation credential to invalidate a previous manifest.
3. **Superseding**: Publishing a new manifest implicitly supersedes older ones.

## Economic Implications

The manifest enables fine-grained economic models:

1. **Resource-Specific Pricing**: Federations can price each resource type differently.
2. **Capability Bonuses**: Rare resources (GPUs, specialized sensors) can receive higher compensation.
3. **Green Energy Incentives**: Nodes with high renewable energy percentages can receive bonuses.
4. **Reputation Integration**: Manifest claims are verified against actual performance to build reputation.

## Security Considerations

1. **Manifest Verification**: Schedulers must verify manifest signatures before trusting claims.
2. **Capability Verification**: Runtime environments should verify claimed capabilities at execution time.
3. **Sybil Attack Prevention**: Federation policies should include mechanisms to prevent fake capability claims.
4. **Privacy**: Manifests should only include necessary information to protect node privacy.

## Example Implementation

```rust
// Create a manifest from system information
let did = Did::from("did:icn:node123".to_string());
let manifest = NodeManifest::from_system(did, "firmware-hash-here").unwrap();

// Convert to a verifiable credential
let vc = manifest.to_verifiable_credential();

// Publish to the gossipsub topic
mesh.publish("mesh-capabilities", vc.to_string()).await?;
```

## References

1. W3C Verifiable Credentials Data Model: https://www.w3.org/TR/vc-data-model/
2. ICN Mesh Protocol Specification
3. ICN DAG Storage Specification
4. W3C Decentralized Identifiers (DIDs): https://www.w3.org/TR/did-core/ 