use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use icn_core_types::Did;

/// Architecture type of a node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    X86_64,
    Arm64,
    RiscV32,
    RiscV64,
    WebAssembly,
    Fpga,
    #[serde(other)]
    Other,
}

// Add Default implementation for Architecture
impl Default for Architecture {
    fn default() -> Self {
        Architecture::X86_64
    }
}

/// GPU capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProfile {
    /// GPU model identifier
    pub model: String,
    
    /// GPU type/API support
    pub api: Vec<GpuApi>,
    
    /// Available VRAM in MB
    pub vram_mb: u64,
    
    /// Number of cores/compute units
    pub cores: u32,
    
    /// Whether tensor operations are supported
    pub tensor_cores: bool,
    
    /// Specific features available (e.g., "ray-tracing", "dlss")
    pub features: Vec<String>,
}

/// GPU API support type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GpuApi {
    Cuda,
    Vulkan,
    Metal,
    WebGpu,
    OpenCl,
    DirectX,
    #[serde(other)]
    Other,
}

/// Sensor capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorProfile {
    /// Sensor type identifier
    pub sensor_type: String,
    
    /// Sensor model/manufacturer
    pub model: Option<String>,
    
    /// Sensor capabilities and specifications
    pub capabilities: serde_json::Value,
    
    /// Access protocol (e.g., "v4l2", "i2c", "spi", "http")
    pub protocol: String,
    
    /// Whether the sensor is currently active
    pub active: bool,
}

/// Actuator capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actuator {
    /// Actuator type identifier (e.g., "relay", "motor", "servo")
    pub actuator_type: String,
    
    /// Actuator model/manufacturer
    pub model: Option<String>,
    
    /// Actuator capabilities and specifications
    pub capabilities: serde_json::Value,
    
    /// Control protocol (e.g., "gpio", "i2c", "pwm", "modbus")
    pub protocol: String,
    
    /// Whether the actuator is currently active
    pub active: bool,
}

/// Energy source and consumption information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyInfo {
    /// Percentage of energy from renewable sources (0-100)
    pub renewable_percentage: u8,
    
    /// Battery level percentage if applicable (0-100)
    pub battery_percentage: Option<u8>,
    
    /// Whether the device is currently charging
    pub charging: Option<bool>,
    
    /// Power consumption in watts
    pub power_consumption_watts: Option<f64>,
    
    /// Energy source details
    pub source: Vec<EnergySource>,
}

// Add Default implementation for EnergyInfo
impl Default for EnergyInfo {
    fn default() -> Self {
        EnergyInfo {
            renewable_percentage: 0,
            battery_percentage: None,
            charging: None,
            power_consumption_watts: None,
            source: vec![EnergySource::Grid],
        }
    }
}

/// Energy source type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnergySource {
    Grid,
    Solar,
    Wind,
    Battery,
    #[serde(other)]
    Other,
}

/// A signed manifest that describes a node's capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeManifest {
    /// The node's decentralized identifier
    pub did: Did,
    
    /// CPU architecture
    pub arch: Architecture,
    
    /// Number of logical CPU cores
    pub cores: u16,
    
    /// GPU profile if available
    pub gpu: Option<GpuProfile>,
    
    /// Available RAM in megabytes
    pub ram_mb: u32,
    
    /// Available storage in bytes
    pub storage_bytes: u64,
    
    /// Available sensors
    pub sensors: Vec<SensorProfile>,
    
    /// Available actuators
    pub actuators: Vec<Actuator>,
    
    /// Energy profile information
    pub energy_profile: EnergyInfo,
    
    /// Trusted firmware hash
    pub trust_fw_hash: String,
    
    /// Supported mesh protocols
    pub mesh_protocols: Vec<String>,
    
    /// Timestamp when the manifest was last updated
    pub last_seen: DateTime<Utc>,
    
    /// Signature of the manifest by the node
    pub signature: Vec<u8>,
}

impl NodeManifest {
    /// Convert the NodeManifest to a W3C Verifiable Credential
    pub fn to_verifiable_credential(&self) -> serde_json::Value {
        let subject = serde_json::json!({
            "id": self.did.to_string(),
            "type": "MeshNode",
            "architecture": self.arch,
            "cores": self.cores,
            "ramMb": self.ram_mb,
            "storageBytes": self.storage_bytes,
            "gpu": self.gpu,
            "sensors": self.sensors,
            "actuators": self.actuators,
            "energyProfile": self.energy_profile,
            "trustFirmwareHash": self.trust_fw_hash,
            "meshProtocols": self.mesh_protocols,
            "lastSeen": self.last_seen,
        });
        
        serde_json::json!({
            "@context": [
                "https://www.w3.org/2018/credentials/v1",
                "https://icn.network/context/mesh-capability/v1"
            ],
            "type": ["VerifiableCredential", "NodeManifestCredential"],
            "issuer": self.did.to_string(),
            "issuanceDate": self.last_seen,
            "credentialSubject": subject,
            // In a real implementation, we'd use a proper proof format
            "proof": {
                "type": "Ed25519Signature2020",
                "verificationMethod": format!("{}#keys-1", self.did.to_string()),
                "created": self.last_seen,
                "proofValue": hex::encode(&self.signature),
            }
        })
    }
    
