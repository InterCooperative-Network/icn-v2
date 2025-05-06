use icn_identity_core::manifest::{
    NodeManifest, Architecture, GpuApi, EnergySource
};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Selector for filtering nodes by their capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitySelector {
    /// Required CPU architecture
    pub arch: Option<Architecture>,
    
    /// Minimum number of CPU cores
    pub min_cores: Option<u16>,
    
    /// Minimum RAM in megabytes
    pub min_ram_mb: Option<u32>,
    
    /// Minimum storage in bytes
    pub min_storage_bytes: Option<u64>,
    
    /// GPU requirements
    pub gpu_requirements: Option<GpuRequirements>,
    
    /// Required sensor types
    pub sensor_requirements: Option<Vec<SensorRequirement>>,
    
    /// Required actuator types
    pub actuator_requirements: Option<Vec<ActuatorRequirement>>,
    
    /// Energy profile requirements
    pub energy_requirements: Option<EnergyRequirements>,
    
    /// Additional requirements as key-value pairs
    pub extensions: HashMap<String, String>,
}

/// GPU capability requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirements {
    /// Minimum VRAM in megabytes
    pub min_vram_mb: Option<u64>,
    
    /// Minimum number of GPU cores
    pub min_cores: Option<u32>,
    
    /// Whether tensor cores are required
    pub requires_tensor_cores: bool,
    
    /// Required GPU APIs
    pub required_api: Option<Vec<GpuApi>>,
    
    /// Required GPU features
    pub required_features: Option<Vec<String>>,
}

/// Sensor capability requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorRequirement {
    /// Required sensor type
    pub sensor_type: String,
    
    /// Required sensor protocol
    pub protocol: Option<String>,
    
    /// Whether the sensor must be active
    pub must_be_active: bool,
}

/// Actuator capability requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorRequirement {
    /// Required actuator type
    pub actuator_type: String,
    
    /// Required actuator protocol
    pub protocol: Option<String>,
    
    /// Whether the actuator must be active
    pub must_be_active: bool,
}

/// Energy requirements for node selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyRequirements {
    /// Minimum renewable energy percentage
    pub min_renewable_percentage: Option<u8>,
    
    /// Required energy sources
    pub required_sources: Option<Vec<EnergySource>>,
    
    /// Whether the node must be on battery power
    pub requires_battery: bool,
    
    /// Whether the node must be charging
    pub requires_charging: bool,
    
    /// Maximum power consumption in watts
    pub max_power_consumption: Option<f64>,
}

impl CapabilitySelector {
    /// Create a new empty capability selector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if a node manifest matches all the requirements
    pub fn matches(&self, manifest: &NodeManifest) -> bool {
        // Check architecture requirement
        if let Some(required_arch) = &self.arch {
            if &manifest.arch != required_arch {
                return false;
            }
        }
        
        // Check core count requirement
        if let Some(min_cores) = self.min_cores {
            if manifest.cores < min_cores {
                return false;
            }
        }
        
        // Check RAM requirement
        if let Some(min_ram) = self.min_ram_mb {
            if manifest.ram_mb < min_ram {
                return false;
            }
        }
        
        // Check storage requirement
        if let Some(min_storage) = self.min_storage_bytes {
            if manifest.storage_bytes < min_storage {
                return false;
            }
        }
        
        // Check GPU requirements
        if let Some(gpu_reqs) = &self.gpu_requirements {
            if !self.matches_gpu(manifest, gpu_reqs) {
                return false;
            }
        }
        
        // Check sensor requirements
        if let Some(sensor_reqs) = &self.sensor_requirements {
            if !self.matches_sensors(manifest, sensor_reqs) {
                return false;
            }
        }
        
        // Check actuator requirements
        if let Some(actuator_reqs) = &self.actuator_requirements {
            if !self.matches_actuators(manifest, actuator_reqs) {
                return false;
            }
        }
        
        // Check energy requirements
        if let Some(energy_reqs) = &self.energy_requirements {
            if !self.matches_energy(manifest, energy_reqs) {
                return false;
            }
        }
        
        // All requirements passed
        true
    }
    
