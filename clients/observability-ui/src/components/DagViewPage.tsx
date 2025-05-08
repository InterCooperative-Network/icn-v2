import React, { useState, useEffect, useCallback } from 'react';
import { 
  Box, 
  Button, 
  Card, 
  Chip, 
  Container, 
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
  Alert
} from '@mui/material';
import ForceGraph2D from 'react-force-graph-2d';
import observabilityApi, { DagNodeInfo } from '../api/observabilityApi';

// Map node types to colors for visualization
const nodeTypeColors: Record<string, string> = {
  Raw: '#64b5f6', // blue
  Json: '#81c784', // green
  Reference: '#ffb74d', // orange
  TrustBundle: '#ba68c8', // purple
  ExecutionReceipt: '#e57373', // red
  Proposal: '#4db6ac', // teal
  Vote: '#fff176', // yellow
  Policy: '#9575cd', // deep purple
  PolicyUpdate: '#4dd0e1', // cyan
};

// Get color for node based on type
const getNodeColor = (nodeType: string): string => {
  return nodeTypeColors[nodeType] || '#90a4ae'; // default grey
};

const DagViewPage: React.FC = () => {
  const [scopeType, setScopeType] = useState<string>('federation');
  const [scopeId, setScopeId] = useState<string>('');
  const [limit, setLimit] = useState<number>(50);
  const [dagNodes, setDagNodes] = useState<DagNodeInfo[]>([]);
  const [graphData, setGraphData] = useState<{ nodes: any[], links: any[] }>({ nodes: [], links: [] });
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<DagNodeInfo | null>(null);

  // Function to fetch DAG nodes
  const fetchDagNodes = useCallback(async () => {
    if (!scopeId) {
      setError('Please enter a scope ID');
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const nodes = await observabilityApi.getDagView(scopeType, scopeId, limit);
      setDagNodes(nodes);
      
      // Transform data for the force graph
      const graphNodes = nodes.map(node => ({
        id: node.cid,
        node: node,
        color: getNodeColor(node.payload_type),
      }));
      
      const graphLinks: any[] = [];
      nodes.forEach(node => {
        node.parent_cids.forEach(parentCid => {
          graphLinks.push({
            source: node.cid,
            target: parentCid,
          });
        });
      });
      
      setGraphData({ nodes: graphNodes, links: graphLinks });
    } catch (err) {
      console.error('Error fetching DAG nodes:', err);
      setError('Failed to fetch DAG nodes. Please try again.');
    } finally {
      setLoading(false);
    }
  }, [scopeType, scopeId, limit]);

  const handleScopeTypeChange = (event: SelectChangeEvent) => {
    setScopeType(event.target.value);
  };

  const handleScopeIdChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setScopeId(event.target.value);
  };

  const handleLimitChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setLimit(parseInt(event.target.value, 10) || 50);
  };

  const handleNodeClick = (node: any) => {
    setSelectedNode(node.node);
  };

  // Format timestamp
  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  return (
    <Box>
      <Paper sx={{ p: 3, mb: 3, borderRadius: 2 }}>
        <Typography variant="h5" gutterBottom>
          DAG Viewer
        </Typography>
        <Typography variant="body2" color="text.secondary" paragraph>
          Explore the Directed Acyclic Graph (DAG) threads with detailed node metadata.
          Visualize parent-child relationships and inspect payload data.
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
              onClick={fetchDagNodes}
              disabled={loading || !scopeId}
              sx={{ height: '56px', mt: { xs: 0, sm: '16px' } }}
            >
              {loading ? <CircularProgress size={24} /> : 'View DAG'}
            </Button>
          </Grid>
        </Grid>
      </Paper>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      )}

      {dagNodes.length > 0 ? (
        <Grid container spacing={3}>
          <Grid item xs={12} lg={8}>
            <Paper sx={{ p: 2, height: 600, borderRadius: 2 }}>
              <Box sx={{ height: '100%', width: '100%' }}>
                <ForceGraph2D
                  graphData={graphData}
                  nodeLabel={(node: any) => `${node.node.payload_type}: ${node.node.payload_preview}`}
                  nodeColor={(node: any) => node.color}
                  onNodeClick={handleNodeClick}
                  linkDirectionalArrowLength={3}
                  linkDirectionalArrowRelPos={1}
                  cooldownTicks={100}
                  linkWidth={1.5}
                />
              </Box>
            </Paper>
          </Grid>
          <Grid item xs={12} lg={4}>
            <Paper sx={{ p: 2, height: 600, borderRadius: 2, overflow: 'auto' }}>
              {selectedNode ? (
                <Box>
                  <Typography variant="h6">Node Details</Typography>
                  <Divider sx={{ my: 2 }} />
                  
                  <Typography variant="subtitle2" color="text.secondary">CID</Typography>
                  <Typography variant="body2" paragraph component="div" sx={{ overflowWrap: 'anywhere' }}>
                    <Chip label={selectedNode.cid} color="primary" size="small" />
                  </Typography>
                  
                  <Typography variant="subtitle2" color="text.secondary">Timestamp</Typography>
                  <Typography variant="body2" paragraph>{formatTimestamp(selectedNode.timestamp)}</Typography>
                  
                  <Typography variant="subtitle2" color="text.secondary">Signer</Typography>
                  <Typography variant="body2" paragraph component="div" sx={{ overflowWrap: 'anywhere' }}>
                    <Chip label={selectedNode.signer_did} color="secondary" size="small" />
                  </Typography>
                  
                  <Typography variant="subtitle2" color="text.secondary">Type</Typography>
                  <Typography variant="body2" paragraph>
                    <Chip 
                      label={selectedNode.payload_type} 
                      size="small" 
                      style={{ 
                        backgroundColor: getNodeColor(selectedNode.payload_type),
                        color: '#fff'
                      }} 
                    />
                  </Typography>
                  
                  <Typography variant="subtitle2" color="text.secondary">Payload</Typography>
                  <Typography variant="body2" paragraph sx={{ wordBreak: 'break-word' }}>
                    {selectedNode.payload_preview}
                  </Typography>
                  
                  <Typography variant="subtitle2" color="text.secondary">Parents</Typography>
                  {selectedNode.parent_cids.length > 0 ? (
                    selectedNode.parent_cids.map((parentCid, index) => (
                      <Chip 
                        key={index}
                        label={parentCid}
                        size="small"
                        color="default"
                        sx={{ mr: 1, mb: 1 }}
                      />
                    ))
                  ) : (
                    <Typography variant="body2">Genesis Node (No Parents)</Typography>
                  )}
                </Box>
              ) : (
                <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}>
                  <Typography variant="body1" color="text.secondary">
                    Select a node in the graph to view details
                  </Typography>
                </Box>
              )}
            </Paper>
          </Grid>
          
          <Grid item xs={12}>
            <Paper sx={{ p: 2, borderRadius: 2, mt: 2 }}>
              <Typography variant="h6" gutterBottom>DAG Nodes List</Typography>
              <Box sx={{ height: 400, overflow: 'auto' }}>
                {dagNodes.map((node, index) => (
                  <Card 
                    key={index} 
                    sx={{ 
                      mb: 2, 
                      p: 2, 
                      cursor: 'pointer',
                      backgroundColor: selectedNode?.cid === node.cid ? '#f5f5f5' : 'white',
                      '&:hover': { backgroundColor: '#f5f5f5' }
                    }}
                    onClick={() => setSelectedNode(node)}
                  >
                    <Grid container spacing={2}>
                      <Grid item xs={12} sm={6}>
                        <Typography variant="subtitle2" color="text.secondary">CID</Typography>
                        <Typography variant="body2" sx={{ wordBreak: 'break-word' }}>{node.cid}</Typography>
                      </Grid>
                      <Grid item xs={12} sm={6}>
                        <Typography variant="subtitle2" color="text.secondary">Timestamp</Typography>
                        <Typography variant="body2">{formatTimestamp(node.timestamp)}</Typography>
                      </Grid>
                      <Grid item xs={12} sm={6}>
                        <Typography variant="subtitle2" color="text.secondary">Type</Typography>
                        <Chip 
                          label={node.payload_type} 
                          size="small" 
                          style={{ 
                            backgroundColor: getNodeColor(node.payload_type),
                            color: '#fff'
                          }} 
                        />
                      </Grid>
                      <Grid item xs={12} sm={6}>
                        <Typography variant="subtitle2" color="text.secondary">Payload</Typography>
                        <Typography variant="body2" sx={{ wordBreak: 'break-word' }}>{node.payload_preview}</Typography>
                      </Grid>
                    </Grid>
                  </Card>
                ))}
              </Box>
            </Paper>
          </Grid>
        </Grid>
      ) : (
        !loading && !error && (
          <Paper sx={{ p: 4, textAlign: 'center', borderRadius: 2 }}>
            <Typography variant="h6" color="text.secondary">
              Enter scope details and click "View DAG" to see the DAG structure
            </Typography>
          </Paper>
        )
      )}
    </Box>
  );
};

export default DagViewPage; 