/// Resource types that can be metered and compensated in the ICN mesh
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// CPU computation time
    CPU,
    
    /// System memory usage
    Memory,
    
    /// Disk I/O operations
    IO,
    
    /// GPU computation time
    GPU,
    
    /// Storage space consumption
    Storage,
    
    /// Network bandwidth for incoming data
    BandwidthIngress,
    
    /// Network bandwidth for outgoing data
    BandwidthEgress,
    
    /// Sensor data inputs (camera, microphone, etc.)
    SensorInput {
        /// Type of sensor being accessed
        sensor_type: String,
    },
    
    /// Environmental data collection
    EnvironmentalData {
        /// Type of environmental data being accessed
        data_type: String,
    },
    
    /// Physical actuation controls
    Actuation {
        /// Type of actuation being performed
        actuation_type: String,
    },
    
    /// Specialized hardware capabilities
    SpecializedHardware {
        /// Type of specialized hardware being used
        hardware_type: String,
    },
}

/// Resources offered by a node for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceOffer {
    /// Available CPU cores
    pub cpu_cores: u32,
    
    /// Available memory in MB
    pub memory_mb: u64,
    
    /// Available GPU models and count
    pub gpus: Option<Vec<GpuResource>>,
    
    /// Available storage in MB
    pub storage_mb: u64,
    
    /// Available bandwidth in Mbps
    pub bandwidth_mbps: u64,
    
    /// Available sensors
    pub sensors: Option<Vec<String>>,
    
    /// Available actuation capabilities
    pub actuation: Option<Vec<String>>,
    
    /// Available specialized hardware
    pub specialized_hardware: Option<Vec<String>>,
    
    /// Renewable energy percentage (0-100)
    pub renewable_energy_pct: u8,
}

/// GPU resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuResource {
    /// GPU model identifier
    pub model: String,
    
    /// Available VRAM in MB
    pub vram_mb: u64,
    
    /// Number of GPU cores/compute units
    pub cores: u32,
}

/// Resource usage record for an execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Resource type being used
    pub resource_type: ResourceType,
    
    /// Amount of resource used (units depend on resource type)
    pub amount: f64,
    
    /// Timestamp when usage was recorded
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Additional metadata about the usage
    pub metadata: Option<serde_json::Value>,
}

/// Resource compensation token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedResourceToken {
    /// Type of resource being compensated
    pub resource_type: ResourceType,
    
    /// Amount of tokens to transfer
    pub amount: f64,
    
    /// Federation ID that authorized this compensation
    pub federation_id: String,
    
    /// Reference to execution that consumed the resources
    pub execution_cid: Option<String>,
} 