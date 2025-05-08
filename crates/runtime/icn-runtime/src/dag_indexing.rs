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