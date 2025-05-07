import React, { useState, useCallback, useEffect } from 'react';
import { 
  Box, 
  Button, 
  Card, 
  CircularProgress, 
  Divider, 
  FormControl, 
  FormHelperText, 
  Grid, 
  InputLabel, 
  MenuItem, 
  Paper, 
  Select, 
  SelectChangeEvent, 
  TextField, 
  Typography,
  Alert,
  Snackbar,
  Switch,
  FormControlLabel,
  Stepper,
  Step,
  StepLabel,
} from '@mui/material';
import {
  Add as AddIcon,
  Send as SendIcon,
  GavelRounded as ProposalIcon,
  AdminPanelSettings as FederationIcon
} from '@mui/icons-material';
import { useDemoMode } from '../demo/DemoModeContext';
import { getSeededData } from '../demo/seedData';
import observabilityApi, { ProposalSubmission } from '../api/observabilityApi';

const ProposalCreationPage: React.FC = () => {
  // Get demo mode and API from context
  const { isDemoMode, api } = useDemoMode();
  const demoData = isDemoMode ? getSeededData() : null;

  // Form state
  const [keyFile, setKeyFile] = useState<string>('');
  const [federation, setFederation] = useState<string>('');
  const [title, setTitle] = useState<string>('');
  const [description, setDescription] = useState<string>('');
  const [proposalType, setProposalType] = useState<string>('textProposal');
  const [votingThreshold, setVotingThreshold] = useState<string>('majority');
  const [votingDuration, setVotingDuration] = useState<string>('86400');
  const [executionCid, setExecutionCid] = useState<string>('');
  const [threadCid, setThreadCid] = useState<string>('');
  const [parameters, setParameters] = useState<string>('');
  
  // Scoped proposal state
  const [isScoped, setIsScoped] = useState<boolean>(false);
  const [scopeType, setScopeType] = useState<string>('cooperative');
  const [scopeId, setScopeId] = useState<string>('');
  
  // UI state
  const [activeStep, setActiveStep] = useState(0);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<boolean>(false);
  const [successMessage, setSuccessMessage] = useState<string>('');

  // Auto-populate some fields in demo mode
  useEffect(() => {
    if (isDemoMode && demoData) {
      // Set federation ID for demo
      if (!federation) {
        setFederation(demoData.federation.id);
      }
      
      // Set a demo key file path
      if (!keyFile) {
        setKeyFile('/home/user/demo/key.jwk');
      }
    }
  }, [isDemoMode, demoData, federation, keyFile]);

  // Step definitions
  const steps = ['Basic Information', 'Proposal Details', 'Review & Submit'];

  // Handle form input changes
  const handleKeyFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setKeyFile(event.target.value);
  };

  const handleFederationChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setFederation(event.target.value);
  };

  const handleTitleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setTitle(event.target.value);
  };

  const handleDescriptionChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    setDescription(event.target.value);
  };

  const handleProposalTypeChange = (event: SelectChangeEvent) => {
    setProposalType(event.target.value);
  };

  const handleVotingThresholdChange = (event: SelectChangeEvent) => {
    setVotingThreshold(event.target.value);
  };

  const handleVotingDurationChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setVotingDuration(event.target.value);
  };

  const handleExecutionCidChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setExecutionCid(event.target.value);
  };

  const handleThreadCidChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setThreadCid(event.target.value);
  };

  const handleParametersChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    setParameters(event.target.value);
  };

  const handleIsScopedChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setIsScoped(event.target.checked);
  };

  const handleScopeTypeChange = (event: SelectChangeEvent) => {
    setScopeType(event.target.value);
  };

  const handleScopeIdChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setScopeId(event.target.value);
  };

  // Step navigation
  const handleNext = () => {
    setActiveStep((prevActiveStep) => prevActiveStep + 1);
  };

  const handleBack = () => {
    setActiveStep((prevActiveStep) => prevActiveStep - 1);
  };

  const handleReset = () => {
    setActiveStep(0);
    setKeyFile('');
    setFederation('');
    setTitle('');
    setDescription('');
    setProposalType('textProposal');
    setVotingThreshold('majority');
    setVotingDuration('86400');
    setExecutionCid('');
    setThreadCid('');
    setParameters('');
    setIsScoped(false);
    setScopeType('cooperative');
    setScopeId('');
  };

  // Validation functions
  const validateStep1 = () => {
    return keyFile.trim() !== '' && federation.trim() !== '' && title.trim() !== '' && description.trim() !== '';
  };

  const validateStep2 = () => {
    // Code execution and upgrade proposals require an execution CID
    if ((proposalType === 'codeExecution' || proposalType === 'codeUpgrade') && executionCid.trim() === '') {
      return false;
    }
    
    // If scoped, both scope type and ID are required
    if (isScoped && (scopeType === '' || scopeId.trim() === '')) {
      return false;
    }
    
    // Parameters, if provided, should be valid JSON
    if (parameters.trim() !== '') {
      try {
        JSON.parse(parameters);
      } catch (e) {
        return false;
      }
    }
    
    return true;
  };

  // Create proposal submission object
  const createProposalSubmission = (): ProposalSubmission => {
    const proposal: ProposalSubmission = {
      key_file: keyFile,
      federation: federation,
      title: title,
      description: description,
      proposal_type: proposalType,
      voting_threshold: votingThreshold,
      voting_duration: votingDuration,
    };

    if (executionCid.trim() !== '') {
      proposal.execution_cid = executionCid;
    }

    if (threadCid.trim() !== '') {
      proposal.thread_cid = threadCid;
    }

    if (parameters.trim() !== '') {
      proposal.parameters = parameters;
    }

    if (isScoped) {
      proposal.scope_type = scopeType;
      proposal.scope_id = scopeId;
    }

    return proposal;
  };

  // Submit proposal
  const submitProposal = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {
      const proposal = createProposalSubmission();
      
      // Use the API from context
      const result = await api.submitProposal(proposal);
      
      setSuccess(true);
      setSuccessMessage(`Proposal "${result.title}" submitted successfully with ID: ${result.proposal_id}`);
      
      // Reset form on successful submission
      handleReset();
    } catch (err: any) {
      console.error('Error submitting proposal:', err);
      setError(err.message || 'Failed to submit proposal. Please try again.');
    } finally {
      setLoading(false);
    }
  }, [
    keyFile, federation, title, description, proposalType, 
    votingThreshold, votingDuration, executionCid, threadCid, 
    parameters, isScoped, scopeType, scopeId, api
  ]);

  // Render step content
  const getStepContent = (step: number) => {
    switch (step) {
      case 0:
        return (
          <Box>
            <Grid container spacing={3}>
              <Grid item xs={12} md={6}>
                <TextField
                  fullWidth
                  label="Key File Path"
                  value={keyFile}
                  onChange={handleKeyFileChange}
                  margin="normal"
                  required
                  helperText="Path to the key file for signing the proposal"
                />
              </Grid>
              <Grid item xs={12} md={6}>
                <TextField
                  fullWidth
                  label="Federation DID"
                  value={federation}
                  onChange={handleFederationChange}
                  margin="normal"
                  required
                  helperText="DID of the federation this proposal is for"
                />
              </Grid>
              <Grid item xs={12}>
                <TextField
                  fullWidth
                  label="Title"
                  value={title}
                  onChange={handleTitleChange}
                  margin="normal"
                  required
                  helperText="A clear, concise title for your proposal"
                />
              </Grid>
              <Grid item xs={12}>
                <TextField
                  fullWidth
                  label="Description"
                  value={description}
                  onChange={handleDescriptionChange}
                  margin="normal"
                  required
                  multiline
                  rows={4}
                  helperText="Detailed description explaining the purpose and impact of this proposal"
                />
              </Grid>
            </Grid>
          </Box>
        );
      case 1:
        return (
          <Box>
            <Grid container spacing={3}>
              <Grid item xs={12} md={4}>
                <FormControl fullWidth margin="normal">
                  <InputLabel>Proposal Type</InputLabel>
                  <Select
                    value={proposalType}
                    label="Proposal Type"
                    onChange={handleProposalTypeChange}
                  >
                    <MenuItem value="textProposal">Text Proposal</MenuItem>
                    <MenuItem value="codeExecution">Code Execution</MenuItem>
                    <MenuItem value="configChange">Configuration Change</MenuItem>
                    <MenuItem value="memberAddition">Member Addition</MenuItem>
                    <MenuItem value="memberRemoval">Member Removal</MenuItem>
                    <MenuItem value="codeUpgrade">Code Upgrade</MenuItem>
                    <MenuItem value="custom">Custom</MenuItem>
                  </Select>
                  <FormHelperText>The type of proposal being submitted</FormHelperText>
                </FormControl>
              </Grid>
              <Grid item xs={12} md={4}>
                <FormControl fullWidth margin="normal">
                  <InputLabel>Voting Threshold</InputLabel>
                  <Select
                    value={votingThreshold}
                    label="Voting Threshold"
                    onChange={handleVotingThresholdChange}
                  >
                    <MenuItem value="majority">Majority (>50%)</MenuItem>
                    <MenuItem value="unanimous">Unanimous (100%)</MenuItem>
                    <MenuItem value="percentage:66">Supermajority (66%)</MenuItem>
                    <MenuItem value="percentage:75">Three-quarters (75%)</MenuItem>
                    <MenuItem value="percentage:33">One-third (33%)</MenuItem>
                  </Select>
                  <FormHelperText>Required threshold for proposal to pass</FormHelperText>
                </FormControl>
              </Grid>
              <Grid item xs={12} md={4}>
                <TextField
                  fullWidth
                  label="Voting Duration (seconds)"
                  value={votingDuration}
                  onChange={handleVotingDurationChange}
                  margin="normal"
                  helperText="Duration in seconds, or 'openEnded' for no time limit"
                />
              </Grid>
              
              {(proposalType === 'codeExecution' || proposalType === 'codeUpgrade') && (
                <Grid item xs={12}>
                  <TextField
                    fullWidth
                    label="Execution CID"
                    value={executionCid}
                    onChange={handleExecutionCidChange}
                    margin="normal"
                    required
                    helperText="CID of the code to execute or upgrade to"
                  />
                </Grid>
              )}
              
              <Grid item xs={12}>
                <TextField
                  fullWidth
                  label="Thread CID (Optional)"
                  value={threadCid}
                  onChange={handleThreadCidChange}
                  margin="normal"
                  helperText="CID of the AgoraNet thread containing the proposal discussion"
                />
              </Grid>
              
              <Grid item xs={12}>
                <TextField
                  fullWidth
                  label="Parameters (JSON)"
                  value={parameters}
                  onChange={handleParametersChange}
                  margin="normal"
                  multiline
                  rows={3}
                  helperText="Additional parameters in JSON format (must be valid JSON)"
                />
              </Grid>
              
              <Grid item xs={12}>
                <Divider sx={{ my: 2 }} />
                <FormControlLabel
                  control={
                    <Switch 
                      checked={isScoped} 
                      onChange={handleIsScopedChange}
                      color="primary"
                    />
                  }
                  label="Is this a scoped proposal?"
                />
                <Typography variant="caption" color="text.secondary">
                  Scoped proposals are tied to a specific cooperative or community within the federation
                </Typography>
              </Grid>
              
              {isScoped && (
                <>
                  <Grid item xs={12} md={6}>
                    <FormControl fullWidth margin="normal">
                      <InputLabel>Scope Type</InputLabel>
                      <Select
                        value={scopeType}
                        label="Scope Type"
                        onChange={handleScopeTypeChange}
                      >
                        <MenuItem value="cooperative">Cooperative</MenuItem>
                        <MenuItem value="community">Community</MenuItem>
                      </Select>
                      <FormHelperText>The type of scope this proposal applies to</FormHelperText>
                    </FormControl>
                  </Grid>
                  <Grid item xs={12} md={6}>
                    <TextField
                      fullWidth
                      label="Scope ID"
                      value={scopeId}
                      onChange={handleScopeIdChange}
                      margin="normal"
                      required
                      helperText="ID of the cooperative or community"
                    />
                  </Grid>
                </>
              )}
            </Grid>
          </Box>
        );
      case 2:
        return (
          <Box>
            <Paper elevation={0} sx={{ p: 3, bgcolor: '#f8f9fa', mb: 3, borderRadius: 2 }}>
              <Typography variant="subtitle1" fontWeight="bold" gutterBottom>
                Basic Information
              </Typography>
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="caption" color="text.secondary">Federation</Typography>
                  <Typography variant="body2" paragraph>{federation}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="caption" color="text.secondary">Key File</Typography>
                  <Typography variant="body2" paragraph>{keyFile}</Typography>
                </Grid>
                <Grid item xs={12}>
                  <Typography variant="caption" color="text.secondary">Title</Typography>
                  <Typography variant="body2" paragraph>{title}</Typography>
                </Grid>
                <Grid item xs={12}>
                  <Typography variant="caption" color="text.secondary">Description</Typography>
                  <Typography variant="body2" paragraph>{description}</Typography>
                </Grid>
              </Grid>
              
              <Typography variant="subtitle1" fontWeight="bold" gutterBottom sx={{ mt: 2 }}>
                Proposal Configuration
              </Typography>
              <Grid container spacing={2}>
                <Grid item xs={12} md={4}>
                  <Typography variant="caption" color="text.secondary">Type</Typography>
                  <Typography variant="body2" paragraph>{proposalType}</Typography>
                </Grid>
                <Grid item xs={12} md={4}>
                  <Typography variant="caption" color="text.secondary">Voting Threshold</Typography>
                  <Typography variant="body2" paragraph>{votingThreshold}</Typography>
                </Grid>
                <Grid item xs={12} md={4}>
                  <Typography variant="caption" color="text.secondary">Voting Duration</Typography>
                  <Typography variant="body2" paragraph>{votingDuration} seconds</Typography>
                </Grid>
              </Grid>
              
              {executionCid && (
                <Box sx={{ mt: 2 }}>
                  <Typography variant="caption" color="text.secondary">Execution CID</Typography>
                  <Typography variant="body2" paragraph>{executionCid}</Typography>
                </Box>
              )}
              
              {threadCid && (
                <Box sx={{ mt: 2 }}>
                  <Typography variant="caption" color="text.secondary">Thread CID</Typography>
                  <Typography variant="body2" paragraph>{threadCid}</Typography>
                </Box>
              )}
              
              {parameters && (
                <Box sx={{ mt: 2 }}>
                  <Typography variant="caption" color="text.secondary">Parameters</Typography>
                  <pre style={{ 
                    backgroundColor: '#f0f0f0', 
                    padding: '8px', 
                    borderRadius: '4px',
                    fontSize: '0.8rem',
                    overflowX: 'auto'
                  }}>
                    {parameters}
                  </pre>
                </Box>
              )}
              
              {isScoped && (
                <Box sx={{ mt: 2 }}>
                  <Typography variant="subtitle1" fontWeight="bold" gutterBottom>
                    Scope Information
                  </Typography>
                  <Grid container spacing={2}>
                    <Grid item xs={12} md={6}>
                      <Typography variant="caption" color="text.secondary">Scope Type</Typography>
                      <Typography variant="body2">{scopeType}</Typography>
                    </Grid>
                    <Grid item xs={12} md={6}>
                      <Typography variant="caption" color="text.secondary">Scope ID</Typography>
                      <Typography variant="body2">{scopeId}</Typography>
                    </Grid>
                  </Grid>
                </Box>
              )}
            </Paper>
            
            {error && (
              <Alert severity="error" sx={{ mb: 3 }}>
                {error}
              </Alert>
            )}
          </Box>
        );
      default:
        return 'Unknown step';
    }
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
          <ProposalIcon fontSize="large" color="primary" sx={{ mr: 2 }} />
          <Typography variant="h5" gutterBottom component="div" sx={{ mb: 0 }}>
            Create New Proposal
          </Typography>
        </Box>
        
        <Typography variant="body2" color="text.secondary" paragraph>
          Submit a new governance proposal to your federation. Proposals can be general or scoped to specific cooperatives or communities.
          {isDemoMode && ' In demo mode, your proposal will be immediately visible in the Activity Log and DAG Viewer.'}
        </Typography>
        
        {isDemoMode && (
          <Alert severity="info" sx={{ mb: 3 }}>
            Demo mode active: Your proposal will be recorded in the demo system and can be viewed in the dashboard. 
            The proposal won't be sent to a real ICN API.
          </Alert>
        )}
        
        <Stepper activeStep={activeStep} sx={{ pt: 3, pb: 5 }}>
          {steps.map((label) => (
            <Step key={label}>
              <StepLabel>{label}</StepLabel>
            </Step>
          ))}
        </Stepper>
        
        {activeStep === steps.length ? (
          <Paper sx={{ p: 3, borderRadius: 2, textAlign: 'center', bgcolor: '#e8f5e9' }}>
            <Typography variant="h6" gutterBottom>
              Proposal submitted successfully!
            </Typography>
            <Typography variant="body1" paragraph>
              Your proposal has been created and is now visible to federation members.
            </Typography>
            <Button 
              onClick={handleReset} 
              variant="outlined" 
              color="primary"
              sx={{ mt: 2 }}
            >
              Create Another Proposal
            </Button>
          </Paper>
        ) : (
          <>
            {getStepContent(activeStep)}
            <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 3 }}>
              {activeStep !== 0 && (
                <Button
                  onClick={handleBack}
                  sx={{ mr: 1 }}
                >
                  Back
                </Button>
              )}
              
              {activeStep === steps.length - 1 ? (
                <Button
                  variant="contained"
                  onClick={submitProposal}
                  disabled={loading || !validateStep2()}
                  startIcon={loading ? <CircularProgress size={20} /> : <SendIcon />}
                >
                  {loading ? 'Submitting...' : 'Submit Proposal'}
                </Button>
              ) : (
                <Button
                  variant="contained"
                  onClick={handleNext}
                  disabled={(activeStep === 0 && !validateStep1()) || (activeStep === 1 && !validateStep2())}
                >
                  Next
                </Button>
              )}
            </Box>
          </>
        )}
      </Paper>
      
      <Snackbar 
        open={success} 
        autoHideDuration={6000} 
        onClose={() => setSuccess(false)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert onClose={() => setSuccess(false)} severity="success" sx={{ width: '100%' }}>
          {successMessage}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default ProposalCreationPage; 