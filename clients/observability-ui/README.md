# ICN Federation Observability Dashboard

A responsive web dashboard for the InterCooperative Network (ICN) that makes trust visible, policy explicit, and governance transparent across cooperatives, communities, and federations.

## Features

### 1. DAG Viewer
- Visualize DAG threads with interactive graph visualization
- Display node metadata: CID, timestamp, signer DID, payload type
- Show parent-child relationships
- Examine payload content

### 2. Policy Inspector
- View current active policies for a scope
- Track policy update history with voting records
- Render policy content in readable format

### 3. Quorum Proof Validator
- Validate quorum proofs on DAG nodes
- Show required vs. actual signers with role information
- Verify governance action thresholds

### 4. Governance Activity Log
- Track recent governance actions
- Filter by activity type: proposals, votes, policy changes, federation joins
- View detailed activity information

### 5. Federation Overview
- Display federation composition: cooperatives and communities
- Show member statistics and latest activity
- View latest DAG heads for each scope

### 6. Proposal Creation
- Submit new governance proposals directly from the dashboard
- Support for federation-wide and scoped proposals
- Multiple proposal types: text, code execution, configuration change, etc.
- Configurable voting thresholds and durations
- Mobile-friendly multi-step submission process

## Getting Started

### Prerequisites
- Node.js 16+ and npm
- Access to ICN API endpoints (configurable in environment)

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd icn-v2/clients/observability-ui
```

2. Install dependencies:
```bash
npm install
```

3. Configure environment variables:
Create a `.env` file with the following:
```
REACT_APP_API_URL=http://localhost:3001/api  # Update with your actual API endpoint
```

4. Start the development server:
```bash
npm start
```

The dashboard will be available at `http://localhost:3000`

## Usage

The dashboard is designed to be intuitive and user-friendly:

1. Select a tool from the sidebar menu
2. Enter the required parameters (scope type, scope ID, etc.)
3. View and interact with the results
4. All tools support JSON export for programmatic use

### Creating Proposals

To create a new governance proposal:

1. Navigate to "Create Proposal" in the sidebar
2. Follow the step-by-step form:
   - Enter basic information (key file, federation, title, description)
   - Configure proposal details (type, voting threshold, duration)
   - Review and submit
3. For scoped proposals, toggle "Is this a scoped proposal?" and select the scope type and ID
4. After submission, your proposal will be visible in the Activity Log

## Mobile Compatibility

The dashboard is fully responsive and works on mobile devices:
- Adapts to different screen sizes
- Touch-friendly interface
- Optimized data display for smaller screens

## Technical Details

### Frontend Architecture
- React with TypeScript
- Material UI for responsive components
- React Router for navigation
- Axios for API communication
- D3.js and React Force Graph for visualizations

### API Integration
The dashboard interfaces with the ICN CLI via an API layer that wraps the observability commands:
- `icn observe dag-view`
- `icn observe inspect-policy`
- `icn observe validate-quorum`
- `icn observe activity-log`
- `icn observe federation-overview`
- `icn proposal submit`
- `icn proposal vote`

## Building for Production

```bash
npm run build
```

The production build will be in the `build` directory.

## License

This project is part of the InterCooperative Network (ICN) and is licensed under [LICENSE].