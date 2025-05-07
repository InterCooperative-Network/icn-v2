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
  IconButton,
  Collapse,
  Tooltip,
  ButtonGroup,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Snackbar
} from '@mui/material';
import {
  Timeline as TimelineIcon,
  Receipt as ReceiptIcon,
  Ballot as BallotIcon,
  Policy as PolicyIcon,
  HowToVote as VoteIcon,
  Group as GroupIcon,
  Event as EventIcon,
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
  Info as InfoIcon,
  ThumbUp as ThumbUpIcon,
  ThumbDown as ThumbDownIcon
} from '@mui/icons-material';
import { useDemoMode } from '../demo/DemoModeContext';
import { ActivityEvent } from '../api/observabilityApi';

const ActivityLogPage: React.FC = () => {
  const { api, isDemoMode } = useDemoMode();
  const [scopeType, setScopeType] = useState<string>('federation');
  const [scopeId, setScopeId] = useState<string>('');
  const [limit, setLimit] = useState<number>(50);
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedItems, setExpandedItems] = useState<Record<string, boolean>>({});
  
  // Demo mode voting functionality
  const [voteDialogOpen, setVoteDialogOpen] = useState(false);
  const [selectedProposal, setSelectedProposal] = useState<any>(null);
  const [voteDecision, setVoteDecision] = useState<string>('');
  const [voteReason, setVoteReason] = useState<string>('');
  const [voteLoading, setVoteLoading] = useState(false);
  const [voteSuccess, setVoteSuccess] = useState(false);
  const [voteMessage, setVoteMessage] = useState('');
  
  // Automatically set federation ID in demo mode
  React.useEffect(() => {
    if (isDemoMode && scopeId === '') {
      // Default federation for demo mode
      setScopeId('fed-democoop-network');
    }
  }, [isDemoMode, scopeId]);

  // Function to fetch activity log
  const fetchActivityLog = useCallback(async () => {
    if (!scopeId) {
      setError('Please enter a scope ID');
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const events = await api.getActivityLog(scopeType, scopeId, limit);
      setActivities(events);
    } catch (err) {
      console.error('Error fetching activity log:', err);
      setError('Failed to fetch activity log. Please try again.');
    } finally {
      setLoading(false);
    }
  }, [scopeType, scopeId, limit, api]);
  
  // Automatically fetch activities when in demo mode and scopeId changes
  React.useEffect(() => {
    if (isDemoMode && scopeId === 'fed-democoop-network') {
      fetchActivityLog();
    }
  }, [isDemoMode, scopeId, fetchActivityLog]);

  const handleScopeTypeChange = (event: SelectChangeEvent) => {
    setScopeType(event.target.value);
  };

  const handleScopeIdChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setScopeId(event.target.value);
  };

  const handleLimitChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setLimit(parseInt(event.target.value, 10) || 50);
  };

  const toggleItemExpanded = (index: number) => {
    setExpandedItems(prev => ({
      ...prev,
      [index]: !prev[index]
    }));
  };
  
  // Vote dialog handlers
  const openVoteDialog = (proposal: any, decision: string) => {
    setSelectedProposal(proposal);
    setVoteDecision(decision);
    setVoteReason(decision === 'approve' ? 
      'I support this proposal because it aligns with our federation values.' : 
      'I cannot support this proposal in its current form.');
    setVoteDialogOpen(true);
  };
  
  const handleVoteDialogClose = () => {
    setVoteDialogOpen(false);
    setSelectedProposal(null);
    setVoteDecision('');
    setVoteReason('');
  };
  
  const handleVoteSubmit = async () => {
    if (!selectedProposal || !voteDecision) return;
    
    setVoteLoading(true);
    
    try {
      // Submit vote using demo API
      const result = await api.voteOnProposal(
        selectedProposal.proposal_id,
        '/demo/user/key.jwk', // Dummy key file
        voteDecision,
        voteReason
      );
      
      setVoteSuccess(true);
      setVoteMessage(`Vote ${voteDecision} recorded successfully!`);
      
      // Refresh activities
      setTimeout(() => {
        fetchActivityLog();
      }, 1000);
      
      handleVoteDialogClose();
    } catch (err: any) {
      setError(err.message || 'Failed to submit vote');
    } finally {
      setVoteLoading(false);
    }
  };

  // Format timestamp
  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  // Get icon for activity type
  const getActivityIcon = (activityType: string) => {
    switch (activityType.toLowerCase()) {
      case 'proposal submitted':
        return <BallotIcon color="primary" />;
      case 'vote cast':
        return <VoteIcon color="info" />;
      case 'policy changed':
        return <PolicyIcon color="secondary" />;
      case 'federation join':
      case 'member joined':
        return <GroupIcon color="success" />;
      default:
        return <EventIcon color="action" />;
    }
  };

  // Get chip color for activity type
  const getActivityChipColor = (activityType: string): "default" | "primary" | "secondary" | "error" | "info" | "success" | "warning" => {
    switch (activityType.toLowerCase()) {
      case 'proposal submitted':
        return 'primary';
      case 'vote cast':
        return 'info';
      case 'policy changed':
        return 'secondary';
      case 'federation join':
      case 'member joined':
        return 'success';
      default:
        return 'default';
    }
  };
  
  // Check if proposal is votable (in demo mode)
  const isProposalVotable = (activity: ActivityEvent) => {
    if (!isDemoMode) return false;
    if (activity.activity_type !== 'Proposal Submitted') return false;
    
    const details = activity.details;
    if (!details || !details.proposal_id) return false;
    
    // In a real system we would check if:
    // 1. Proposal is active
    // 2. User hasn't voted already
    // 3. User has permission to vote
    // For demo, we'll always allow voting
    return true;
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Typography variant="h5" gutterBottom>
          Governance Activity Log
        </Typography>
        <Typography variant="body2" color="text.secondary" paragraph>
          Track recent governance actions like proposals, votes, policy changes, and federation joins.
          Monitor the activity across scopes to maintain transparency.
        </Typography>
        
        <Grid container spacing={2} alignItems="flex-end" sx={{ mt: 1 }}>
          <Grid item xs={12} sm={4}>
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
          <Grid item xs={12} sm={4}>
            <TextField
              fullWidth
              label="Scope ID"
              value={scopeId}
              onChange={handleScopeIdChange}
              margin="normal"
            />
          </Grid>
          <Grid item xs={12} sm={2}>
            <TextField
              fullWidth
              label="Limit"
              type="number"
              value={limit}
              onChange={handleLimitChange}
              margin="normal"
              inputProps={{ min: 1, max: 500 }}
            />
          </Grid>
          <Grid item xs={12} sm={2}>
            <Button 
              variant="contained" 
              fullWidth 
              onClick={fetchActivityLog}
              disabled={loading || !scopeId}
              sx={{ height: '56px', mt: { xs: 0, sm: '16px' } }}
            >
              {loading ? <CircularProgress size={24} /> : 'View Activity'}
            </Button>
          </Grid>
        </Grid>
      </Paper>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      )}

      {activities.length > 0 ? (
        <Paper sx={{ p: 3, borderRadius: 2 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
            <TimelineIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
            <Typography variant="h6">
              Activity Timeline for {scopeType} '{scopeId}'
            </Typography>
          </Box>
          <Divider sx={{ mb: 3 }} />
          
          <List sx={{ width: '100%' }}>
            {activities.map((activity, index) => (
              <Card key={index} sx={{ mb: 2, borderRadius: 2, overflow: 'visible' }}>
                <ListItem 
                  alignItems="flex-start"
                  secondaryAction={
                    <IconButton 
                      edge="end" 
                      onClick={() => toggleItemExpanded(index)}
                      aria-expanded={expandedItems[index]}
                      aria-label="show more"
                    >
                      {expandedItems[index] ? <ExpandLessIcon /> : <ExpandMoreIcon />}
                    </IconButton>
                  }
                  sx={{ 
                    py: 2,
                    borderBottom: expandedItems[index] ? '1px solid rgba(0, 0, 0, 0.12)' : 'none'
                  }}
                >
                  <ListItemIcon>
                    {getActivityIcon(activity.activity_type)}
                  </ListItemIcon>
                  <ListItemText
                    primary={
                      <Box sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 1 }}>
                        <Typography variant="subtitle1" component="span">
                          {activity.description}
                        </Typography>
                        <Chip 
                          label={activity.activity_type} 
                          size="small" 
                          color={getActivityChipColor(activity.activity_type)}
                        />
                        
                        {/* Vote buttons for proposals in demo mode */}
                        {isDemoMode && isProposalVotable(activity) && (
                          <ButtonGroup size="small" sx={{ ml: 'auto' }}>
                            <Tooltip title="Vote to approve">
                              <Button 
                                startIcon={<ThumbUpIcon />} 
                                color="success" 
                                variant="outlined"
                                onClick={() => openVoteDialog(activity.details, 'approve')}
                              >
                                Approve
                              </Button>
                            </Tooltip>
                            <Tooltip title="Vote to reject">
                              <Button 
                                startIcon={<ThumbDownIcon />} 
                                color="error" 
                                variant="outlined"
                                onClick={() => openVoteDialog(activity.details, 'reject')}
                              >
                                Reject
                              </Button>
                            </Tooltip>
                          </ButtonGroup>
                        )}
                      </Box>
                    }
                    secondary={
                      <>
                        <Typography component="span" variant="body2" color="text.primary" sx={{ display: 'block' }}>
                          {formatTimestamp(activity.timestamp)}
                        </Typography>
                        <Typography component="span" variant="body2" color="text.secondary" sx={{ display: 'block', mt: 0.5 }}>
                          Actor: {activity.actor}
                        </Typography>
                      </>
                    }
                  />
                </ListItem>
                <Collapse in={expandedItems[index]} timeout="auto" unmountOnExit>
                  <Box sx={{ p: 2, pt: 1 }}>
                    <Typography variant="subtitle2" color="text.secondary" gutterBottom>
                      Activity Details
                    </Typography>
                    <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                      CID: {activity.cid}
                    </Typography>
                    
                    {activity.details && (
                      <Box sx={{ 
                        backgroundColor: '#f5f5f5', 
                        p: 2, 
                        borderRadius: 1,
                        mt: 1
                      }}>
                        <Typography variant="subtitle2" gutterBottom>
                          Payload Data
                        </Typography>
                        <pre style={{ 
                          margin: 0, 
                          fontSize: '0.8rem', 
                          overflowX: 'auto' 
                        }}>
                          {JSON.stringify(activity.details, null, 2)}
                        </pre>
                      </Box>
                    )}
                  </Box>
                </Collapse>
              </Card>
            ))}
          </List>
        </Paper>
      ) : (
        !loading && !error && (
          <Paper sx={{ p: 4, textAlign: 'center', borderRadius: 2 }}>
            <Typography variant="h6" color="text.secondary">
              Enter scope details and click "View Activity" to see the governance activity log
            </Typography>
          </Paper>
        )
      )}
      
      {/* Vote Dialog */}
      <Dialog 
        open={voteDialogOpen} 
        onClose={handleVoteDialogClose}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>
          {voteDecision === 'approve' ? 
            <Box sx={{ display: 'flex', alignItems: 'center', color: 'success.main' }}>
              <ThumbUpIcon sx={{ mr: 1 }} />
              Approve Proposal
            </Box> : 
            <Box sx={{ display: 'flex', alignItems: 'center', color: 'error.main' }}>
              <ThumbDownIcon sx={{ mr: 1 }} />
              Reject Proposal
            </Box>
          }
        </DialogTitle>
        <DialogContent>
          {selectedProposal && (
            <>
              <DialogContentText>
                You are about to {voteDecision} the proposal:
              </DialogContentText>
              <Typography variant="subtitle1" sx={{ mt: 2 }}>
                {selectedProposal.proposal_title}
              </Typography>
              
              <TextField
                fullWidth
                label="Reason for your vote"
                multiline
                rows={4}
                value={voteReason}
                onChange={(e) => setVoteReason(e.target.value)}
                margin="normal"
              />
              
              <Typography variant="caption" color="text.secondary">
                Your vote will be recorded on the DAG and visible to all federation members.
              </Typography>
            </>
          )}
        </DialogContent>
        <DialogActions sx={{ p: 2 }}>
          <Button onClick={handleVoteDialogClose} disabled={voteLoading}>
            Cancel
          </Button>
          <Button 
            onClick={handleVoteSubmit} 
            variant="contained" 
            color={voteDecision === 'approve' ? 'success' : 'error'}
            disabled={voteLoading}
            startIcon={voteLoading ? <CircularProgress size={20} /> : null}
          >
            {voteLoading ? 'Submitting...' : 'Submit Vote'}
          </Button>
        </DialogActions>
      </Dialog>
      
      {/* Success Snackbar */}
      <Snackbar
        open={voteSuccess}
        autoHideDuration={5000}
        onClose={() => setVoteSuccess(false)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert 
          onClose={() => setVoteSuccess(false)} 
          severity="success" 
          sx={{ width: '100%' }}
        >
          {voteMessage}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default ActivityLogPage; 