import React, { useState, useEffect } from 'react';
import { 
  Box, 
  FormControlLabel, 
  Switch, 
  Typography, 
  Snackbar,
  Alert,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Chip,
  IconButton,
  Tooltip
} from '@mui/material';
import {
  Info as InfoIcon,
  Schema as DemoIcon
} from '@mui/icons-material';

interface DemoModeToggleProps {
  isDemoMode: boolean;
  onToggle: (isDemoMode: boolean) => void;
}

const DemoModeToggle: React.FC<DemoModeToggleProps> = ({ isDemoMode, onToggle }) => {
  const [infoOpen, setInfoOpen] = useState(false);
  const [notificationOpen, setNotificationOpen] = useState(false);

  // Show notification when demo mode is toggled
  useEffect(() => {
    setNotificationOpen(true);
  }, [isDemoMode]);

  const handleToggle = (event: React.ChangeEvent<HTMLInputElement>) => {
    onToggle(event.target.checked);
  };

  const handleCloseInfo = () => {
    setInfoOpen(false);
  };

  const handleCloseNotification = () => {
    setNotificationOpen(false);
  };

  return (
    <>
      <Box sx={{ 
        display: 'flex', 
        alignItems: 'center',
        bgcolor: isDemoMode ? 'rgba(0, 200, 83, 0.08)' : undefined,
        borderRadius: 1,
        p: isDemoMode ? 1 : 0
      }}>
        <FormControlLabel
          control={
            <Switch
              checked={isDemoMode}
              onChange={handleToggle}
              color="success"
            />
          }
          label={
            <Box sx={{ display: 'flex', alignItems: 'center' }}>
              {isDemoMode && <DemoIcon sx={{ mr: 0.5, color: 'success.main' }} fontSize="small" />}
              <Typography variant="body2" color={isDemoMode ? 'success.main' : 'text.secondary'}>
                Demo Mode
              </Typography>
            </Box>
          }
        />
        
        <Tooltip title="Learn about demo mode">
          <IconButton size="small" onClick={() => setInfoOpen(true)}>
            <InfoIcon fontSize="small" color="action" />
          </IconButton>
        </Tooltip>
        
        {isDemoMode && (
          <Chip 
            label="Using seeded federation data" 
            size="small" 
            color="success" 
            variant="outlined"
            sx={{ ml: 1 }}
          />
        )}
      </Box>

      {/* Info Dialog */}
      <Dialog open={infoOpen} onClose={handleCloseInfo}>
        <DialogTitle>
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <DemoIcon sx={{ mr: 1, color: 'success.main' }} />
            Demo Mode Information
          </Box>
        </DialogTitle>
        <DialogContent>
          <DialogContentText paragraph>
            Demo Mode provides a simulated federation environment with pre-populated data, allowing you to 
            explore and interact with all dashboard features without requiring a running ICN CLI or real federation.
          </DialogContentText>
          
          <Typography variant="subtitle2" gutterBottom>Demo Features:</Typography>
          <ul>
            <li>Complete seeded federation with cooperatives and communities</li>
            <li>Active and historical governance proposals</li>
            <li>Interactive voting and proposal creation</li>
            <li>DAG visualization with node relationships</li>
            <li>Policy information and history</li>
          </ul>
          
          <Typography variant="body2" color="text.secondary" paragraph>
            All actions in demo mode are stored in memory and will be reset if the page is refreshed.
            Your actions will be processed by the demo API service, not sent to any real ICN API.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={handleCloseInfo}>Close</Button>
        </DialogActions>
      </Dialog>

      {/* Notification Snackbar */}
      <Snackbar 
        open={notificationOpen} 
        autoHideDuration={3000} 
        onClose={handleCloseNotification}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert 
          onClose={handleCloseNotification} 
          severity={isDemoMode ? 'success' : 'info'} 
          variant="filled"
          sx={{ width: '100%' }}
        >
          {isDemoMode 
            ? 'Demo mode activated! Using seeded federation data.' 
            : 'Demo mode deactivated. Using real API.'}
        </Alert>
      </Snackbar>
    </>
  );
};

export default DemoModeToggle; 