    /// Create a new NodeManifest from system information
    pub fn from_system(did: Did, trust_fw_hash: &str) -> Result<Self, std::io::Error> {
        use std::io::Error;
        use std::io::ErrorKind;
        
        // Get CPU information
        let cores = num_cpus::get() as u16;
        
        // Get memory information
        let ram_mb = match sys_info::mem_info() {
            Ok(mem) => (mem.total / 1024) as u32,  // Convert KB to MB
            Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to get memory info")),
        };
        
        // Get storage information
        let storage_bytes = match sys_info::disk_info() {
            Ok(disk) => disk.total * 1024,  // Convert KB to bytes
            Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to get disk info")),
        };
        
        // Default to a minimal manifest with detected system resources
        // In a real implementation, more sophisticated detection would be used
        Ok(NodeManifest {
            did,
            arch: match std::env::consts::ARCH {
                "x86_64" => Architecture::X86_64,
                "aarch64" => Architecture::Arm64,
                "riscv32" => Architecture::RiscV32,
                "riscv64" => Architecture::RiscV64,
                "wasm32" => Architecture::WebAssembly,
                _ => Architecture::Other,
            },
            cores,
            gpu: None,  // Would be detected with a GPU library
            ram_mb,
            storage_bytes,
            sensors: Vec::new(),  // Would be detected with sensors library
            actuators: Vec::new(),  // Would be detected with GPIO/hardware library
            energy_profile: EnergyInfo {
                renewable_percentage: 0,  // Default to unknown
                battery_percentage: None,
                charging: None,
                power_consumption_watts: None,
                source: vec![EnergySource::Grid],  // Default assumption
            },
            trust_fw_hash: trust_fw_hash.to_string(),
            mesh_protocols: vec!["gossipsub".to_string(), "kademlia".to_string()],
            last_seen: Utc::now(),
            signature: Vec::new(),  // Would be signed during publication
        })
    }
}

/// CapabilitySelector allows for filtering nodes based on capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySelector {
    /// Architecture requirements (e.g., "x86_64|arm64")
    pub arch: Option<String>,
    
    /// Minimum number of cores required
    pub cores_min: Option<u16>,
    
    /// Minimum RAM required in MB
    pub ram_min: Option<u32>,
    
    /// Minimum storage required in bytes
    pub storage_min: Option<u64>,
    
    /// GPU requirements
    pub gpu: Option<GpuRequirements>,
    
    /// Required sensors by type
    pub sensors: Option<Vec<String>>,
    
    /// Required actuators by type
    pub actuators: Option<Vec<String>>,
    
    /// Minimum renewable energy percentage required
    pub energy_green_min: Option<u8>,
    
    /// Required firmware hash (for secure enclaves/attested execution)
    pub trust_fw_hash: Option<String>,
    
    /// Required mesh protocols
    pub mesh_protocols: Option<Vec<String>>,
}

/// GPU requirements for capability selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirements {
    /// Required GPU API support
    pub api: Option<Vec<String>>,
    
    /// Minimum VRAM required in MB
    pub vram_min: Option<u64>,
    
    /// Minimum number of cores required
    pub cores_min: Option<u32>,
    
    /// Whether tensor cores are required
    pub tensor_cores_required: Option<bool>,
    
    /// Required GPU features
    pub features: Option<Vec<String>>,
}

