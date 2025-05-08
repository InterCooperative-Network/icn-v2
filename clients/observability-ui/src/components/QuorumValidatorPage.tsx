import React, { useState, useCallback } from 'react';
import { 
  Box, 
  Button, 
  Card, 
  Chip, 
  FormControl, 
  FormControlLabel,
  Checkbox,
  Grid, 
  InputLabel, 
  Paper, 
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
  Avatar
} from '@mui/material';
import {
  Check as CheckIcon,
  Close as CloseIcon,
  Person as PersonIcon,
  VerifiedUser as VerifiedIcon,
  Security as SecurityIcon,
  Warning as WarningIcon,
  Assignment as AssignmentIcon
} from '@mui/icons-material';
import observabilityApi, { QuorumInfo, SignerInfo } from '../api/observabilityApi';

const QuorumValidatorPage: React.FC = () => {
  const [cid, setCid] = useState<string>('');
  const [showSigners, setShowSigners] = useState<boolean>(true);
  const [quorumInfo, setQuorumInfo] = useState<QuorumInfo | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Function to validate quorum
  const validateQuorum = useCallback(async () => {
    if (!cid) {
      setError('Please enter a CID');
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const info = await observabilityApi.validateQuorum(cid, showSigners);
      setQuorumInfo(info);
    } catch (err) {
      console.error('Error validating quorum:', err);
      setError('Failed to validate quorum. Please check the CID and try again.');
    } finally {
      setLoading(false);
    }
  }, [cid, showSigners]);

  const handleCidChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setCid(event.target.value);
  };

  const handleShowSignersChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setShowSigners(event.target.checked);
  };

  // Format timestamp
  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  // Render signers list
  const renderSignersList = (signers: SignerInfo[]) => {
    if (signers.length === 0) {
      return (
        <Typography variant="body2" color="text.secondary">
          No signers found.
        </Typography>
      );
    }

    return (
      <List dense>
        {signers.map((signer, index) => (
          <ListItem key={index} divider>
            <ListItemAvatar>
              <Avatar sx={{ bgcolor: 'primary.light' }}>
                <PersonIcon />
              </Avatar>
            </ListItemAvatar>
            <ListItemText
              primary={
                <Box sx={{ display: 'flex', alignItems: 'center' }}>
                  <Typography variant="body2" sx={{ fontWeight: 'medium', wordBreak: 'break-all' }}>
                    {signer.did}
                  </Typography>
                </Box>
              }
              secondary={
                <Box>
                  {signer.role && (
                    <Chip
                      label={`Role: ${signer.role}`}
                      size="small"
                      color="primary"
                      variant="outlined"
                      sx={{ mr: 1, mt: 0.5 }}
                    />
                  )}
                  {signer.scope && (
                    <Chip
                      label={`Scope: ${signer.scope}`}
                      size="small"
                      color="secondary"
                      variant="outlined"
                      sx={{ mt: 0.5 }}
                    />
                  )}
                </Box>
              }
            />
          </ListItem>
        ))}
      </List>
    );
  };

  // Render required signers list
  const renderRequiredSignersList = (signers: string[]) => {
    if (signers.length === 0) {
      return (
        <Typography variant="body2" color="text.secondary">
          No required signers specified.
        </Typography>
      );
    }

    return (
      <List dense>
        {signers.map((signer, index) => (
          <ListItem key={index} divider>
            <ListItemIcon>
              <AssignmentIcon color="primary" />
            </ListItemIcon>
            <ListItemText
              primary={
                <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
                  {signer}
                </Typography>
              }
            />
          </ListItem>
        ))}
      </List>
    );
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Typography variant="h5" gutterBottom>
          Quorum Proof Validator
        </Typography>
        <Typography variant="body2" color="text.secondary" paragraph>
          Validate quorum proofs on DAG nodes, showing required vs. actual signers.
          Verify that governance actions have met the required threshold.
        </Typography>
        
        <Grid container spacing={2} alignItems="flex-end" sx={{ mt: 1 }}>
          <Grid item xs={12} sm={8}>
            <TextField
              fullWidth
              label="Node CID"
              value={cid}
              onChange={handleCidChange}
              margin="normal"
              placeholder="Enter the CID of the DAG node to validate"
            />
          </Grid>
          <Grid item xs={12} sm={2}>
            <FormControlLabel
              control={
                <Checkbox 
                  checked={showSigners} 
                  onChange={handleShowSignersChange}
                  color="primary"
                />
              }
              label="Show Signers"
              sx={{ mt: 2 }}
            />
          </Grid>
          <Grid item xs={12} sm={2}>
            <Button 
              variant="contained" 
              fullWidth 
              onClick={validateQuorum}
              disabled={loading || !cid}
              sx={{ height: '56px', mt: { xs: 0, sm: '16px' } }}
            >
              {loading ? <CircularProgress size={24} /> : 'Validate'}
            </Button>
          </Grid>
        </Grid>
      </Paper>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      )}

      {quorumInfo ? (
        <Grid container spacing={3}>
          <Grid item xs={12}>
            <Paper sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                {quorumInfo.is_valid ? (
                  <VerifiedIcon fontSize="large" color="success" sx={{ mr: 2 }} />
                ) : (
                  <WarningIcon fontSize="large" color="error" sx={{ mr: 2 }} />
                )}
                <Typography variant="h6">
                  Quorum Validation {quorumInfo.is_valid ? 'Successful' : 'Failed'}
                </Typography>
              </Box>
              <Divider sx={{ mb: 3 }} />
              
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Card variant="outlined" sx={{ p: 2, mb: 2 }}>
                    <Typography variant="subtitle2" color="text.secondary">Node CID</Typography>
                    <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                      {quorumInfo.cid}
                    </Typography>
                    
                    <Typography variant="subtitle2" color="text.secondary">Author</Typography>
                    <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                      {quorumInfo.node.author}
                    </Typography>
                    
                    <Typography variant="subtitle2" color="text.secondary">Timestamp</Typography>
                    <Typography variant="body2" paragraph>
                      {formatTimestamp(quorumInfo.node.timestamp)}
                    </Typography>
                  </Card>
                </Grid>
                
                <Grid item xs={12} md={6}>
                  <Card variant="outlined" sx={{ p: 2, mb: 2 }}>
                    <Typography variant="subtitle2" color="text.secondary">Quorum Status</Typography>
                    <Box sx={{ display: 'flex', alignItems: 'center', my: 1 }}>
                      {quorumInfo.is_valid ? (
                        <Chip 
                          icon={<CheckIcon />} 
                          label="VALID" 
                          color="success" 
                          variant="filled" 
                        />
                      ) : (
                        <Chip 
                          icon={<CloseIcon />} 
                          label="INVALID" 
                          color="error" 
                          variant="filled" 
                        />
                      )}
                    </Box>
                    
                    {quorumInfo.error_message && (
                      <Alert severity="error" sx={{ mt: 2 }}>
                        {quorumInfo.error_message}
                      </Alert>
                    )}
                    
                    <Box sx={{ mt: 2 }}>
                      <Typography variant="subtitle2" color="text.secondary">Required Signers</Typography>
                      <Typography variant="body1" fontWeight="bold">
                        {quorumInfo.required_signers.length}
                      </Typography>
                      
                      <Typography variant="subtitle2" color="text.secondary" sx={{ mt: 1 }}>Actual Signers</Typography>
                      <Typography variant="body1" fontWeight="bold">
                        {quorumInfo.actual_signers.length}
                      </Typography>
                    </Box>
                  </Card>
                </Grid>
              </Grid>
            </Paper>
          </Grid>
          
          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <AssignmentIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
                <Typography variant="h6">Required Signers</Typography>
              </Box>
              <Divider sx={{ mb: 3 }} />
              
              {renderRequiredSignersList(quorumInfo.required_signers)}
            </Paper>
          </Grid>
          
          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <SecurityIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
                <Typography variant="h6">Actual Signers</Typography>
              </Box>
              <Divider sx={{ mb: 3 }} />
              
              {showSigners ? renderSignersList(quorumInfo.actual_signers) : (
                <Typography variant="body2" color="text.secondary">
                  Enable "Show Signers" to view detailed signer information.
                </Typography>
              )}
            </Paper>
          </Grid>
        </Grid>
      ) : (
        !loading && !error && (
          <Paper sx={{ p: 4, textAlign: 'center', borderRadius: 2 }}>
            <Typography variant="h6" color="text.secondary">
              Enter a DAG node CID and click "Validate" to check quorum proof
            </Typography>
          </Paper>
        )
      )}
    </Box>
  );
};

export default QuorumValidatorPage; 