use sled::{Db, IVec};
use icn_types::dag::{DagNode, NodeScope};
use icn_identity_core::Did;
use icn_core_types::Cid;
use serde::{Serialize, Deserialize};
use bincode;
use std::collections::HashMap; // This import seems unused in the provided scaffold, will keep for now.

#[derive(Debug)]
pub enum IndexError {
    SledError(sled::Error),
    SerializeError(bincode::Error),
    NotFound, // Should this be used in current methods or is it for future use?
}

impl From<sled::Error> for IndexError {
    fn from(e: sled::Error) -> Self {
        IndexError::SledError(e)
    }
}

impl From<bincode::Error> for IndexError {
    fn from(e: bincode::Error) -> Self {
        IndexError::SerializeError(e)
    }
}

pub trait DagIndex {
    fn add_node_to_index(&self, cid: &Cid, metadata_provider: &DagNode) -> Result<(), IndexError>;
    fn nodes_by_did(&self, did: &Did) -> Result<Vec<Cid>, IndexError>;
    fn nodes_by_scope(&self, scope: &NodeScope) -> Result<Vec<Cid>, IndexError>;
}

pub struct SledDagIndex {
    db: Db,
}

impl SledDagIndex {
    pub fn new(path: &str) -> Result<Self, IndexError> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    // Helper to append a new CID to a vector stored in a sled tree.
    // It deserializes the existing vector, adds the new CID, and serializes it back.
    fn append_to_vec<K: AsRef<[u8]>>(
        &self,
        tree: &sled::Tree,
        key: K,
        value_to_append: Cid, // Changed to accept Cid directly for clarity
    ) -> Result<(), IndexError> {
        let key_ref = key.as_ref();
        let existing_bytes = tree.get(key_ref)?;
        
        let mut vec: Vec<Cid> = match existing_bytes {
            Some(bytes) => bincode::deserialize(&bytes).map_err(IndexError::SerializeError)?,
            None => Vec::new(),
        };
        
        vec.push(value_to_append);
        let encoded = bincode::serialize(&vec).map_err(IndexError::SerializeError)?;
        tree.insert(key_ref, encoded)?;
        Ok(())
    }
}

impl DagIndex for SledDagIndex {
    fn add_node_to_index(&self, cid: &Cid, metadata_provider: &DagNode) -> Result<(), IndexError> {
        // It's good practice to open trees once if possible, perhaps in new() or store them in the struct,
        // but for simplicity in this prototype, opening them here is fine.
        let did_tree = self.db.open_tree("by_did")?;
        let scope_tree = self.db.open_tree("by_scope")?;
        
        // Key for DID index: DID string
        // Value for DID index: The CID of the node being indexed
        self.append_to_vec(&did_tree, metadata_provider.author.to_string().as_bytes(), cid.clone())?;
        
        // Key for Scope index: Debug representation of NodeScope
        // Value for Scope index: The CID of the node being indexed
        // Consider a more canonical string representation for NodeScope if Debug format isn't stable/ideal.
        self.append_to_vec(&scope_tree, format!("{:?}", metadata_provider.metadata.scope).as_bytes(), cid.clone())?;
        
        Ok(())
    }

    fn nodes_by_did(&self, did: &Did) -> Result<Vec<Cid>, IndexError> {
        let tree = self.db.open_tree("by_did")?;
        match tree.get(did.to_string().as_bytes())? {
            Some(bytes) => bincode::deserialize(&bytes).map_err(IndexError::SerializeError),
            None => Ok(vec![]), // Return an empty vector if no CIDs are associated with the DID
        }
    }

