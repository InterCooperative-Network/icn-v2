# ICN v2 — Governance Flow

The lifecycle of a network proposal:

1.  **Proposal Creation**: A member crafts a proposal (e.g., code update, parameter change) and submits it via AgoraNet.
2.  **Deliberation**: Discussion and refinement occur within AgoraNet.
3.  **Voting**: Eligible members cast votes on the proposal using their Wallet agent. Votes are recorded on the DAG.
4.  **Tallying & Execution**: Votes are tallied; if passed, the proposal's execution logic is triggered within CoVM.
5.  **Receipt**: A cryptographic receipt confirming the outcome (pass/fail, execution results) is generated and anchored to the DAG.

## Implementation Status

The governance flow is implemented with the following components:

### Verifiable Credentials

| Credential                | Status      | Description                                               |
|---------------------------|-------------|-----------------------------------------------------------|
| `ProposalCredential`      | ✅ Complete | VC representing a governance proposal with voting rules   |
| `VoteCredential`          | ✅ Complete | VC representing an individual vote on a proposal          |
| `ExecutionReceipt`        | ✅ Complete | VC confirming execution of passed proposal                |

### Quorum Engine

| Component                 | Status      | Description                                                |
|---------------------------|-------------|------------------------------------------------------------|
| `QuorumEngine`            | ✅ Complete | Service to evaluate votes against proposal thresholds      |
| `QuorumTally`             | ✅ Complete | Result of vote counting with detailed statistics           |
| `QuorumOutcome`           | ✅ Complete | Determination of proposal success (Passed/Failed/etc.)     |

The Quorum Engine supports the following voting threshold types:

- **Majority**: More than 50% of votes must be "yes"
- **Percentage**: A specified percentage of votes must be "yes" (e.g., 66%)
- **Unanimous**: All voters must vote "yes"
- **Weighted**: Voters have different weights, and a threshold of total voting power must be reached

### CLI Commands

The governance flow can be managed using the following CLI commands:

```bash
# Submit a new proposal
icn proposal submit --key-file member.jwk --federation did:icn:fed1 --title "Test Proposal" --description "This is a test proposal" 

# Activate a proposal for voting
icn proposal activate --key-file admin.jwk [proposal-id]

# Cast a vote on a proposal
icn vote cast --key-file voter.jwk --federation did:icn:fed1 --proposal-id [proposal-id] --decision yes

# Tally votes and determine outcome
icn vote tally --key-file admin.jwk [proposal-id]

# Execute a passed proposal
icn runtime execute --proposal-id [proposal-id] --key-file admin.jwk

# View proposal details
icn proposal show [proposal-id]

# List all proposals
icn proposal list

# View votes for a proposal
icn vote list [proposal-id]
```

## Governance Flow Example

Here's a complete example workflow:

1. **Federation Setup**:
   ```bash
   icn federation init --quorum majority --members did:key:alice,did:key:bob,did:key:charlie
   ```

2. **Proposal Submission**:
   ```bash
   icn proposal submit --key-file alice.jwk --federation did:icn:fed1 \
     --title "Update Configuration" --description "Update federation params" \
     --proposal-type configChange --voting-threshold majority --voting-duration 86400
   ```

3. **Proposal Deliberation**:
   - Members discuss the proposal via AgoraNet
   - Make amendments if necessary

4. **Proposal Activation**:
   ```bash
   icn proposal activate --key-file alice.jwk urn:uuid:123e4567-e89b-12d3-a456-426614174000
   ```

5. **Voting**:
   ```bash
   icn vote cast --key-file alice.jwk --federation did:icn:fed1 \
     --proposal-id urn:uuid:123e4567-e89b-12d3-a456-426614174000 --decision yes

   icn vote cast --key-file bob.jwk --federation did:icn:fed1 \
     --proposal-id urn:uuid:123e4567-e89b-12d3-a456-426614174000 --decision yes

   icn vote cast --key-file charlie.jwk --federation did:icn:fed1 \
     --proposal-id urn:uuid:123e4567-e89b-12d3-a456-426614174000 --decision no
   ```