    /// Check if a node manifest's GPU capabilities match the requirements
    fn matches_gpu(&self, manifest: &NodeManifest, requirements: &GpuRequirements) -> bool {
        // If GPU is required but not present, fail
        if requirements.min_vram_mb.is_some() || 
           requirements.min_cores.is_some() || 
           requirements.requires_tensor_cores || 
           requirements.required_api.is_some() || 
           requirements.required_features.is_some() {
            // If no GPU in manifest, requirements can't be met
            if manifest.gpu.is_none() {
                return false;
            }
        } else {
            // No specific GPU requirements
            return true;
        }
        
        let gpu = manifest.gpu.as_ref().unwrap();
        
        // Check VRAM requirement
        if let Some(min_vram) = requirements.min_vram_mb {
            if gpu.vram_mb < min_vram {
                return false;
            }
        }
        
        // Check GPU cores requirement
        if let Some(min_cores) = requirements.min_cores {
            if gpu.cores < min_cores {
                return false;
            }
        }
        
        // Check tensor cores requirement
        if requirements.requires_tensor_cores && !gpu.tensor_cores {
            return false;
        }
        
        // Check required APIs
        if let Some(required_apis) = &requirements.required_api {
            for required_api in required_apis {
                if !gpu.api.contains(required_api) {
                    return false;
                }
            }
        }
        
        // Check required features
        if let Some(required_features) = &requirements.required_features {
            for required_feature in required_features {
                if !gpu.features.iter().any(|f| f == required_feature) {
                    return false;
                }
            }
        }
        
        true
    }
    
    /// Check if a node manifest's sensor capabilities match the requirements
    fn matches_sensors(&self, manifest: &NodeManifest, requirements: &[SensorRequirement]) -> bool {
        for requirement in requirements {
            let matching_sensor = manifest.sensors.iter().find(|s| {
                // Check sensor type
                if s.sensor_type != requirement.sensor_type {
                    return false;
                }
                
                // Check protocol if specified
                if let Some(protocol) = &requirement.protocol {
                    if &s.protocol != protocol {
                        return false;
                    }
                }
                
                // Check if must be active
                if requirement.must_be_active && !s.active {
                    return false;
                }
                
                true
            });
            
            if matching_sensor.is_none() {
                return false;
            }
        }
        
        true
    }
    
    /// Check if a node manifest's actuator capabilities match the requirements
    fn matches_actuators(&self, manifest: &NodeManifest, requirements: &[ActuatorRequirement]) -> bool {
        for requirement in requirements {
            let matching_actuator = manifest.actuators.iter().find(|a| {
                // Check actuator type
                if a.actuator_type != requirement.actuator_type {
                    return false;
                }
                
                // Check protocol if specified
                if let Some(protocol) = &requirement.protocol {
                    if &a.protocol != protocol {
                        return false;
                    }
                }
                
                // Check if must be active
                if requirement.must_be_active && !a.active {
                    return false;
                }
                
                true
            });
            
            if matching_actuator.is_none() {
                return false;
            }
        }
        
        true
    }
    
    /// Check if a node manifest's energy profile matches the requirements
    fn matches_energy(&self, manifest: &NodeManifest, requirements: &EnergyRequirements) -> bool {
        // Check renewable percentage
        if let Some(min_renewable) = requirements.min_renewable_percentage {
            if manifest.energy_profile.renewable_percentage < min_renewable {
                return false;
            }
        }
        
        // Check required energy sources
        if let Some(required_sources) = &requirements.required_sources {
            for source in required_sources {
                if !manifest.energy_profile.source.contains(source) {
                    return false;
                }
            }
        }
        
        // Check battery requirement
        if requirements.requires_battery {
            if manifest.energy_profile.battery_percentage.is_none() {
                return false;
            }
        }
        
        // Check charging requirement
        if requirements.requires_charging {
            match manifest.energy_profile.charging {
                Some(is_charging) if is_charging => {} // Good, it's charging
                _ => return false, // Not charging or unknown
            }
        }
        
        // Check power consumption
        if let Some(max_power) = requirements.max_power_consumption {
            if let Some(consumption) = manifest.energy_profile.power_consumption_watts {
                if consumption > max_power {
                    return false;
                }
            }
        }
        
        true
    }
    
