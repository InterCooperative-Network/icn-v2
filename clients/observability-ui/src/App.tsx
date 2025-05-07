import React, { useState } from 'react';
import { Routes, Route, useLocation, useNavigate } from 'react-router-dom';
import { 
  AppBar, 
  Box, 
  CssBaseline, 
  Drawer, 
  IconButton, 
  List, 
  ListItem, 
  ListItemButton, 
  ListItemIcon, 
  ListItemText, 
  Toolbar, 
  Typography,
  useTheme,
  useMediaQuery
} from '@mui/material';
import {
  Menu as MenuIcon,
  AccountTree as DagIcon,
  Policy as PolicyIcon,
  CheckCircle as QuorumIcon,
  Timeline as ActivityIcon,
  Group as FederationIcon,
  Dashboard as DashboardIcon,
  Add as AddIcon
} from '@mui/icons-material';

// Import our dashboard components
import DashboardPage from './components/DashboardPage';
import DagViewPage from './components/DagViewPage';
import PolicyInspectorPage from './components/PolicyInspectorPage';
import QuorumValidatorPage from './components/QuorumValidatorPage';
import ActivityLogPage from './components/ActivityLogPage';
import FederationOverviewPage from './components/FederationOverviewPage';
import ProposalCreationPage from './components/ProposalCreationPage';

// Import demo mode components
import DemoModeToggle from './demo/DemoModeToggle';
import { useDemoMode } from './demo/DemoModeContext';

// Drawer width
const drawerWidth = 280;

// Menu items configuration
const menuItems = [
  { text: 'Dashboard', icon: <DashboardIcon />, path: '/' },
  { text: 'DAG Viewer', icon: <DagIcon />, path: '/dag-view' },
  { text: 'Policy Inspector', icon: <PolicyIcon />, path: '/policy-inspector' },
  { text: 'Quorum Validator', icon: <QuorumIcon />, path: '/quorum-validator' },
  { text: 'Activity Log', icon: <ActivityIcon />, path: '/activity-log' },
  { text: 'Federation Overview', icon: <FederationIcon />, path: '/federation-overview' },
  { text: 'Create Proposal', icon: <AddIcon />, path: '/create-proposal' },
];

const App: React.FC = () => {
  const [mobileOpen, setMobileOpen] = useState(false);
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const location = useLocation();
  const navigate = useNavigate();
  
  // Get demo mode from context
  const { isDemoMode, toggleDemoMode } = useDemoMode();

  const handleDrawerToggle = () => {
    setMobileOpen(!mobileOpen);
  };

  const getPageTitle = () => {
    const item = menuItems.find(item => item.path === location.pathname);
    return item ? item.text : 'ICN Observability';
  };

  const drawer = (
    <div>
      <Toolbar>
        <Typography variant="h6" noWrap component="div">
          ICN Observability
        </Typography>
      </Toolbar>
      <List>
        {menuItems.map((item) => (
          <ListItem key={item.text} disablePadding>
            <ListItemButton 
              selected={location.pathname === item.path}
              onClick={() => {
                navigate(item.path);
                if (isMobile) setMobileOpen(false);
              }}
            >
              <ListItemIcon>
                {item.icon}
              </ListItemIcon>
              <ListItemText primary={item.text} />
            </ListItemButton>
          </ListItem>
        ))}
      </List>
    </div>
  );

  return (
    <Box sx={{ display: 'flex', height: '100vh' }}>
      <CssBaseline />
      <AppBar
        position="fixed"
        sx={{
          width: { md: `calc(100% - ${drawerWidth}px)` },
          ml: { md: `${drawerWidth}px` },
        }}
      >
        <Toolbar sx={{ display: 'flex', justifyContent: 'space-between' }}>
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <IconButton
              color="inherit"
              aria-label="open drawer"
              edge="start"
              onClick={handleDrawerToggle}
              sx={{ mr: 2, display: { md: 'none' } }}
            >
              <MenuIcon />
            </IconButton>
            <Typography variant="h6" noWrap component="div">
              {getPageTitle()}
            </Typography>
          </Box>
          
          {/* Demo Mode Toggle */}
          <DemoModeToggle 
            isDemoMode={isDemoMode} 
            onToggle={toggleDemoMode}
          />
        </Toolbar>
      </AppBar>
      <Box
        component="nav"
        sx={{ width: { md: drawerWidth }, flexShrink: { md: 0 } }}
      >
        <Drawer
          variant={isMobile ? 'temporary' : 'permanent'}
          open={isMobile ? mobileOpen : true}
          onClose={handleDrawerToggle}
          sx={{
            '& .MuiDrawer-paper': {
              boxSizing: 'border-box',
              width: drawerWidth,
            },
          }}
        >
          {drawer}
        </Drawer>
      </Box>
      <Box
        component="main"
        sx={{
          flexGrow: 1,
          p: 3,
          width: { md: `calc(100% - ${drawerWidth}px)` },
          height: '100%',
          overflow: 'auto',
          mt: '64px' // AppBar height
        }}
      >
        <Routes>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/dag-view" element={<DagViewPage />} />
          <Route path="/policy-inspector" element={<PolicyInspectorPage />} />
          <Route path="/quorum-validator" element={<QuorumValidatorPage />} />
          <Route path="/activity-log" element={<ActivityLogPage />} />
          <Route path="/federation-overview" element={<FederationOverviewPage />} />
          <Route path="/create-proposal" element={<ProposalCreationPage />} />
        </Routes>
      </Box>
    </Box>
  );
};

export default App; 