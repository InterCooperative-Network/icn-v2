import React from 'react';
import { useNavigate } from 'react-router-dom';
import { 
  Box, 
  Card, 
  CardContent, 
  CardMedia, 
  CardActionArea,
  Grid, 
  Typography,
  Paper
} from '@mui/material';
import {
  AccountTree as DagIcon,
  Policy as PolicyIcon,
  CheckCircle as QuorumIcon,
  Timeline as ActivityIcon,
  Group as FederationIcon,
  Add as AddIcon
} from '@mui/icons-material';
import { useDemoMode } from '../demo/DemoModeContext';
import DemoWelcome from '../demo/DemoWelcome';

const DashboardPage: React.FC = () => {
  const navigate = useNavigate();
  const { isDemoMode } = useDemoMode();

  // Dashboard card data
  const cards = [
    {
      title: 'DAG Viewer',
      description: 'Explore the Directed Acyclic Graph (DAG) threads with detailed node metadata.',
      icon: <DagIcon fontSize="large" color="primary" />,
      path: '/dag-view',
      color: '#e3f2fd'
    },
    {
      title: 'Policy Inspector',
      description: 'View current active policies and track their update history.',
      icon: <PolicyIcon fontSize="large" color="secondary" />,
      path: '/policy-inspector',
      color: '#f3e5f5'
    },
    {
      title: 'Quorum Validator',
      description: 'Validate quorum proofs on DAG nodes and verify governance actions.',
      icon: <QuorumIcon fontSize="large" color="info" />,
      path: '/quorum-validator',
      color: '#e1f5fe'
    },
    {
      title: 'Activity Log',
      description: 'Track governance activity like proposals, votes, policy changes and joins.',
      icon: <ActivityIcon fontSize="large" color="success" />,
      path: '/activity-log',
      color: '#e8f5e9'
    },
    {
      title: 'Federation Overview',
      description: 'View federation members and their status at a high level.',
      icon: <FederationIcon fontSize="large" color="error" />,
      path: '/federation-overview',
      color: '#ffebee'
    },
    {
      title: 'Create Proposal',
      description: 'Submit a new governance proposal to the federation.',
      icon: <AddIcon fontSize="large" color="warning" />,
      path: '/create-proposal',
      color: '#fff8e1'
    }
  ];

  return (
    <Box>
      {/* Show demo welcome screen when in demo mode */}
      {isDemoMode && <DemoWelcome />}
      
      {/* Main dashboard header */}
      {!isDemoMode && (
        <Paper elevation={0} sx={{ p: 3, mb: 4, borderRadius: 2, bgcolor: '#f8f9fa' }}>
          <Typography variant="h4" gutterBottom>
            ICN Federation Observability
          </Typography>
          <Typography variant="body1" color="text.secondary">
            This dashboard provides transparency into the InterCooperative Network (ICN) federation,
            making trust visible, policy explicit, and governance transparent across all scopes.
          </Typography>
        </Paper>
      )}

      <Grid container spacing={3}>
        {cards.map((card) => (
          <Grid item xs={12} sm={6} md={4} key={card.title}>
            <Card 
              elevation={2} 
              sx={{ 
                height: '100%', 
                display: 'flex', 
                flexDirection: 'column',
                borderRadius: 2,
                transition: 'transform 0.2s',
                '&:hover': {
                  transform: 'translateY(-5px)',
                },
              }}
            >
              <CardActionArea 
                onClick={() => navigate(card.path)}
                sx={{ flexGrow: 1, display: 'flex', flexDirection: 'column', alignItems: 'stretch' }}
              >
                <Box sx={{ p: 3, bgcolor: card.color, display: 'flex', justifyContent: 'center', alignItems: 'center' }}>
                  {React.cloneElement(card.icon, { style: { fontSize: 60 } })}
                </Box>
                <CardContent sx={{ flexGrow: 1 }}>
                  <Typography gutterBottom variant="h5" component="div">
                    {card.title}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    {card.description}
                  </Typography>
                </CardContent>
              </CardActionArea>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
};

export default DashboardPage; 