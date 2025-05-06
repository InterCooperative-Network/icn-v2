# ICN Mesh Verification Guide

This guide explains how to use the verification features of the InterCooperative Network (ICN) Mesh to audit and verify dispatch decisions, node manifests, and capability requirements.

## Trusted DIDs Policy

The ICN Mesh uses a trust system based on Decentralized Identifiers (DIDs) to determine which entities are authorized to perform various actions within a federation. The trust policy is defined in a TOML file with the following structure:

```toml
# Basic configuration
federation_id = "my-federation"
allow_dag_updates = true  # Whether policy updates via DAG are allowed

# Fully trusted entities (can perform all actions)
[[trusted_dids]]
did = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"
level = "Full"
notes = "Federation coordinator"

# Manifest providers (can submit node manifests)
[[trusted_dids]]
did = "did:key:z6MkjchhfUsD6mmvni8mCdXHw216Xrm9bQe2mBH1P5RDjVJG"
level = "ManifestProvider"
notes = "Cloud compute provider"

# Admin DIDs for policy management
policy_admins = [
  "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"
]
```

Each DID entry specifies:

- `did`: The DID of the entity
- `level`: Trust level (Full, ManifestProvider, Requestor, Worker, Admin)
- `notes` (optional): Human-readable notes about this entity
- `expires` (optional): Expiration date for this trust entry (ISO 8601 format)

## Trust Levels

The following trust levels are available:

- **Full**: Can perform all actions (submit manifests, request tasks, dispatch tasks, etc.)
- **ManifestProvider**: Can only submit node manifests
- **Requestor**: Can only request tasks
- **Worker**: Can only execute tasks
- **Admin**: Can update trust policies

## Verifying Dispatch Credentials

The ICN Mesh provides an audit command to verify dispatch decisions and check if they adhere to the trust policy:

```bash
icn mesh audit --audit-type dispatch --dag-dir ./my-dag --federation my-federation --verify --trusted-dids-path ./trusted_dids.toml
```

This command performs several verifications:

1. **Signature Verification**: Checks if the dispatch credential's signature is valid
2. **DAG Record Match**: Ensures the credential matches the corresponding DAG record
3. **Trust Policy Compliance**: Verifies that:
   - The scheduler (issuer) is trusted at the Full level
   - The selected node is trusted at the Worker level
   - The requestor is trusted at the Requestor level

You can export the verification results to a JSON file:

```bash
icn mesh audit --audit-type dispatch --dag-dir ./my-dag --federation my-federation --verify --trusted-dids-path ./trusted_dids.toml --export-results verification_report.json
```

## Filtering Audit Records

You can filter the audit records by:

```bash
# Filter by specific CID
icn mesh audit --cid bafyreigsj2osro5no3vdxy3krvxwnagauep4wjcrn6iofwwp4koo2x5hei

# Filter by scheduler
icn mesh audit --scheduler did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK

# Filter by task
icn mesh audit --task bafyreigsj2osro5no3vdxy3krvxwnagauep4wjcrn6iofwwp4koo2x5hei

# Limit results
icn mesh audit --limit 5
```

## Verification Output

Here's an example of verification output:

```
Dispatch Audit Records for Federation: test-federation
2 records found

1: bafyreigsj2osro5no3vdxy3krvxwnagauep4wjcrn6iofwwp4koo2x5hei
  Scheduler: did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK
  Task CID: bafyreidcs2jaovtqf4hjgulrkgexdbgnwyge4y3z34uqtzohgzrv3wqj4
  Task Type: wasm-compute
  Timestamp: 2023-09-15T14:22:30Z
  Capability Requirements:
    memory_mb = 512
    cores = 2
    renewable = 50
  Selected Node: did:key:z6MkhyDx5DjrfudJDK9oYM1rhxRtPPphPGQRbwk6jgr9dVRQ
  Score: 0.8700
  Verifying dispatch credential...
  ✓ Signature: Valid
  ✓ DAG Match: Valid
  ✓ Issuer Trust: Trusted
  ✓ Worker Trust: Trusted
  ✓ Requestor Trust: Trusted

2: bafyreibquxopl3t5bq2njwcb5cnxvwgxvno3qfembnrjdkwntrmvnvkryi
  Scheduler: did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK
  Task CID: bafyreibl3co6sfwlmfeva7ivvypbflkpjd5rkuik5e5cv5r5xkv43ezrli
  Task Type: wasm-compute
  Timestamp: 2023-09-15T14:23:45Z
  Capability Requirements:
    memory_mb = 1024
    cores = 4
    gpu = true
  Selected Node: did:key:z6MkgYAZjnMQQNrR7xz5wKBkjEqemgydPnS9umwBNFZb8uLG
  Score: 0.9100
  Verifying dispatch credential...
  ✓ Signature: Valid
  ✓ DAG Match: Valid
  ✓ Issuer Trust: Trusted
  ✓ Worker Trust: Trusted
  ✗ Requestor Trust: Not trusted

Verification Summary:
  Total Credentials: 2
  Valid & Trusted: 1
  Invalid or Untrusted: 1
```

## Using Trusted DIDs in Your Application

You can also use the TrustedDidPolicy module in your own code:

```rust
use planetary_mesh::trusted_did_policy::{TrustPolicyFactory, TrustLevel};
use icn_identity_core::Did;

async fn verify_trusted_dids() -> Result<()> {
    // Load the policy from a file
    let factory = TrustPolicyFactory::new();
    let policy = factory.from_file("./trusted_dids.toml")?;
    
    // Check if a DID is trusted
    let did = Did::from("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK");
    if policy.is_trusted(&did) {
        println!("DID is trusted");
    }
    
    // Check if a DID is trusted for a specific level
    if policy.is_trusted_for(&did, TrustLevel::Full) {
        println!("DID is fully trusted");
    }
    
    Ok(())
}
```

## Security Considerations

1. **Policy Updates**: When updating your trust policy, make sure to keep a backup of the previous version.
2. **Key Security**: The DIDs listed in your trust policy represent cryptographic keys. Keep the private keys secure.
3. **Expiration**: Consider using the `expires` field for DIDs that should only be trusted temporarily.
4. **Admin Access**: Limit the number of DIDs with Admin level access.

## Next Steps

- Integrate verification into your federation governance processes
- Set up regular auditing schedules to ensure compliance
- Configure monitoring tools to alert on verification failures 