    fn nodes_by_scope(&self, scope: &NodeScope) -> Result<Vec<Cid>, IndexError> {
        let tree = self.db.open_tree("by_scope")?;
        // Using Debug representation for scope key. Ensure this is consistent and suitable.
        match tree.get(format!("{:?}", scope).as_bytes())? {
            Some(bytes) => bincode::deserialize(&bytes).map_err(IndexError::SerializeError),
            None => Ok(vec![]), // Return an empty vector if no CIDs are associated with the scope
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_types::dag::{DagNode, DagNodeMetadata, NodeScope, DagPayload}; // Use actual types
    use icn_identity_core::Did;
    use icn_core_types::Cid;
    use tempfile::tempdir;
    use std::str::FromStr;
    use multihash::{Code, MultihashDigest};

    // --- Test Helpers (similar to integration test) ---
    fn mock_did(name: &str) -> Did {
        Did::from_str(&format!("did:icn:test-{}", name)).unwrap()
    }

    fn mock_cid(id: u8) -> Cid {
        let data = format!("node-content-{}", id).into_bytes();
        Cid::new_v1(0x55, Code::Sha2_256.digest(&data))
    }

    // Creates a DagNode (not SignedDagNode as indexer uses DagNode)
    fn mock_dag_node(index: u32, author: Did, scope: NodeScope) -> DagNode {
        let metadata = DagNodeMetadata {
            federation_id: "test-fed".into(),
            timestamp: chrono::Utc::now(),
            label: Some(format!("node-label-{}", index)),
            scope: scope.clone(),
            scope_id: Some(format!("scope-id-{}", index % 3)),
        };

        DagNode {
            author: author.clone(),
            metadata,
            payload: DagPayload::Raw(format!("payload-data-{}", index).into_bytes()),
            parents: Vec::new(),
        }
    }
    // --- End Test Helpers ---

    #[test]
    fn test_sled_index_add_and_query() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let index_path = temp_dir.path().to_str().unwrap();
        let index = SledDagIndex::new(index_path).expect("Failed to create SledDagIndex");

        let did_a = mock_did("alice");
        let did_b = mock_did("bob");
        let scope_a = NodeScope::Community; // Keep it simple for testing
        let scope_b = NodeScope::Cooperative;

        let cid_1 = mock_cid(1);
        let node_1 = mock_dag_node(1, did_a.clone(), scope_a.clone());
        
        let cid_2 = mock_cid(2);
        let node_2 = mock_dag_node(2, did_b.clone(), scope_b.clone());
        
        let cid_3 = mock_cid(3);
        let node_3 = mock_dag_node(3, did_a.clone(), scope_a.clone()); // did_a, scope_a again

        // Index nodes
        index.add_node_to_index(&cid_1, &node_1).unwrap();
        index.add_node_to_index(&cid_2, &node_2).unwrap();
        index.add_node_to_index(&cid_3, &node_3).unwrap();

        // --- Verify nodes_by_did ---
        let cids_by_did_a = index.nodes_by_did(&did_a).unwrap();
        assert_eq!(cids_by_did_a.len(), 2, "Expected 2 nodes for DID A");
        assert!(cids_by_did_a.contains(&cid_1));
        assert!(cids_by_did_a.contains(&cid_3));

        let cids_by_did_b = index.nodes_by_did(&did_b).unwrap();
        assert_eq!(cids_by_did_b.len(), 1, "Expected 1 node for DID B");
        assert!(cids_by_did_b.contains(&cid_2));

        let did_c = mock_did("charlie"); // Non-existent DID
        let cids_by_did_c = index.nodes_by_did(&did_c).unwrap();
        assert!(cids_by_did_c.is_empty(), "Expected no nodes for DID C");

        // --- Verify nodes_by_scope ---
        let cids_by_scope_a = index.nodes_by_scope(&scope_a).unwrap();
        assert_eq!(cids_by_scope_a.len(), 2, "Expected 2 nodes for Scope A");
        assert!(cids_by_scope_a.contains(&cid_1));
        assert!(cids_by_scope_a.contains(&cid_3));

        let cids_by_scope_b = index.nodes_by_scope(&scope_b).unwrap();
        assert_eq!(cids_by_scope_b.len(), 1, "Expected 1 node for Scope B");
        assert!(cids_by_scope_b.contains(&cid_2));

        let scope_c = NodeScope::Federation; // Non-existent Scope
        let cids_by_scope_c = index.nodes_by_scope(&scope_c).unwrap();
        assert!(cids_by_scope_c.is_empty(), "Expected no nodes for Scope C");
    }

    #[test]
    fn test_sled_index_empty_queries() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let index_path = temp_dir.path().to_str().unwrap();
        let index = SledDagIndex::new(index_path).expect("Failed to create SledDagIndex");

        let did_a = mock_did("alice");
        let scope_a = NodeScope::Community;

        let cids_by_did = index.nodes_by_did(&did_a).unwrap();
        assert!(cids_by_did.is_empty(), "Expected empty result for DID on empty index");

        let cids_by_scope = index.nodes_by_scope(&scope_a).unwrap();
        assert!(cids_by_scope.is_empty(), "Expected empty result for scope on empty index");
    }

    // TODO: Add tests for error conditions (sled errors, serialization errors if possible)
} 