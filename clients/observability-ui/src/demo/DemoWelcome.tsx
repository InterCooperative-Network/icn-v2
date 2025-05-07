import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Box,
  Button,
  Card,
  CardContent,
  Divider,
  Paper,
  Typography,
  Stepper,
  Step,
  StepLabel,
  StepContent,
  Chip,
  Grid,
  Fade
} from '@mui/material';
import {
  Send as SendIcon,
  AccountTree as DagIcon,
  Policy as PolicyIcon,
  HowToVote as VoteIcon,
  Timeline as ActivityIcon,
  Group as FederationIcon,
  Add as AddIcon,
  PlayArrow as PlayIcon,
} from '@mui/icons-material';
import { getSeededData } from './seedData';

const demoData = getSeededData();

const DemoWelcome: React.FC = () => {
  const navigate = useNavigate();
  const [activeStep, setActiveStep] = useState(0);

  const handleNext = () => {
    setActiveStep((prevActiveStep) => prevActiveStep + 1);
  };

  const handleBack = () => {
    setActiveStep((prevActiveStep) => prevActiveStep - 1);
  };

  const handleGo = (path: string) => {
    navigate(path);
  };

  const steps = [
    {
      label: 'Welcome to the DemoCoop Federation',
      description: (
        <>
          <Typography paragraph>
            Welcome to the interactive demo of the InterCooperative Network (ICN) governance dashboard! 
            You are now exploring the <strong>{demoData.federation.name}</strong>, a simulated federation 
            with {demoData.cooperatives.length} cooperatives and {demoData.communities.length} communities.
          </Typography>
          
          <Typography paragraph>
            This demo allows you to experience the complete governance feedback loop:
          </Typography>
          
          <ul>
            <li>Observe ongoing governance through the DAG viewer</li>
            <li>Inspect federation policies and their history</li>
            <li>Validate quorum proofs to verify decisions</li>
            <li>Track governance activities in real-time</li>
            <li>Create new proposals to shape the federation</li>
            <li>Vote on open proposals to participate in governance</li>
          </ul>
          
          <Typography>
            Follow the steps in this guide to experience the key features of the system.
          </Typography>
        </>
      ),
      action: {
        label: 'View Federation Overview',
        path: '/federation-overview'
      }
    },
    {
      label: 'Observe the Federation Activity',
      description: (
        <>
          <Typography paragraph>
            The Activity Log shows all governance actions across the federation. Here you can see:
          </Typography>
          
          <ul>
            <li>Active proposals and their current voting status</li>
            <li>Recent votes by federation members</li>
            <li>Policy changes and their impact</li>
            <li>Member join events and federation milestones</li>
          </ul>
          
          <Typography paragraph>
            In the activity log, you can also <strong>vote on active proposals</strong> to experience
            direct participation in federation governance.
          </Typography>
        </>
      ),
      action: {
        label: 'Open Activity Log',
        path: '/activity-log'
      }
    },
    {
      label: 'Explore the DAG Structure',
      description: (
        <>
          <Typography paragraph>
            The Directed Acyclic Graph (DAG) provides the cryptographic backbone of the federation governance.
            This immutable record ensures:
          </Typography>
          
          <ul>
            <li>All governance actions are permanently recorded</li>
            <li>Parent-child relationships establish clear lineage</li>
            <li>Every node is signed by its creator</li>
            <li>Federation history can be verified by anyone</li>
          </ul>
          
          <Typography>
            In the DAG viewer, you can click on any node to see its details, metadata, and payload content.
          </Typography>
        </>
      ),
      action: {
        label: 'Open DAG Viewer',
        path: '/dag-view'
      }
    },
    {
      label: 'Inspect Federation Policies',
      description: (
        <>
          <Typography paragraph>
            The Policy Inspector shows the active governance rules and their history.
          </Typography>
          
          <ul>
            <li>View quorum rules for different action types</li>
            <li>See voting thresholds and durations</li>
            <li>Track the policy update history</li>
            <li>Verify who proposed and voted on each policy change</li>
          </ul>
          
          <Typography>
            This transparency ensures that all governance rules are explicit and verifiable.
          </Typography>
        </>
      ),
      action: {
        label: 'Open Policy Inspector',
        path: '/policy-inspector'
      }
    },
    {
      label: 'Create Your Own Proposal',
      description: (
        <>
          <Typography paragraph>
            Now it's your turn to participate directly in federation governance.
            Create a new proposal to suggest a change or action for the federation.
          </Typography>
          
          <Typography paragraph>
            You can create various types of proposals:
          </Typography>
          
          <ul>
            <li>Text Proposals for general discussions and decisions</li>
            <li>Configuration Changes to modify federation settings</li>
            <li>Member Additions to invite new cooperatives or communities</li>
            <li>Code Execution proposals for system upgrades</li>
          </ul>
          
          <Typography paragraph>
            In the proposal creation form, you can also scope your proposal to specific
            cooperatives or communities for more targeted governance.
          </Typography>
          
          <Typography variant="subtitle2" color="primary">
            Any proposals you create in the demo will be visible in the Activity Log and DAG Viewer!
          </Typography>
        </>
      ),
      action: {
        label: 'Create a Proposal',
        path: '/create-proposal'
      }
    },
    {
      label: 'Complete Governance Loop',
      description: (
        <>
          <Typography paragraph>
            You've now experienced the complete governance feedback loop:
          </Typography>
          
          <Box sx={{ p: 2, bgcolor: '#f5f5f5', borderRadius: 2, mb: 2 }}>
            <ol>
              <li><strong>Observe</strong> - See governance activities and their history</li>
              <li><strong>Verify</strong> - Validate quorum proofs and policy compliance</li>
              <li><strong>Propose</strong> - Create new governance proposals</li>
              <li><strong>Decide</strong> - Vote on active proposals</li>
            </ol>
          </Box>
          
          <Typography paragraph>
            This cycle demonstrates how the ICN enables a <strong>living, participatory democracy</strong> that:
          </Typography>
          
          <ul>
            <li>Makes governance actions visible in real-time</li>
            <li>Allows participation from any device, anywhere</li>
            <li>Creates a complete feedback loop in a single interface</li>
            <li>Makes governance inclusive and accessible</li>
          </ul>
          
          <Typography paragraph>
            Continue exploring the dashboard to experience all its features!
          </Typography>
        </>
      ),
      action: {
        label: 'Return to Dashboard',
        path: '/'
      }
    },
  ];

  return (
    <Fade in timeout={800}>
      <Paper 
        elevation={3} 
        sx={{ 
          p: 3, 
          borderRadius: 2, 
          bgcolor: '#fcfcfc',
          backgroundImage: 'radial-gradient(circle at 25px 25px, #f5f5f5 2%, transparent 0%), radial-gradient(circle at 75px 75px, #f5f5f5 2%, transparent 0%)',
          backgroundSize: '100px 100px',
          mb: 4 
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
          <PlayIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
          <Typography variant="h4" gutterBottom component="div" sx={{ mb: 0, fontWeight: 'medium' }}>
            Interactive Governance Demo
          </Typography>
        </Box>

        <Typography variant="subtitle1" color="text.secondary" paragraph>
          Experience a complete governance feedback loop in the {demoData.federation.name}.
        </Typography>

        <Divider sx={{ mt: 1, mb: 4 }} />

        <Grid container spacing={3}>
          <Grid item xs={12} md={4}>
            <Card sx={{ height: '100%', bgcolor: '#f8f8f8', borderRadius: 2 }}>
              <CardContent>
                <Typography variant="h6" gutterBottom>Federation Members</Typography>
                <Divider sx={{ mb: 2 }} />
                
                <Typography variant="subtitle2" color="primary" gutterBottom>Cooperatives</Typography>
                {demoData.cooperatives.map((coop, index) => (
                  <Chip 
                    key={index} 
                    label={coop.name} 
                    variant="outlined" 
                    color="primary" 
                    size="small" 
                    sx={{ m: 0.5 }} 
                  />
                ))}
                
                <Typography variant="subtitle2" color="secondary" sx={{ mt: 2 }} gutterBottom>
                  Communities
                </Typography>
                {demoData.communities.map((comm, index) => (
                  <Chip 
                    key={index} 
                    label={comm.name} 
                    variant="outlined" 
                    color="secondary" 
                    size="small" 
                    sx={{ m: 0.5 }} 
                  />
                ))}
                
                <Typography variant="subtitle2" color="text.secondary" sx={{ mt: 2 }} gutterBottom>
                  Active Proposals
                </Typography>
                <Typography variant="h5" color="primary">
                  {demoData.proposals.filter(p => p.status === 'active').length}
                </Typography>
              </CardContent>
            </Card>
          </Grid>
          
          <Grid item xs={12} md={8}>
            <Stepper activeStep={activeStep} orientation="vertical">
              {steps.map((step, index) => (
                <Step key={step.label}>
                  <StepLabel>
                    <Typography variant="subtitle1">{step.label}</Typography>
                  </StepLabel>
                  <StepContent>
                    <Box sx={{ mb: 2 }}>
                      {step.description}
                      <Box sx={{ mt: 2 }}>
                        <Button
                          variant="contained"
                          onClick={() => handleGo(step.action.path)}
                          sx={{ mt: 1, mr: 1 }}
                          startIcon={
                            step.action.path === '/dag-view' ? <DagIcon /> :
                            step.action.path === '/policy-inspector' ? <PolicyIcon /> :
                            step.action.path === '/activity-log' ? <ActivityIcon /> :
                            step.action.path === '/federation-overview' ? <FederationIcon /> :
                            step.action.path === '/create-proposal' ? <AddIcon /> :
                            <SendIcon />
                          }
                        >
                          {step.action.label}
                        </Button>
                        
                        <Button
                          disabled={index === 0}
                          onClick={handleBack}
                          sx={{ mt: 1, mr: 1 }}
                        >
                          Back
                        </Button>
                        
                        {index < steps.length - 1 && (
                          <Button
                            variant="text"
                            onClick={handleNext}
                            sx={{ mt: 1, mr: 1 }}
                          >
                            Skip to Next Step
                          </Button>
                        )}
                      </Box>
                    </Box>
                  </StepContent>
                </Step>
              ))}
            </Stepper>
          </Grid>
        </Grid>
      </Paper>
    </Fade>
  );
};

export default DemoWelcome; 