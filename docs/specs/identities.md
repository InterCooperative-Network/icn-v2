# ICN v2 — Identities Specification

Defines the structure and serialization for DIDs and VCs.

- **DID Method**: Specifies the ICN DID method (e.g., `did:icn:...`) and associated DID Document structure.
- **VC Schema**: Conforms to W3C VC Data Model v1.1. Defines specific credential types used within ICN.
- **Serialization**: Uses JSON-LD for canonical representation.

**Example VC (DemoCredential):**
```json
{
  "@context": ["https://www.w3.org/2018/credentials/v1", "https://intercoop.net/contexts/v1"],
  "id": "urn:uuid:...",
  "type": ["VerifiableCredential", "DemoCredential"],
  "issuer": "did:icn:...",
  "issuanceDate": "2024-01-01T00:00:00Z",
  "credentialSubject": {
    "id": "did:key:z123...",
    "demoAttribute": "ExampleValue"
  },
  "proof": { ... }
}
``` 