impl CapabilitySelector {
    /// Check if a node manifest matches this capability selector
    pub fn matches(&self, manifest: &NodeManifest) -> bool {
        // Architecture check
        if let Some(arch_pattern) = &self.arch {
            let node_arch = format!("{:?}", manifest.arch).to_lowercase();
            if !Self::pattern_matches(&node_arch, arch_pattern) {
                return false;
            }
        }
        
        // Cores check
        if let Some(cores_min) = self.cores_min {
            if manifest.cores < cores_min {
                return false;
            }
        }
        
        // RAM check
        if let Some(ram_min) = self.ram_min {
            if manifest.ram_mb < ram_min {
                return false;
            }
        }
        
        // Storage check
        if let Some(storage_min) = self.storage_min {
            if manifest.storage_bytes < storage_min {
                return false;
            }
        }
        
        // GPU check
        if let Some(gpu_req) = &self.gpu {
            match &manifest.gpu {
                Some(gpu) => {
                    // API check
                    if let Some(api_list) = &gpu_req.api {
                        let node_apis: Vec<String> = gpu.api.iter()
                            .map(|api| format!("{:?}", api).to_lowercase())
                            .collect();
                        
                        let matches_any_api = api_list.iter().any(|required_api| {
                            node_apis.iter().any(|node_api| Self::pattern_matches(node_api, required_api))
                        });
                        
                        if !matches_any_api {
                            return false;
                        }
                    }
                    
                    // VRAM check
                    if let Some(vram_min) = gpu_req.vram_min {
                        if gpu.vram_mb < vram_min {
                            return false;
                        }
                    }
                    
                    // Cores check
                    if let Some(cores_min) = gpu_req.cores_min {
                        if gpu.cores < cores_min {
                            return false;
                        }
                    }
                    
                    // Tensor cores check
                    if let Some(tensor_required) = gpu_req.tensor_cores_required {
                        if tensor_required && !gpu.tensor_cores {
                            return false;
                        }
                    }
                    
                    // Features check
                    if let Some(features) = &gpu_req.features {
                        for feature in features {
                            if !gpu.features.contains(feature) {
                                return false;
                            }
                        }
                    }
                },
                None => {
                    // Requires GPU but node has none
                    return false;
                }
            }
        }
        
        // Sensors check
        if let Some(required_sensors) = &self.sensors {
            let node_sensors: Vec<String> = manifest.sensors.iter()
                .map(|s| s.sensor_type.clone())
                .collect();
            
            for sensor in required_sensors {
                if !node_sensors.iter().any(|s| Self::pattern_matches(s, sensor)) {
                    return false;
                }
            }
        }
        
        // Actuators check
        if let Some(required_actuators) = &self.actuators {
            let node_actuators: Vec<String> = manifest.actuators.iter()
                .map(|a| a.actuator_type.clone())
                .collect();
            
            for actuator in required_actuators {
                if !node_actuators.iter().any(|a| Self::pattern_matches(a, actuator)) {
                    return false;
                }
            }
        }
        
        // Energy check
        if let Some(energy_min) = self.energy_green_min {
            if manifest.energy_profile.renewable_percentage < energy_min {
                return false;
            }
        }
        
        // Firmware hash check
        if let Some(required_hash) = &self.trust_fw_hash {
            if &manifest.trust_fw_hash != required_hash {
                return false;
            }
        }
        
        // Protocols check
        if let Some(required_protocols) = &self.mesh_protocols {
            for protocol in required_protocols {
                if !manifest.mesh_protocols.contains(protocol) {
                    return false;
                }
            }
        }
        
        // All checks passed
        true
    }
    
    /// Helper to check if a value matches a pattern (supports | for alternatives)
    fn pattern_matches(value: &str, pattern: &str) -> bool {
        pattern.split('|').any(|p| value == p.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pattern_matching() {
        assert!(CapabilitySelector::pattern_matches("x86_64", "x86_64"));
        assert!(CapabilitySelector::pattern_matches("x86_64", "x86_64|arm64"));
        assert!(CapabilitySelector::pattern_matches("arm64", "x86_64|arm64"));
        assert!(!CapabilitySelector::pattern_matches("riscv64", "x86_64|arm64"));
    }
    
    #[test]
    fn test_capability_matching() {
        let manifest = NodeManifest {
            did: Did::from("did:icn:test".to_string()),
            arch: Architecture::X86_64,
            cores: 8,
            gpu: Some(GpuProfile {
                model: "NVIDIA RTX 3080".to_string(),
                api: vec![GpuApi::Cuda, GpuApi::Vulkan],
                vram_mb: 10240,
                cores: 8704,
                tensor_cores: true,
                features: vec!["ray-tracing".to_string(), "dlss".to_string()],
            }),
            ram_mb: 32768,
            storage_bytes: 1024 * 1024 * 1024 * 500, // 500 GB
            sensors: vec![
                SensorProfile {
                    sensor_type: "camera".to_string(),
                    model: Some("Logitech C920".to_string()),
                    capabilities: serde_json::json!({"resolution": "1080p"}),
                    protocol: "v4l2".to_string(),
                    active: true,
                }
            ],
            actuators: vec![],
            energy_profile: EnergyInfo {
                renewable_percentage: 80,
                battery_percentage: None,
                charging: None,
                power_consumption_watts: Some(150.0),
                source: vec![EnergySource::Grid, EnergySource::Solar],
            },
            trust_fw_hash: "abcdef123456".to_string(),
            mesh_protocols: vec!["gossipsub".to_string(), "kademlia".to_string()],
            last_seen: Utc::now(),
            signature: vec![],
        };
        
        // Matching selector
        let matching_selector = CapabilitySelector {
            arch: Some("x86_64".to_string()),
            cores_min: Some(4),
            ram_min: Some(16384),
            storage_min: Some(1024 * 1024 * 1024 * 100), // 100 GB
            gpu: Some(GpuRequirements {
                api: Some(vec!["cuda".to_string()]),
                vram_min: Some(8192),
                cores_min: Some(5000),
                tensor_cores_required: Some(true),
                features: Some(vec!["ray-tracing".to_string()]),
            }),
            sensors: Some(vec!["camera".to_string()]),
            actuators: None,
            energy_green_min: Some(75),
            trust_fw_hash: None,
            mesh_protocols: Some(vec!["gossipsub".to_string()]),
        };
        
        // Non-matching selector
        let non_matching_selector = CapabilitySelector {
            arch: Some("arm64".to_string()),
            cores_min: Some(16),
            ram_min: None,
            storage_min: None,
            gpu: None,
            sensors: None,
            actuators: None,
            energy_green_min: None,
            trust_fw_hash: None,
            mesh_protocols: None,
        };
        
        assert!(matching_selector.matches(&manifest));
        assert!(!non_matching_selector.matches(&manifest));
    }
} 