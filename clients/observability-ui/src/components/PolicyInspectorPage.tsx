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
  Accordion,
  AccordionSummary,
  AccordionDetails,
  ListItemAvatar,
  Avatar
} from '@mui/material';
import {
  ExpandMore as ExpandMoreIcon,
  History as HistoryIcon,
  Gavel as GavelIcon,
  VerifiedUser as PolicyIcon,
  ThumbUp as ThumbUpIcon,
  ThumbDown as ThumbDownIcon,
  Person as PersonIcon
} from '@mui/icons-material';
import observabilityApi, { PolicyInfo, PolicyUpdateInfo, VoteInfo } from '../api/observabilityApi';

const PolicyInspectorPage: React.FC = () => {
  const [scopeType, setScopeType] = useState<string>('federation');
  const [scopeId, setScopeId] = useState<string>('');
  const [policyInfo, setPolicyInfo] = useState<PolicyInfo | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Function to fetch policy info
  const fetchPolicyInfo = useCallback(async () => {
    if (!scopeId) {
      setError('Please enter a scope ID');
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const policy = await observabilityApi.getPolicy(scopeType, scopeId);
      setPolicyInfo(policy);
    } catch (err) {
      console.error('Error fetching policy info:', err);
      setError('Failed to fetch policy information. Please try again.');
    } finally {
      setLoading(false);
    }
  }, [scopeType, scopeId]);

  const handleScopeTypeChange = (event: SelectChangeEvent) => {
    setScopeType(event.target.value);
  };

  const handleScopeIdChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setScopeId(event.target.value);
  };

  // Format timestamp
  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  // Render JSON with indentation for better readability
  const renderJsonContent = (content: any) => {
    return (
      <pre style={{ 
        backgroundColor: '#f5f5f5', 
        padding: '16px', 
        borderRadius: '4px',
        overflowX: 'auto', 
        fontSize: '14px',
        lineHeight: '1.5'
      }}>
        {JSON.stringify(content, null, 2)}
      </pre>
    );
  };

  // Render update history
  const renderUpdateHistory = (updates: PolicyUpdateInfo[]) => {
    if (updates.length === 0) {
      return (
        <Typography variant="body2" color="text.secondary">
          No policy updates have been made yet.
        </Typography>
      );
    }

    return (
      <List>
        {updates.map((update, index) => (
          <Accordion key={index} sx={{ mb: 2 }}>
            <AccordionSummary
              expandIcon={<ExpandMoreIcon />}
              sx={{ backgroundColor: '#f5f5f5' }}
            >
              <ListItemIcon>
                <HistoryIcon color="primary" />
              </ListItemIcon>
              <ListItemText
                primary={
                  <Typography variant="subtitle1">
                    Policy Update {index + 1}
                  </Typography>
                }
                secondary={`Proposed by ${update.proposer.substring(0, 16)}... on ${formatTimestamp(update.timestamp)}`}
              />
            </AccordionSummary>
            <AccordionDetails>
              <Typography variant="subtitle2" color="text.secondary" gutterBottom>
                CID
              </Typography>
              <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                {update.cid}
              </Typography>

              <Divider sx={{ my: 2 }} />
              
              <Typography variant="subtitle2" fontWeight="bold" gutterBottom>
                Votes
              </Typography>
              
              {update.votes.length > 0 ? (
                <List dense>
                  {update.votes.map((vote, voteIndex) => (
                    <ListItem key={voteIndex} sx={{ py: 1 }}>
                      <ListItemAvatar>
                        <Avatar sx={{ 
                          bgcolor: vote.decision === 'approve' ? 'success.light' : 'error.light'
                        }}>
                          {vote.decision === 'approve' ? <ThumbUpIcon /> : <ThumbDownIcon />}
                        </Avatar>
                      </ListItemAvatar>
                      <ListItemText
                        primary={
                          <Box sx={{ display: 'flex', alignItems: 'center' }}>
                            <PersonIcon fontSize="small" sx={{ mr: 1 }} />
                            <Typography variant="body2" sx={{ fontWeight: 'medium' }}>
                              {vote.voter.substring(0, 16)}...
                            </Typography>
                          </Box>
                        }
                        secondary={
                          <>
                            <Chip
                              label={vote.decision}
                              size="small"
                              color={vote.decision === 'approve' ? 'success' : 'error'}
                              sx={{ mr: 1 }}
                            />
                            {vote.reason && (
                              <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
                                Reason: {vote.reason}
                              </Typography>
                            )}
                          </>
                        }
                      />
                    </ListItem>
                  ))}
                </List>
              ) : (
                <Typography variant="body2" color="text.secondary">
                  No votes recorded for this update.
                </Typography>
              )}
            </AccordionDetails>
          </Accordion>
        ))}
      </List>
    );
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Typography variant="h5" gutterBottom>
          Policy Inspector
        </Typography>
        <Typography variant="body2" color="text.secondary" paragraph>
          View and analyze active policies for a scope and track policy update history.
          Inspect voting records and policy changes over time.
        </Typography>
        
        <Grid container spacing={2} alignItems="flex-end" sx={{ mt: 1 }}>
          <Grid item xs={12} sm={5}>
            <FormControl fullWidth margin="normal">
              <InputLabel>Scope Type</InputLabel>
              <Select
                value={scopeType}
                label="Scope Type"
                onChange={handleScopeTypeChange}
              >
                <MenuItem value="federation">Federation</MenuItem>
                <MenuItem value="cooperative">Cooperative</MenuItem>
                <MenuItem value="community">Community</MenuItem>
              </Select>
            </FormControl>
          </Grid>
          <Grid item xs={12} sm={5}>
            <TextField
              fullWidth
              label="Scope ID"
              value={scopeId}
              onChange={handleScopeIdChange}
              margin="normal"
            />
          </Grid>
          <Grid item xs={12} sm={2}>
            <Button 
              variant="contained" 
              fullWidth 
              onClick={fetchPolicyInfo}
              disabled={loading || !scopeId}
              sx={{ height: '56px', mt: { xs: 0, sm: '16px' } }}
            >
              {loading ? <CircularProgress size={24} /> : 'Inspect Policy'}
            </Button>
          </Grid>
        </Grid>
      </Paper>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      )}

      {policyInfo ? (
        <Grid container spacing={3}>
          <Grid item xs={12}>
            <Paper sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <PolicyIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
                <Typography variant="h6">Active Policy</Typography>
              </Box>
              <Divider sx={{ mb: 3 }} />
              
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="subtitle2" color="text.secondary">Policy CID</Typography>
                  <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                    {policyInfo.cid}
                  </Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="subtitle2" color="text.secondary">Last Updated</Typography>
                  <Typography variant="body2" paragraph>
                    {formatTimestamp(policyInfo.timestamp)}
                  </Typography>
                </Grid>
              </Grid>
              
              <Typography variant="subtitle1" sx={{ mt: 2, mb: 1 }}>Policy Content</Typography>
              {renderJsonContent(policyInfo.content)}
            </Paper>
          </Grid>
          
          <Grid item xs={12}>
            <Paper sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <GavelIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
                <Typography variant="h6">Policy Update History</Typography>
              </Box>
              <Divider sx={{ mb: 3 }} />
              
              {renderUpdateHistory(policyInfo.update_trail)}
            </Paper>
          </Grid>
        </Grid>
      ) : (
        !loading && !error && (
          <Paper sx={{ p: 4, textAlign: 'center', borderRadius: 2 }}>
            <Typography variant="h6" color="text.secondary">
              Enter scope details and click "Inspect Policy" to view policy information
            </Typography>
          </Paper>
        )
      )}
    </Box>
  );
};

export default PolicyInspectorPage; 