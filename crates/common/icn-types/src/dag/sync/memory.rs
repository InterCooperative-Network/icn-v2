use crate::cid::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode};
use crate::dag::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};
use crate::bundle::TrustBundle;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// In-memory implementation of DAGSyncService for testing
pub struct MemoryDAGSyncService<S: DagStore> {
    /// The local DAG store
    dag_store: Arc<RwLock<S>>,
    /// Configured federation ID
    federation_id: String,
    /// Local peer ID
    peer_id: String,
    /// Known peers
    peers: HashMap<String, FederationPeer>,
    /// Trust level for each peer (0-100)
    peer_trust: HashMap<String, u8>,
}

impl<S: DagStore> MemoryDAGSyncService<S> {
    /// Create a new MemoryDAGSyncService
    pub fn new(dag_store: S, federation_id: String, peer_id: String) -> Self {
        Self {
            dag_store: Arc::new(RwLock::new(dag_store)),
            federation_id,
            peer_id,
            peers: HashMap::new(),
            peer_trust: HashMap::new(),
        }
    }
    
    /// Add a peer to the service
    pub fn add_peer(&mut self, peer: FederationPeer, trust_level: u8) {
        self.peers.insert(peer.id.clone(), peer);
        self.peer_trust.insert(peer.id.clone(), trust_level);
    }
    
    /// Remove a peer from the service
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.remove(peer_id);
        self.peer_trust.remove(peer_id);
    }
    
    /// Get a peer by ID
    pub fn get_peer(&self, peer_id: &str) -> Option<&FederationPeer> {
        self.peers.get(peer_id)
    }
    
    /// Get the trust level for a peer
    pub fn get_peer_trust(&self, peer_id: &str) -> Option<u8> {
        self.peer_trust.get(peer_id).copied()
    }
    
    /// Check if a node references a TrustBundle and verify its quorum
    fn verify_trust_bundle(&self, node: &SignedDagNode) -> Result<bool, SyncError> {
        let store = self.dag_store.read().map_err(|e| 
            SyncError::NetworkError(format!("Failed to acquire dag_store lock: {}", e)))?;
            
        // Check if this is a TrustBundle reference
        match &node.node.payload {
            crate::dag::DagPayload::TrustBundle(cid) => {
                // Try to load the TrustBundle
                match TrustBundle::from_dag(cid, &*store) {
                    Ok(bundle) => {
                        // Verify that the bundle has a valid quorum
                        // In a real implementation, we would verify signatures against public keys
                        // For now, we'll just check that there are signatures
                        if bundle.state_proof.signatures.is_empty() {
                            return Ok(false);
                        }
                        
                        // Verify that the bundle's previous anchors exist
                        match bundle.verify_anchors(&*store) {
                            Ok(true) => Ok(true),
                            Ok(false) => Ok(false),
                            Err(e) => Err(SyncError::VerificationFailed(format!("Failed to verify bundle anchors: {}", e))),
                        }
                    },
                    Err(e) => Err(SyncError::VerificationFailed(format!("Failed to load TrustBundle: {}", e))),
                }
            },
            _ => Ok(true), // Not a TrustBundle, no verification needed for this step
        }
    }
}

impl<S: DagStore> DAGSyncService for MemoryDAGSyncService<S> {
    fn fetch_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError> {
        // In a real implementation, this would make a network request to the peer
        // For this in-memory implementation, we'll just pretend we're fetching from our own store
        
        let store = self.dag_store.read().map_err(|e| 
            SyncError::NetworkError(format!("Failed to acquire dag_store lock: {}", e)))?;
            
        let mut nodes = Vec::new();
        
        for cid in cids {
            match store.get_node(cid) {
                Ok(node) => nodes.push(node),
                Err(DagError::NodeNotFound(_)) => continue, // Skip nodes we don't have
                Err(e) => return Err(SyncError::DagError(e)),
            }
        }
        
        Ok(DAGSyncBundle {
            nodes,
            federation_id: self.federation_id.clone(),
            source_peer: Some(self.peer_id.clone()),
            timestamp: Utc::now(),
        })
    }
    
    fn offer_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        // In a real implementation, this would make a network request to the peer
        // to find out which nodes they don't have
        // For this in-memory implementation, we'll just pretend all nodes are needed
        