    /// Parse a key=value string into a capability requirement
    pub fn parse_requirement(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "arch" => {
                self.arch = Some(match value.to_lowercase().as_str() {
                    "x86_64" => Architecture::X86_64,
                    "arm64" => Architecture::Arm64,
                    "riscv32" => Architecture::RiscV32,
                    "riscv64" => Architecture::RiscV64,
                    "wasm32" => Architecture::WebAssembly,
                    _ => return Err(format!("Unknown architecture: {}", value)),
                });
            },
            "min_cores" => {
                self.min_cores = Some(value.parse::<u16>()
                    .map_err(|_| format!("Invalid core count: {}", value))?);
            },
            "min_ram_mb" => {
                self.min_ram_mb = Some(value.parse::<u32>()
                    .map_err(|_| format!("Invalid RAM value: {}", value))?);
            },
            "min_storage_gb" => {
                let gb = value.parse::<u64>()
                    .map_err(|_| format!("Invalid storage value: {}", value))?;
                self.min_storage_bytes = Some(gb * 1024 * 1024 * 1024);
            },
            "gpu_vram_mb" => {
                let vram = value.parse::<u64>()
                    .map_err(|_| format!("Invalid GPU VRAM value: {}", value))?;
                
                self.gpu_requirements.get_or_insert_with(|| GpuRequirements {
                    min_vram_mb: None,
                    min_cores: None,
                    requires_tensor_cores: false,
                    required_api: None,
                    required_features: None,
                }).min_vram_mb = Some(vram);
            },
            "gpu_cores" => {
                let cores = value.parse::<u32>()
                    .map_err(|_| format!("Invalid GPU core count: {}", value))?;
                
                self.gpu_requirements.get_or_insert_with(|| GpuRequirements {
                    min_vram_mb: None,
                    min_cores: None,
                    requires_tensor_cores: false,
                    required_api: None,
                    required_features: None,
                }).min_cores = Some(cores);
            },
            "gpu_tensor_cores" => {
                let required = value.parse::<bool>()
                    .map_err(|_| format!("Invalid tensor cores value (expected true/false): {}", value))?;
                
                self.gpu_requirements.get_or_insert_with(|| GpuRequirements {
                    min_vram_mb: None,
                    min_cores: None,
                    requires_tensor_cores: false,
                    required_api: None,
                    required_features: None,
                }).requires_tensor_cores = required;
            },
            "gpu_api" => {
                let api = match value.to_lowercase().as_str() {
                    "cuda" => GpuApi::Cuda,
                    "vulkan" => GpuApi::Vulkan,
                    "metal" => GpuApi::Metal,
                    "webgpu" => GpuApi::WebGpu,
                    "opencl" => GpuApi::OpenCl,
                    "directx" => GpuApi::DirectX,
                    _ => return Err(format!("Unknown GPU API: {}", value)),
                };
                
                let gpu_reqs = self.gpu_requirements.get_or_insert_with(|| GpuRequirements {
                    min_vram_mb: None,
                    min_cores: None,
                    requires_tensor_cores: false,
                    required_api: None,
                    required_features: None,
                });
                
                let apis = gpu_reqs.required_api.get_or_insert_with(Vec::new);
                apis.push(api);
            },
            "gpu_feature" => {
                let gpu_reqs = self.gpu_requirements.get_or_insert_with(|| GpuRequirements {
                    min_vram_mb: None,
                    min_cores: None,
                    requires_tensor_cores: false,
                    required_api: None,
                    required_features: None,
                });
                
                let features = gpu_reqs.required_features.get_or_insert_with(Vec::new);
                features.push(value.to_string());
            },
            "sensor" => {
                let parts: Vec<&str> = value.split(':').collect();
                let sensor_type = parts[0].to_string();
                let protocol = if parts.len() > 1 { Some(parts[1].to_string()) } else { None };
                let must_be_active = if parts.len() > 2 { 
                    parts[2].parse::<bool>().unwrap_or(true) 
                } else { 
                    true 
                };
                
                let sensors = self.sensor_requirements.get_or_insert_with(Vec::new);
                sensors.push(SensorRequirement {
                    sensor_type,
                    protocol,
                    must_be_active,
                });
            },
            "actuator" => {
                let parts: Vec<&str> = value.split(':').collect();
                let actuator_type = parts[0].to_string();
                let protocol = if parts.len() > 1 { Some(parts[1].to_string()) } else { None };
                let must_be_active = if parts.len() > 2 { 
                    parts[2].parse::<bool>().unwrap_or(true) 
                } else { 
                    true 
                };
                
                let actuators = self.actuator_requirements.get_or_insert_with(Vec::new);
                actuators.push(ActuatorRequirement {
                    actuator_type,
                    protocol,
                    must_be_active,
                });
            },
            "min_renewable" => {
                let percentage = value.parse::<u8>()
                    .map_err(|_| format!("Invalid renewable percentage: {}", value))?;
                
                let energy = self.energy_requirements.get_or_insert_with(|| EnergyRequirements {
                    min_renewable_percentage: None,
                    required_sources: None,
                    requires_battery: false,
                    requires_charging: false,
                    max_power_consumption: None,
                });
                
                energy.min_renewable_percentage = Some(percentage);
            },
            "energy_source" => {
                let source = match value.to_lowercase().as_str() {
                    "grid" => EnergySource::Grid,
                    "solar" => EnergySource::Solar,
                    "wind" => EnergySource::Wind,
                    "battery" => EnergySource::Battery,
                    "generator" => EnergySource::Generator,
                    _ => return Err(format!("Unknown energy source: {}", value)),
                };
                
                let energy = self.energy_requirements.get_or_insert_with(|| EnergyRequirements {
                    min_renewable_percentage: None,
                    required_sources: None,
                    requires_battery: false,
                    requires_charging: false,
                    max_power_consumption: None,
                });
                
                let sources = energy.required_sources.get_or_insert_with(Vec::new);
                sources.push(source);
            },
            "requires_battery" => {
                let required = value.parse::<bool>()
                    .map_err(|_| format!("Invalid battery requirement (expected true/false): {}", value))?;
                
                let energy = self.energy_requirements.get_or_insert_with(|| EnergyRequirements {
                    min_renewable_percentage: None,
                    required_sources: None,
                    requires_battery: false,
                    requires_charging: false,
                    max_power_consumption: None,
                });
                
                energy.requires_battery = required;
            },
            "requires_charging" => {
                let required = value.parse::<bool>()
                    .map_err(|_| format!("Invalid charging requirement (expected true/false): {}", value))?;
                
                let energy = self.energy_requirements.get_or_insert_with(|| EnergyRequirements {
                    min_renewable_percentage: None,
                    required_sources: None,
                    requires_battery: false,
                    requires_charging: false,
                    max_power_consumption: None,
                });
                
                energy.requires_charging = required;
            },
            "max_power_watts" => {
                let watts = value.parse::<f64>()
                    .map_err(|_| format!("Invalid power consumption value: {}", value))?;
                
                let energy = self.energy_requirements.get_or_insert_with(|| EnergyRequirements {
                    min_renewable_percentage: None,
                    required_sources: None,
                    requires_battery: false,
                    requires_charging: false,
                    max_power_consumption: None,
                });
                
                energy.max_power_consumption = Some(watts);
            },
            _ => {
                // Store unknown requirements as extensions
                self.extensions.insert(key.to_string(), value.to_string());
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    fn create_test_manifest() -> NodeManifest {
        NodeManifest {
            did: "did:icn:test".into(),
            arch: Architecture::X86_64,
            cores: 8,
            gpu: Some(icn_identity_core::manifest::GpuProfile {
                model: "Test GPU".to_string(),
                api: vec![GpuApi::Cuda, GpuApi::Vulkan],
                vram_mb: 8192,
                cores: 4096,
                tensor_cores: true,
                features: vec!["ray-tracing".to_string(), "ai-acceleration".to_string()],
            }),
            ram_mb: 16384,
            storage_bytes: 1_000_000_000_000, // 1TB
            sensors: vec![
                icn_identity_core::manifest::SensorProfile {
                    sensor_type: "temperature".to_string(),
                    model: Some("DHT22".to_string()),
                    protocol: "i2c".to_string(),
                    capabilities: serde_json::json!({"accuracy": 0.5}),
                    active: true,
                },
                icn_identity_core::manifest::SensorProfile {
                    sensor_type: "camera".to_string(),
                    model: Some("IMX477".to_string()),
                    protocol: "csi".to_string(),
                    capabilities: serde_json::json!({"resolution": "4056x3040"}),
                    active: true,
                },
            ],
            actuators: vec![
                icn_identity_core::manifest::Actuator {
                    actuator_type: "motor".to_string(),
                    model: Some("NEMA17".to_string()),
                    protocol: "gpio".to_string(),
                    capabilities: serde_json::json!({"steps": 200}),
                    active: true,
                },
            ],
            energy_profile: icn_identity_core::manifest::EnergyInfo {
                renewable_percentage: 75,
                battery_percentage: Some(80),
                charging: Some(true),
                power_consumption_watts: Some(45.5),
                source: vec![EnergySource::Solar, EnergySource::Battery],
            },
            trust_fw_hash: "test-hash".to_string(),
            mesh_protocols: vec!["gossipsub".to_string()],
            last_seen: Utc::now(),
            signature: vec![],
        }
    }
    
    #[test]
    fn test_matches_architecture() {
        let manifest = create_test_manifest();
        
        let mut selector = CapabilitySelector::new();
        selector.arch = Some(Architecture::X86_64);
        assert!(selector.matches(&manifest));
        
        selector.arch = Some(Architecture::Arm64);
        assert!(!selector.matches(&manifest));
    }
    
    #[test]
    fn test_matches_resources() {
        let manifest = create_test_manifest();
        
        let mut selector = CapabilitySelector::new();
        selector.min_cores = Some(4);
        selector.min_ram_mb = Some(8192);
        selector.min_storage_bytes = Some(500_000_000_000);
        assert!(selector.matches(&manifest));
        
        selector.min_cores = Some(16);
        assert!(!selector.matches(&manifest));
    }
    
    #[test]
    fn test_matches_gpu() {
        let manifest = create_test_manifest();
        
        let mut selector = CapabilitySelector::new();
        selector.gpu_requirements = Some(GpuRequirements {
            min_vram_mb: Some(4096),
            min_cores: Some(2048),
            requires_tensor_cores: true,
            required_api: Some(vec![GpuApi::Cuda]),
            required_features: Some(vec!["ray-tracing".to_string()]),
        });
        assert!(selector.matches(&manifest));
        
        selector.gpu_requirements = Some(GpuRequirements {
            min_vram_mb: Some(16384), // Too much VRAM required
            min_cores: Some(2048),
            requires_tensor_cores: true,
            required_api: Some(vec![GpuApi::Cuda]),
            required_features: Some(vec!["ray-tracing".to_string()]),
        });
        assert!(!selector.matches(&manifest));
    }
    
    #[test]
    fn test_matches_sensors() {
        let manifest = create_test_manifest();
        
        let mut selector = CapabilitySelector::new();
        selector.sensor_requirements = Some(vec![
            SensorRequirement {
                sensor_type: "temperature".to_string(),
                protocol: Some("i2c".to_string()),
                must_be_active: true,
            }
        ]);
        assert!(selector.matches(&manifest));
        
        selector.sensor_requirements = Some(vec![
            SensorRequirement {
                sensor_type: "humidity".to_string(), // Not available
                protocol: None,
                must_be_active: true,
            }
        ]);
        assert!(!selector.matches(&manifest));
    }
    
    #[test]
    fn test_matches_combined() {
        let manifest = create_test_manifest();
        
        let mut selector = CapabilitySelector::new();
        selector.arch = Some(Architecture::X86_64);
        selector.min_cores = Some(4);
        selector.gpu_requirements = Some(GpuRequirements {
            min_vram_mb: Some(4096),
            min_cores: None,
            requires_tensor_cores: true,
            required_api: Some(vec![GpuApi::Cuda]),
            required_features: None,
        });
        selector.energy_requirements = Some(EnergyRequirements {
            min_renewable_percentage: Some(70),
            required_sources: Some(vec![EnergySource::Solar]),
            requires_battery: true,
            requires_charging: true,
            max_power_consumption: Some(50.0),
        });
        
        assert!(selector.matches(&manifest));
    }
    
    #[test]
    fn test_parse_requirements() {
        let mut selector = CapabilitySelector::new();
        
        assert!(selector.parse_requirement("arch", "x86_64").is_ok());
        assert!(selector.parse_requirement("min_cores", "4").is_ok());
        assert!(selector.parse_requirement("min_ram_mb", "8192").is_ok());
        assert!(selector.parse_requirement("gpu_vram_mb", "4096").is_ok());
        assert!(selector.parse_requirement("gpu_tensor_cores", "true").is_ok());
        assert!(selector.parse_requirement("gpu_api", "cuda").is_ok());
        assert!(selector.parse_requirement("min_renewable", "70").is_ok());
        
        assert_eq!(selector.arch, Some(Architecture::X86_64));
        assert_eq!(selector.min_cores, Some(4));
        assert_eq!(selector.min_ram_mb, Some(8192));
        assert!(selector.gpu_requirements.is_some());
        assert_eq!(selector.gpu_requirements.as_ref().unwrap().min_vram_mb, Some(4096));
        assert!(selector.gpu_requirements.as_ref().unwrap().requires_tensor_cores);
        assert_eq!(
            selector.gpu_requirements.as_ref().unwrap().required_api.as_ref().unwrap()[0], 
            GpuApi::Cuda
        );
        assert_eq!(
            selector.energy_requirements.as_ref().unwrap().min_renewable_percentage, 
            Some(70)
        );
    }
} 