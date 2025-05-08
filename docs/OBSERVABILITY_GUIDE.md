# ICN Observability Guide

This guide provides a comprehensive overview of the ICN federation observability system and how to use the observability dashboard to gain insights into your network. For a general project overview, see the main [project README](https://github.com/InterCooperative-Network/icn-v2/blob/main/README.md).

## 1. Introduction to Observability in ICN

The InterCooperative Network (ICN) observability layer provides **trust visibility**, **policy transparency**, and **governance accountability** across all scopes within the network. By making these processes visible, we enhance trust, accountability, and inclusivity in the federation.

### Core Principles

- **If it's not observable, it's not governable**
- **Trust depends on transparency**
- **Policy must be explicit, not implicit**
- **Governance requires accountability**

## 2. Observability Components

The observability system consists of:

1. **CLI Tools**: Command-line tools for inspecting DAGs, policies, quorum proofs, and governance activities
2. **Observability API**: A REST API that interfaces with the CLI tools
3. **Observability Dashboard**: A web-based UI for visualizing and interacting with the observability data

## 3. Getting Started with the CLI

The ICN CLI provides several commands for observability:

### DAG Viewer

```bash
# View DAG for a cooperative
icn observe dag-view --scope-type cooperative --scope-id coop-x

# Get JSON output
icn observe dag-view --scope-type cooperative --scope-id coop-x --output json
```

### Policy Inspector

```bash
# Inspect policy for a community
icn observe inspect-policy --scope-type community --scope-id oakridge

# Get JSON output
icn observe inspect-policy --scope-type community --scope-id oakridge --output json
```

### Quorum Validator

```bash
# Validate quorum for a DAG node
icn observe validate-quorum --cid zQm... --show-signers

# Get JSON output
icn observe validate-quorum --cid zQm... --output json
```

### Activity Log

```bash
# View governance activity for a federation
icn observe activity-log --scope-type federation --scope-id fed-main --limit 10

# Get JSON output
icn observe activity-log --scope-type federation --scope-id fed-main --output json
```

### Federation Overview

```bash
# View federation overview
icn observe federation-overview --federation-id fed-main

# Get JSON output
icn observe federation-overview --federation-id fed-main --output json
```

## 4. Using the Observability Dashboard

The Observability Dashboard provides a visual interface to interact with the observability system.

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd icn-v2/clients/observability-ui
```

2. Run the setup script:
```bash
./setup.sh
```

3. Start the API server:
```bash
cd server && npm start
```

4. In a new terminal, start the UI:
```bash
npm start
```

5. Access the dashboard at http://localhost:3000

### Dashboard Sections

#### DAG Viewer

The DAG Viewer allows you to visualize the Directed Acyclic Graph (DAG) for any scope:

1. Select a scope type (Federation, Cooperative, Community)
2. Enter the scope ID
3. Click "View DAG"
4. Explore the graph visualization and node details
5. Click on nodes to view metadata and payload information

#### Policy Inspector

The Policy Inspector shows active policies and their update history:

1. Select a scope type
2. Enter the scope ID
3. Click "Inspect Policy"
4. View the active policy content and rules
5. Explore the policy update history with voting records

#### Quorum Validator

The Quorum Validator verifies quorum proofs for governance actions:

1. Enter a DAG node CID
2. Toggle "Show Signers" to view detailed signer information
3. Click "Validate"
4. See the validation result, required signers, and actual signers
5. Inspect signer roles and scopes

#### Activity Log

The Activity Log tracks governance actions:

1. Select a scope type
2. Enter the scope ID
3. Set a limit for the number of activities to display
4. Click "View Activity"
5. Browse the timeline of governance actions
6. Expand activities to view detailed information

#### Federation Overview

The Federation Overview provides a high-level view of a federation:

1. Enter the federation ID
2. Click "View Federation"
3. See federation details and statistics
4. Browse the list of cooperatives and communities
5. View member details and activity information

#### Proposal Creation

The Proposal Creation feature allows you to submit new governance proposals directly from the dashboard:

1. Navigate to the "Create Proposal" page
2. In the first step, provide basic information:
   - Key file path (for signing the proposal)
   - Federation DID
   - Proposal title and description
3. In the second step, configure proposal details:
   - Proposal type (text, code execution, configuration changes, etc.)
   - Voting threshold (majority, unanimous, or custom percentage)
   - Voting duration
   - Any additional parameters
4. For scoped proposals (specific to a cooperative or community):
   - Toggle "Is this a scoped proposal?"
   - Select the scope type and ID
5. Review all information before submitting
6. Submit the proposal, which will be anchored in the DAG

Once submitted, proposals can be tracked in the Activity Log, and federation members can vote on them through the governance process.

## 5. The Complete Governance Feedback Loop

The ICN observability system enables a complete governance feedback loop:

1. **Observation**: Monitor governance activities, policy changes, and federation status through the dashboard.
2. **Verification**: Validate quorum proofs and policy compliance using built-in tools.
3. **Participation**: Submit proposals and participate in governance directly from the same interface.
4. **Accountability**: Track the outcome of votes and policy implementations.

This closed-loop system ensures that governance is not just a one-way process but a dynamic conversation where participants can both observe and act based on what they see.

### Mobile-First Governance

The mobile-friendly design ensures that governance is accessible to all members:

- Federation coordinators can monitor activity from anywhere
- Community members can verify decisions on their phones
- Cooperative representatives can submit proposals on the go
- All governance activities are recorded with full transparency

This creates a truly inclusive governance system that is accessible regardless of device or location.

## 6. Integration with Other Systems

The observability system can be integrated with:

- **Monitoring Tools**: Export JSON data for custom dashboards
- **Governance Workflows**: Verify quorum and policy compliance
- **Audit Processes**: Track governance actions and policy changes
- **Member Onboarding**: Demonstrate transparency to new members

## 7. Best Practices

### For Federation Operators

- **Regular Audits**: Schedule regular reviews of governance activities
- **Transparency Reports**: Generate periodic transparency reports for members
- **Policy Verification**: Verify policy compliance before executing changes
- **Quorum Validation**: Validate quorum proofs for all significant actions

### For Cooperative and Community Members

- **Active Monitoring**: Regularly check governance activities
- **Policy Awareness**: Stay informed about current policies
- **Participation Tracking**: Monitor your participation in governance
- **Verification**: Verify quorum for decisions that affect your scope

## 8. Troubleshooting

### Common Issues

- **API Connection Error**: Ensure the API server is running
- **CLI Not Found**: Make sure the ICN CLI is installed and in your PATH
- **Empty DAG View**: Verify the scope ID and ensure DAG nodes exist
- **Invalid Quorum**: Check the CID and ensure it contains a quorum proof

### Getting Help

- **Documentation**: Refer to the [ICN Documentation](../index.md)
- **CLI Help**: Run `icn observe --help` for command details
- **Issue Tracker**: Report issues on the project's issue tracker
- **Community Forum**: Ask questions in the ICN community forum

## 9. Advanced Topics

### Custom Visualizations

You can create custom visualizations by using the JSON output from the CLI commands:

```bash
icn observe dag-view --scope-type federation --scope-id fed-main --output json > fed-dag.json
icn observe activity-log --scope-type federation --scope-id fed-main --output json > fed-activity.json
```

### Automated Monitoring

Set up automated monitoring using cron jobs:

```bash
# Check for policy changes daily
0 0 * * * icn observe inspect-policy --scope-type federation --scope-id fed-main --output json > /var/log/icn/policy-$(date +\%Y\%m\%d).json

# Generate weekly activity report
0 0 * * 0 icn observe activity-log --scope-type federation --scope-id fed-main --output json > /var/log/icn/activity-$(date +\%Y\%m\%d).json
```

### Multi-Federation Observability

For organizations participating in multiple federations, you can aggregate data across federations:

```bash
# Create a list of federations
FEDERATIONS=("fed-main" "fed-test" "fed-dev")

# Loop through federations
for fed in "${FEDERATIONS[@]}"; do
  icn observe federation-overview --federation-id $fed --output json > /var/log/icn/$fed-overview.json
done
```

## 10. Conclusion

The ICN observability system provides a comprehensive view into the federation's operations, making trust visible, policy explicit, and governance transparent. By using the observability tools and dashboard, members can participate more effectively in the network's governance and ensure accountability at all levels.

Remember: **What is observable becomes governable, and what is transparent becomes trustworthy.** 