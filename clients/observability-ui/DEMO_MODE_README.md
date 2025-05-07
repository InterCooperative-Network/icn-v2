# ICN Demo Mode

The ICN Observability Dashboard now includes a comprehensive demo mode that showcases the entire governance feedback loop. This feature allows users to explore a fully functional federation with seeded data and interact with all aspects of the governance system without requiring a running ICN instance.

## Overview

Demo mode creates a complete simulated environment with:

1. A federation with cooperatives and communities
2. Active and historical governance proposals
3. Votes and policy changes
4. A fully interactive DAG visualization
5. Ability to create proposals and cast votes
6. Complete visibility of governance activities

## Key Features

### 1. Interactive Governance Loop

The demo provides a complete constitutional governance loop:

- **Observation**: View federation activities, DAG structure, and policies
- **Verification**: Validate quorum proofs and governance legitimacy
- **Participation**: Create new proposals and vote on existing ones
- **Feedback**: See your actions immediately reflected in the system

### 2. Guided Tour

A step-by-step guided tour introduces users to the main features:

- Federation overview and membership structure
- Activity log with governance history
- DAG viewer for cryptographic verification
- Policy inspection for rule transparency
- Proposal creation for direct participation

### 3. Realistic Interactions

The demo allows users to:

- Submit new governance proposals
- Vote on active proposals
- See their actions recorded in the DAG
- Track governance history across time
- Verify proposal validity and voting rules

## Technical Implementation

The demo mode implementation includes:

### Seeded Data

- `seedData.ts`: Generates a consistent set of federation data
- Includes members, DAG nodes, activities, proposals, and votes

### Demo API

- `demoApi.ts`: In-memory API service that mimics real API calls
- Remembers user actions during the session
- Simulates proposal submission and voting

### Demo Context

- `DemoModeContext.tsx`: React context for managing demo state
- Allows toggling between real API and demo API
- Persists preferences across page refreshes

### UI Components

- `DemoModeToggle.tsx`: Control for enabling/disabling demo mode
- `DemoWelcome.tsx`: Interactive guided tour of features

## Using the Demo

1. Toggle the "Demo Mode" switch in the top-right corner of the dashboard
2. Follow the guided tour on the dashboard homepage
3. Interact with any feature as you would in a real system
4. Create proposals and vote on existing ones
5. See your actions reflected across the entire dashboard

## Implementation Notes

- All demo data and interactions are stored in memory
- Data resets when the page is refreshed
- Demo mode is toggled with a persistent setting in localStorage
- No API calls are made to actual ICN instances while in demo mode

## Educational Value

This demo mode serves multiple purposes:

1. **Training**: Onboard new federation members without risk
2. **Demonstration**: Showcase ICN features to potential adopters
3. **Testing**: Explore edge cases and user flows without infrastructure
4. **Conceptual clarity**: Make abstract governance concepts tangible

## Future Enhancements

Potential future improvements to demo mode:

- Multiple federation templates with different governance structures
- Simulated failure scenarios to demonstrate resilience
- Time acceleration to show governance over longer periods
- Network visualization showing federation member relationships 