6. **Vote Tallying**:
   ```bash
   icn vote tally --key-file alice.jwk urn:uuid:123e4567-e89b-12d3-a456-426614174000
   ```

7. **Execution** (if passed):
   ```bash
   icn runtime execute --proposal-id urn:uuid:123e4567-e89b-12d3-a456-426614174000 --key-file alice.jwk
   ```

8. **Receipt Verification**:
   ```bash
   icn receipt show [receipt-id]
   ```

## Quorum Engine Details

The QuorumEngine is responsible for processing votes and determining if a proposal passes. It features:

- **Valid vote filtering**: Ensures only valid votes from eligible members are counted
- **Vote de-duplication**: Considers only the most recent vote from each member
- **Flexible threshold support**: Handles majority, percentage, unanimous, and weighted voting
- **Veto support**: Any veto vote causes proposal to fail regardless of other votes
- **Voting power**: Supports weighted votes with different power levels
- **Detailed tallies**: Provides complete statistics about the voting process

### Example usage:

```rust
// Create a quorum engine
let engine = QuorumEngine::new();

// Or with a restricted member list
let engine = QuorumEngine::with_members(vec![member1_did, member2_did]);

// Evaluate votes on a proposal
let tally = engine.evaluate(&proposal, &votes)?;

// Check the outcome
match tally.outcome {
    QuorumOutcome::Passed => {
        // Handle passed proposal
    },
    QuorumOutcome::Failed => {
        // Handle failed proposal
    },
    QuorumOutcome::Inconclusive => {
        // Voting period not ended yet
    },
    QuorumOutcome::Invalid => {
        // Proposal not in votable state
    }
}
```

## Next Steps

- ✅ **DAG Anchoring**: Implement real DAG loading logic for proposals and votes
- ✅ **Runtime Integration**: Enable proposal-triggered execution via `icn proposal execute` 
- **AgoraNet Integration**: Link proposal threads to ProposalCredentials
- **Wallet Support**: Add UI for proposal browsing and voting

With the implementation of DAG integration for the `proposal execute` command, the governance flow now enables:
1. **Proposal Creation** (`proposal submit`) → DAG anchoring
2. **Voting** (`vote cast`) → DAG anchoring 
3. **Verifiable Resolution** - Retrieving proposals and votes from the DAG
4. **Execution** (`proposal execute`) - Using QuorumEngine to validate votes
5. **Verification** - Anchoring ExecutionReceipt back to the DAG

This creates a complete trust chain from deliberation to execution, with each step cryptographically linked and verifiable.

## Diagrams

```
                   ┌─────────────┐
                   │  AgoraNet   │
                   │ Deliberation│
                   └──────┬──────┘
                          │
                          ▼
┌─────────────┐    ┌──────────────┐    ┌──────────────┐
│             │    │              │    │              │
│  Proposal   │───▶│    Voting    │───▶│   Execution  │
│  Submission │    │              │    │              │
│             │    │              │    │              │
└─────────────┘    └──────────────┘    └──────────────┘
                          │                    │
                          │                    │
                          ▼                    ▼
                   ┌──────────────┐    ┌──────────────┐
                   │              │    │              │
                   │   Vote       │    │  Execution   │
                   │  Credential  │    │   Receipt    │
                   │              │    │              │
                   └──────────────┘    └──────────────┘
```

### Governance Processing Flow

```
┌───────────────┐     ┌───────────────┐     ┌────────────────┐
│ Proposals     │     │ Votes         │     │ Quorum Engine  │
│ (DAG)         │─────┤ (DAG)         │────▶│                │
└───────────────┘     └───────────────┘     └───────┬────────┘
                                                     │
                                                     ▼
┌───────────────┐     ┌───────────────┐     ┌────────────────┐
│ Runtime       │◀────│ Status Update │◀────│ Tally Result   │
│ Execution     │     │ (if passed)   │     │                │
└───────────────┘     └───────────────┘     └────────────────┘
         │
         ▼
┌───────────────┐
│ Execution     │
│ Receipt       │
└───────────────┘
``` 