# ICN v2 — DAG Specification

Details the structure and validation rules for the Directed Acyclic Graph.

- **DagNode Schema**: Defines the fields within each node (e.g., payload, timestamp, parent tips, signature).
```json
{
  "payload": "...", // Base64 encoded transaction/data
  "parents": ["hash1", "hash2"], // Hashes of parent DagNodes
  "identity": "did:icn:...", // Author's DID
  "sequence": 123, // Sequence number for the identity
  "timestamp": 1678886400, // Unix timestamp
  "signature": "..." // Signature over the node content
}
```
- **Tip Selection**: Algorithm for choosing parent nodes for new additions.
- **Signature Verification**: Process for validating node authorship and integrity.
- **Conflict Resolution**: How conflicting transactions are handled (typically first-seen wins, based on DAG structure). 