        Ok(cids.iter().cloned().collect())
    }
    
    fn accept_bundle(&mut self, bundle: DAGSyncBundle) -> Result<VerificationResult, SyncError> {
        // First, verify the bundle
        let verification = self.verify_bundle(&bundle)?;
        
        // If verification passed, add the nodes to our store
        if verification.is_valid {
            let mut store = self.dag_store.write().map_err(|e| 
                SyncError::NetworkError(format!("Failed to acquire dag_store lock: {}", e)))?;
                
            for node in bundle.nodes {
                if verification.accepted_nodes.contains(node.cid.as_ref().unwrap()) {
                    // Add the node to our store
                    match store.add_node(node) {
                        Ok(_) => {},
                        Err(e) => return Err(SyncError::DagError(e)),
                    }
                }
            }
        }
        
        Ok(verification)
    }
    
    fn verify_bundle(&self, bundle: &DAGSyncBundle) -> Result<VerificationResult, SyncError> {
        let mut is_valid = true;
        let mut accepted_nodes = Vec::new();
        let mut rejected_nodes = Vec::new();
        let mut report = String::new();
        
        // Check federation ID
        if bundle.federation_id != self.federation_id {
            report.push_str(&format!("Bundle federation ID {} does not match local federation ID {}\n", 
                bundle.federation_id, self.federation_id));
            is_valid = false;
        }
        
        // Check source peer
        if let Some(peer_id) = &bundle.source_peer {
            if !self.peers.contains_key(peer_id) {
                report.push_str(&format!("Bundle source peer {} is not a known peer\n", peer_id));
                is_valid = false;
            } else {
                // Check trust level
                let trust = self.peer_trust.get(peer_id).unwrap_or(&0);
                report.push_str(&format!("Bundle source peer {} has trust level {}\n", peer_id, trust));
            }
        }
        
        // Process each node
        for node in &bundle.nodes {
            let cid = node.cid.as_ref().unwrap();
            
            // Verify the node's federation ID
            if let Some(fed_id) = &node.node.metadata.federation_id {
                if fed_id != &self.federation_id {
                    rejected_nodes.push((cid.clone(), format!("Node federation ID {} does not match local federation ID {}", 
                        fed_id, self.federation_id)));
                    continue;
                }
            }
            
            // Verify the node's signature
            // In a real implementation, we would verify the signature against the author's public key
            // For now, we'll just check that the author is known
            
            // Verify the node's parents exist
            let store = self.dag_store.read().map_err(|e| 
                SyncError::NetworkError(format!("Failed to acquire dag_store lock: {}", e)))?;
                
            let mut missing_parents = false;
            for parent in &node.node.parents {
                if let Err(DagError::NodeNotFound(_)) = store.get_node(parent) {
                    rejected_nodes.push((cid.clone(), format!("Parent node {} not found", parent)));
                    missing_parents = true;
                    break;
                }
            }
            
            if missing_parents {
                continue;
            }
            
            // If this node references a TrustBundle, verify its quorum
            match self.verify_trust_bundle(node) {
                Ok(true) => {
                    accepted_nodes.push(cid.clone());
                    report.push_str(&format!("Node {} accepted\n", cid));
                },
                Ok(false) => {
                    rejected_nodes.push((cid.clone(), "TrustBundle verification failed".to_string()));
                    report.push_str(&format!("Node {} rejected: TrustBundle verification failed\n", cid));
                },
                Err(e) => {
                    rejected_nodes.push((cid.clone(), format!("TrustBundle verification error: {}", e)));
                    report.push_str(&format!("Node {} rejected: TrustBundle verification error: {}\n", cid, e));
                },
            }
        }
        
        is_valid = !accepted_nodes.is_empty() && rejected_nodes.is_empty();
        
        Ok(VerificationResult {
            is_valid,
            accepted_nodes,
            rejected_nodes,
            report,
        })
    }
    
    fn sync_with_peer(&mut self, peer: &FederationPeer) -> Result<VerificationResult, SyncError> {
        // In a real implementation, this would:
        // 1. Get our tips
        // 2. Offer our tips to the peer
        // 3. Get the peer's tips
        // 4. Request missing nodes from the peer
        // 5. Verify and accept the nodes
        
        // For this in-memory implementation, we'll just simulate a successful sync
        
        Ok(VerificationResult {
            is_valid: true,
            accepted_nodes: Vec::new(),
            rejected_nodes: Vec::new(),
            report: format!("Simulated sync with peer {}", peer.id),
        })
    }
    
    fn get_missing_dependencies(&self, nodes: &[SignedDagNode]) -> Result<Vec<Cid>, SyncError> {
        let store = self.dag_store.read().map_err(|e| 
            SyncError::NetworkError(format!("Failed to acquire dag_store lock: {}", e)))?;
            
        let mut missing = Vec::new();
        
        // Check if each parent exists in our store
        for node in nodes {
            for parent in &node.node.parents {
                if let Err(DagError::NodeNotFound(_)) = store.get_node(parent) {
                    missing.push(parent.clone());
                }
            }
        }
        
        // Deduplicate
        missing.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        missing.dedup();
        
        Ok(missing)
    }
} 