import React, { useState, useCallback } from 'react';
import { 
  Box, 
  Button, 
  Card, 
  Chip, 
  FormControl, 
  Grid, 
  InputLabel, 
  MenuItem, 
  Paper, 
  Select, 
  SelectChangeEvent, 
  TextField, 
  Typography,
  Divider,
  CircularProgress,
  Alert,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  ListItemAvatar,
  Avatar,
  Stack,
  Tab,
  Tabs
} from '@mui/material';
import {
  Group as FederationIcon,
  Business as CooperativeIcon,
  Hub as CommunityIcon,
  Info as InfoIcon,
  Description as DescriptionIcon,
  Link as LinkIcon,
  LocalOffer as TagIcon
} from '@mui/icons-material';
import observabilityApi, { FederationOverview, MemberInfo } from '../api/observabilityApi';

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

const TabPanel = (props: TabPanelProps) => {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`simple-tabpanel-${index}`}
      aria-labelledby={`simple-tab-${index}`}
      {...other}
    >
      {value === index && (
        <Box sx={{ py: 3 }}>
          {children}
        </Box>
      )}
    </div>
  );
};

const FederationOverviewPage: React.FC = () => {
  const [federationId, setFederationId] = useState<string>('');
  const [overview, setOverview] = useState<FederationOverview | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [tabValue, setTabValue] = useState(0);

  // Function to fetch federation overview
  const fetchFederationOverview = useCallback(async () => {
    if (!federationId) {
      setError('Please enter a federation ID');
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const data = await observabilityApi.getFederationOverview(federationId);
      setOverview(data);
    } catch (err) {
      console.error('Error fetching federation overview:', err);
      setError('Failed to fetch federation overview. Please try again.');
    } finally {
      setLoading(false);
    }
  }, [federationId]);

  const handleFederationIdChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setFederationId(event.target.value);
  };

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setTabValue(newValue);
  };

  // Format timestamp
  const formatTimestamp = (timestamp: string | undefined) => {
    if (!timestamp) return 'N/A';
    return new Date(timestamp).toLocaleString();
  };

  // Render member list
  const renderMemberList = (members: MemberInfo[]) => {
    if (members.length === 0) {
      return (
        <Typography variant="body2" color="text.secondary" align="center" sx={{ py: 3 }}>
          No members found in this category.
        </Typography>
      );
    }

    return (
      <List>
        {members.map((member, index) => (
          <Card key={index} variant="outlined" sx={{ mb: 2, borderRadius: 2 }}>
            <ListItem alignItems="flex-start" sx={{ py: 2 }}>
              <ListItemAvatar>
                <Avatar sx={{ bgcolor: member.type === 'Cooperative' ? 'primary.light' : 'secondary.light' }}>
                  {member.type === 'Cooperative' ? <CooperativeIcon /> : <CommunityIcon />}
                </Avatar>
              </ListItemAvatar>
              <ListItemText
                primary={
                  <Typography variant="h6" component="div">
                    {member.name || member.id}
                    <Chip 
                      label={member.type} 
                      size="small" 
                      color={member.type === 'Cooperative' ? 'primary' : 'secondary'}
                      sx={{ ml: 1 }}
                    />
                  </Typography>
                }
                secondary={
                  <Box sx={{ mt: 1 }}>
                    <Typography variant="body2" color="text.secondary" gutterBottom>
                      ID: {member.id}
                    </Typography>
                    
                    <Grid container spacing={2} sx={{ mt: 1 }}>
                      <Grid item xs={12} sm={6}>
                        <Typography variant="caption" color="text.secondary" display="block">
                          Last Activity
                        </Typography>
                        <Typography variant="body2">
                          {formatTimestamp(member.latest_timestamp)}
                        </Typography>
                      </Grid>
                      {member.latest_head && (
                        <Grid item xs={12} sm={6}>
                          <Typography variant="caption" color="text.secondary" display="block">
                            Latest DAG Head
                          </Typography>
                          <Typography 
                            variant="body2" 
                            sx={{ 
                              wordBreak: 'break-all',
                              display: 'flex',
                              alignItems: 'center'
                            }}
                          >
                            <LinkIcon fontSize="small" sx={{ mr: 0.5 }} />
                            {member.latest_head}
                          </Typography>
                        </Grid>
                      )}
                    </Grid>
                  </Box>
                }
              />
            </ListItem>
          </Card>
        ))}
      </List>
    );
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Typography variant="h5" gutterBottom>
          Federation Overview
        </Typography>
        <Typography variant="body2" color="text.secondary" paragraph>
          View federation members and their latest DAG heads at a glance.
          Get insights into the federation's composition and activity.
        </Typography>
        
        <Grid container spacing={2} alignItems="flex-end" sx={{ mt: 1 }}>
          <Grid item xs={12} sm={9}>
            <TextField
              fullWidth
              label="Federation ID"
              value={federationId}
              onChange={handleFederationIdChange}
              margin="normal"
              placeholder="Enter federation ID to view its structure"
            />
          </Grid>
          <Grid item xs={12} sm={3}>
            <Button 
              variant="contained" 
              fullWidth 
              onClick={fetchFederationOverview}
              disabled={loading || !federationId}
              sx={{ height: '56px', mt: { xs: 0, sm: '16px' } }}
            >
              {loading ? <CircularProgress size={24} /> : 'View Federation'}
            </Button>
          </Grid>
        </Grid>
      </Paper>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      )}

      {overview ? (
        <>
          <Paper sx={{ p: 3, borderRadius: 2, mb: 3 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
              <FederationIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
              <Typography variant="h6">
                Federation Details: {overview.federation.id}
              </Typography>
            </Box>
            <Divider sx={{ mb: 3 }} />
            
            <Grid container spacing={4}>
              <Grid item xs={12} md={6}>
                <Card variant="outlined" sx={{ p: 2, height: '100%' }}>
                  <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                    <InfoIcon color="primary" sx={{ mr: 1 }} />
                    <Typography variant="subtitle1">Federation Information</Typography>
                  </Box>
                  
                  <Stack spacing={2}>
                    <Box>
                      <Typography variant="caption" color="text.secondary">
                        Federation ID
                      </Typography>
                      <Typography variant="body1">
                        {overview.federation.id}
                      </Typography>
                    </Box>
                    
                    {overview.federation.description && (
                      <Box>
                        <Typography variant="caption" color="text.secondary">
                          Description
                        </Typography>
                        <Typography variant="body1">
                          {overview.federation.description}
                        </Typography>
                      </Box>
                    )}
                    
                    {overview.federation.head && (
                      <Box>
                        <Typography variant="caption" color="text.secondary">
                          Federation DAG Head
                        </Typography>
                        <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
                          <Chip 
                            label={overview.federation.head}
                            size="small"
                            color="primary"
                            variant="outlined"
                            sx={{ maxWidth: '100%', overflowX: 'hidden' }}
                          />
                        </Typography>
                      </Box>
                    )}
                  </Stack>
                </Card>
              </Grid>
              
              <Grid item xs={12} md={6}>
                <Card variant="outlined" sx={{ p: 2, height: '100%' }}>
                  <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                    <TagIcon color="primary" sx={{ mr: 1 }} />
                    <Typography variant="subtitle1">Federation Statistics</Typography>
                  </Box>
                  
                  <Grid container spacing={3}>
                    <Grid item xs={6}>
                      <Card sx={{ p: 2, bgcolor: 'primary.light', color: 'primary.contrastText', textAlign: 'center' }}>
                        <Typography variant="h6">{overview.members.cooperatives.count}</Typography>
                        <Typography variant="caption">Cooperatives</Typography>
                      </Card>
                    </Grid>
                    <Grid item xs={6}>
                      <Card sx={{ p: 2, bgcolor: 'secondary.light', color: 'secondary.contrastText', textAlign: 'center' }}>
                        <Typography variant="h6">{overview.members.communities.count}</Typography>
                        <Typography variant="caption">Communities</Typography>
                      </Card>
                    </Grid>
                    <Grid item xs={12}>
                      <Card sx={{ p: 2, bgcolor: 'success.light', color: 'success.contrastText', textAlign: 'center' }}>
                        <Typography variant="h6">
                          {overview.members.cooperatives.count + overview.members.communities.count}
                        </Typography>
                        <Typography variant="caption">Total Members</Typography>
                      </Card>
                    </Grid>
                  </Grid>
                </Card>
              </Grid>
            </Grid>
          </Paper>
          
          <Paper sx={{ borderRadius: 2 }}>
            <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
              <Tabs 
                value={tabValue} 
                onChange={handleTabChange} 
                aria-label="federation members tabs"
                centered
              >
                <Tab 
                  label={`Cooperatives (${overview.members.cooperatives.count})`} 
                  icon={<CooperativeIcon />} 
                  iconPosition="start"
                />
                <Tab 
                  label={`Communities (${overview.members.communities.count})`} 
                  icon={<CommunityIcon />} 
                  iconPosition="start"
                />
              </Tabs>
            </Box>
            
            <TabPanel value={tabValue} index={0}>
              {renderMemberList(overview.members.cooperatives.items)}
            </TabPanel>
            
            <TabPanel value={tabValue} index={1}>
              {renderMemberList(overview.members.communities.items)}
            </TabPanel>
          </Paper>
        </>
      ) : (
        !loading && !error && (
          <Paper sx={{ p: 4, textAlign: 'center', borderRadius: 2 }}>
            <Typography variant="h6" color="text.secondary">
              Enter a federation ID and click "View Federation" to see the federation overview
            </Typography>
          </Paper>
        )
      )}
    </Box>
  );
};

export default FederationOverviewPage; 