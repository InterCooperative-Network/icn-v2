import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { 
  Box, 
  Button, 
  Card, 
  Chip, 
  Container, 
  Dialog, 
  DialogActions, 
  DialogContent, 
  DialogTitle, 
  Divider, 
  FormControl, 
  Grid, 
  IconButton, 
  InputLabel, 
  Link, 
  MenuItem, 
  Paper, 
  Select, 
  SelectChangeEvent, 
  Stack, 
  Table, 
  TableBody, 
  TableCell, 
  TableContainer, 
  TableHead, 
  TableRow, 
  TextField, 
  Typography 
} from '@mui/material';
import { 
  FilterList as FilterIcon, 
  Close as CloseIcon, 
  Share as ShareIcon, 
  ContentCopy as CopyIcon, 
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon
} from '@mui/icons-material';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';

// Type definitions
interface SerializedReceipt {
  id: string;
  cid: string;
  federation_did: string;
  module_cid?: string;
  status: string;
  scope: string;
  submitter?: string;
  execution_timestamp: number;
  result_summary?: string;
  source_event_id?: string;
  wallet_stored_at: number;
  json_vc: string;
}

interface FilterState {
  federation_did?: string;
  module_cid?: string;
  scope?: string;
  status?: string;
  submitter_did?: string;
  start_time?: number;
  end_time?: number;
  limit: number;
  offset: number;
}

// Utility functions
const shortenString = (str: string, length = 12) => {
  if (!str || str.length <= length) return str;
  return `${str.substring(0, length / 2)}...${str.substring(str.length - length / 2)}`;
};

const formatDate = (timestamp: number) => {
  return new Date(timestamp * 1000).toLocaleString();
};

const getStatusColor = (status: string) => {
  if (status.toLowerCase().includes('completed')) {
    return 'success';
  } else if (status.toLowerCase().includes('failed')) {
    return 'error';
  } else {
    return 'warning';
  }
};

// Component for filters
const ReceiptFilters = ({ 
  filter, 
  setFilter, 
  onApply, 
  onClear 
}: { 
  filter: FilterState; 
  setFilter: (filter: FilterState) => void; 
  onApply: () => void;
  onClear: () => void;
}) => {
  const handleScopeChange = (event: SelectChangeEvent) => {
    setFilter({ 
      ...filter, 
      scope: event.target.value === 'Any' ? undefined : event.target.value 
    });
  };

  const handleStatusChange = (event: SelectChangeEvent) => {
    setFilter({ 
      ...filter, 
      status: event.target.value === 'Any' ? undefined : event.target.value 
    });
  };

  const handleStartDateChange = (date: Date | null) => {
    if (date) {
      setFilter({
        ...filter,
        start_time: Math.floor(date.getTime() / 1000)
      });
    } else {
      const { start_time, ...rest } = filter;
      setFilter(rest as FilterState);
    }
  };

  const handleEndDateChange = (date: Date | null) => {
    if (date) {
      setFilter({
        ...filter,
        end_time: Math.floor(date.getTime() / 1000)
      });
    } else {
      const { end_time, ...rest } = filter;
      setFilter(rest as FilterState);
    }
  };

  return (
    <Paper sx={{ p: 2, mb: 2 }}>
      <Typography variant="h6" gutterBottom>Filter Receipts</Typography>
      <Grid container spacing={2}>
        <Grid item xs={12} md={6}>
          <TextField
            fullWidth
            label="Federation DID"
            value={filter.federation_did || ''}
            onChange={(e) => setFilter({ ...filter, federation_did: e.target.value || undefined })}
            placeholder="did:example:federation"
            margin="normal"
          />
        </Grid>
        
        <Grid item xs={12} md={6}>
          <TextField
            fullWidth
            label="Module CID"
            value={filter.module_cid || ''}
            onChange={(e) => setFilter({ ...filter, module_cid: e.target.value || undefined })}
            placeholder="bafy..."
            margin="normal"
          />
        </Grid>
        
        <Grid item xs={12} md={6}>
          <FormControl fullWidth margin="normal">
            <InputLabel>Scope</InputLabel>
            <Select
              value={filter.scope || 'Any'}
              label="Scope"
              onChange={handleScopeChange}
            >
              <MenuItem value="Any">Any</MenuItem>
              <MenuItem value="Federation">Federation</MenuItem>
              <MenuItem value="MeshCompute">Mesh Compute</MenuItem>
              <MenuItem value="Cooperative">Cooperative</MenuItem>
              <MenuItem value="Custom">Custom</MenuItem>
            </Select>
          </FormControl>
        </Grid>
        
        <Grid item xs={12} md={6}>
          <FormControl fullWidth margin="normal">
            <InputLabel>Status</InputLabel>
            <Select
              value={filter.status || 'Any'}
              label="Status"
              onChange={handleStatusChange}
            >
              <MenuItem value="Any">Any</MenuItem>
              <MenuItem value="Completed">Completed</MenuItem>
              <MenuItem value="Pending">Pending</MenuItem>
              <MenuItem value="Failed">Failed</MenuItem>
            </Select>
          </FormControl>
        </Grid>
        
        <Grid item xs={12} md={6}>
          <LocalizationProvider dateAdapter={AdapterDateFns}>
            <DatePicker
              label="Start Date"
              value={filter.start_time ? new Date(filter.start_time * 1000) : null}
              onChange={handleStartDateChange}
              slotProps={{ textField: { fullWidth: true, margin: 'normal' } }}
            />
          </LocalizationProvider>
        </Grid>
        
        <Grid item xs={12} md={6}>
          <LocalizationProvider dateAdapter={AdapterDateFns}>
            <DatePicker
              label="End Date"
              value={filter.end_time ? new Date(filter.end_time * 1000) : null}
              onChange={handleEndDateChange}
              slotProps={{ textField: { fullWidth: true, margin: 'normal' } }}
            />
          </LocalizationProvider>
        </Grid>
      </Grid>
      
      <Box sx={{ mt: 2, display: 'flex', gap: 2, justifyContent: 'flex-end' }}>
        <Button variant="outlined" color="error" onClick={onClear}>
          Clear Filters
        </Button>
        <Button variant="contained" onClick={onApply}>
          Apply Filters
        </Button>
      </Box>
    </Paper>
  );
};

// Receipt Detail Dialog
const ReceiptDetailDialog = ({
  receipt,
  open,
  onClose
}: {
  receipt: SerializedReceipt | null;
  open: boolean;
  onClose: () => void;
}) => {
  const [expandJson, setExpandJson] = useState(false);
  
  if (!receipt) return null;
  
  const handleCopyJson = async () => {
    await navigator.clipboard.writeText(receipt.json_vc);
  };
  
  const handleCopyId = async () => {
    await navigator.clipboard.writeText(receipt.id);
  };
  
  const vcJson = JSON.parse(receipt.json_vc);
  
  return (
    <Dialog 
      open={open} 
      onClose={onClose}
      fullWidth
      maxWidth="md"
    >
      <DialogTitle>
        <Box display="flex" justifyContent="space-between" alignItems="center">
          <Typography variant="h6">Execution Receipt Details</Typography>
          <IconButton onClick={onClose} size="small">
            <CloseIcon />
          </IconButton>
        </Box>
      </DialogTitle>
      
      <DialogContent dividers>
        <Grid container spacing={2}>
          <Grid item xs={12}>
            <Box display="flex" alignItems="center" mb={1}>
              <Typography variant="subtitle1" fontWeight="bold" mr={1}>ID:</Typography>
              <Typography variant="body1" sx={{ wordBreak: 'break-all', flex: 1 }}>
                {receipt.id}
              </Typography>
              <IconButton size="small" onClick={handleCopyId}>
                <CopyIcon fontSize="small" />
              </IconButton>
            </Box>
          </Grid>
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">CID:</Typography>
            <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
              {receipt.cid}
            </Typography>
          </Grid>
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">Federation DID:</Typography>
            <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
              {receipt.federation_did}
            </Typography>
          </Grid>
          
          {receipt.module_cid && (
            <Grid item xs={12} md={6}>
              <Typography variant="subtitle1" fontWeight="bold">Module CID:</Typography>
              <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
                {receipt.module_cid}
              </Typography>
            </Grid>
          )}
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">Status:</Typography>
            <Chip 
              label={receipt.status} 
              color={getStatusColor(receipt.status)} 
              size="small"
            />
          </Grid>
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">Scope:</Typography>
            <Typography variant="body2">{receipt.scope}</Typography>
          </Grid>
          
          {receipt.submitter && (
            <Grid item xs={12} md={6}>
              <Typography variant="subtitle1" fontWeight="bold">Submitter:</Typography>
              <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
                {receipt.submitter}
              </Typography>
            </Grid>
          )}
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">Execution Date:</Typography>
            <Typography variant="body2">
              {formatDate(receipt.execution_timestamp)}
            </Typography>
          </Grid>
          
          {receipt.result_summary && (
            <Grid item xs={12}>
              <Typography variant="subtitle1" fontWeight="bold">Result Summary:</Typography>
              <Typography variant="body2">{receipt.result_summary}</Typography>
            </Grid>
          )}
          
          {receipt.source_event_id && (
            <Grid item xs={12} md={6}>
              <Typography variant="subtitle1" fontWeight="bold">Source Event ID:</Typography>
              <Typography variant="body2" sx={{ wordBreak: 'break-all' }}>
                {receipt.source_event_id}
              </Typography>
            </Grid>
          )}
          
          <Grid item xs={12} md={6}>
            <Typography variant="subtitle1" fontWeight="bold">Added to Wallet:</Typography>
            <Typography variant="body2">
              {formatDate(receipt.wallet_stored_at)}
            </Typography>
          </Grid>
          
          <Grid item xs={12}>
            <Divider sx={{ my: 2 }} />
            <Box 
              display="flex" 
              justifyContent="space-between" 
              alignItems="center" 
              onClick={() => setExpandJson(!expandJson)}
              sx={{ cursor: 'pointer', mb: 1 }}
            >
              <Typography variant="subtitle1" fontWeight="bold">
                Verifiable Credential JSON
              </Typography>
              <IconButton size="small">
                {expandJson ? <ExpandLessIcon /> : <ExpandMoreIcon />}
              </IconButton>
            </Box>
            
            {expandJson && (
              <Box position="relative">
                <IconButton 
                  size="small" 
                  sx={{ position: 'absolute', top: 8, right: 8 }}
                  onClick={handleCopyJson}
                >
                  <CopyIcon fontSize="small" />
                </IconButton>
                <Paper 
                  elevation={0} 
                  sx={{ 
                    p: 2, 
                    backgroundColor: '#f5f5f5', 
                    maxHeight: '400px', 
                    overflow: 'auto',
                    fontFamily: '"Roboto Mono", monospace',
                    fontSize: '0.875rem'
                  }}
                >
                  <pre>{JSON.stringify(vcJson, null, 2)}</pre>
                </Paper>
              </Box>
            )}
          </Grid>
        </Grid>
      </DialogContent>
      
      <DialogActions>
        <Button startIcon={<ShareIcon />} variant="outlined" onClick={() => {}}>
          Share Receipt
        </Button>
        <Button variant="contained" onClick={onClose}>
          Close
        </Button>
      </DialogActions>
    </Dialog>
  );
};

// Main ReceiptsTab component
const ReceiptsTab: React.FC = () => {
  const [receipts, setReceipts] = useState<SerializedReceipt[]>([]);
  const [loading, setLoading] = useState(true);
  const [showFilters, setShowFilters] = useState(false);
  const [filter, setFilter] = useState<FilterState>({ limit: 25, offset: 0 });
  const [selectedReceipt, setSelectedReceipt] = useState<SerializedReceipt | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  
  const loadReceipts = async () => {
    setLoading(true);
    try {
      const results = await invoke<SerializedReceipt[]>("list_receipts", {
        federationDid: filter.federation_did,
        moduleCid: filter.module_cid,
        scope: filter.scope,
        status: filter.status,
        submitterDid: filter.submitter_did,
        startTime: filter.start_time,
        endTime: filter.end_time,
        limit: filter.limit,
        offset: filter.offset,
      });
      setReceipts(results);
    } catch (error) {
      console.error("Failed to load receipts:", error);
    } finally {
      setLoading(false);
    }
  };
  
  useEffect(() => {
    loadReceipts();
  }, []);
  
  const handleViewReceipt = (receipt: SerializedReceipt) => {
    setSelectedReceipt(receipt);
    setDetailOpen(true);
  };
  
  const handleApplyFilter = () => {
    loadReceipts();
  };
  
  const handleClearFilter = () => {
    setFilter({ limit: 25, offset: 0 });
    loadReceipts();
  };
  
  const handleLoadMore = () => {
    setFilter(prev => ({
      ...prev,
      offset: prev.offset + prev.limit
    }));
    loadReceipts();
  };
  
  return (
    <Container maxWidth="lg" sx={{ py: 4 }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 3 }}>
        <Typography variant="h4" component="h1">
          Execution Receipts
        </Typography>
        <Button 
          variant="outlined" 
          startIcon={<FilterIcon />}
          onClick={() => setShowFilters(!showFilters)}
        >
          {showFilters ? 'Hide Filters' : 'Show Filters'}
        </Button>
      </Box>
      
      {showFilters && (
        <ReceiptFilters 
          filter={filter}
          setFilter={setFilter}
          onApply={handleApplyFilter}
          onClear={handleClearFilter}
        />
      )}
      
      {loading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
          <Typography>Loading receipts...</Typography>
        </Box>
      ) : receipts.length === 0 ? (
        <Card sx={{ p: 4, textAlign: 'center' }}>
          <Typography variant="h6" color="text.secondary" gutterBottom>
            No Receipts Found
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Try adjusting your filters or check back after executing some modules.
          </Typography>
        </Card>
      ) : (
        <>
          <TableContainer component={Paper}>
            <Table sx={{ minWidth: 650 }}>
              <TableHead>
                <TableRow>
                  <TableCell>ID</TableCell>
                  <TableCell>Module</TableCell>
                  <TableCell>Federation</TableCell>
                  <TableCell>Scope</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Date</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {receipts.map((receipt) => (
                  <TableRow
                    key={receipt.id}
                    sx={{ '&:last-child td, &:last-child th': { border: 0 } }}
                  >
                    <TableCell component="th" scope="row">
                      {shortenString(receipt.id)}
                    </TableCell>
                    <TableCell>{receipt.module_cid ? shortenString(receipt.module_cid) : 'N/A'}</TableCell>
                    <TableCell>{shortenString(receipt.federation_did)}</TableCell>
                    <TableCell>{receipt.scope}</TableCell>
                    <TableCell>
                      <Chip 
                        label={receipt.status} 
                        color={getStatusColor(receipt.status)} 
                        size="small"
                      />
                    </TableCell>
                    <TableCell>{formatDate(receipt.execution_timestamp)}</TableCell>
                    <TableCell align="right">
                      <Button
                        size="small"
                        onClick={() => handleViewReceipt(receipt)}
                      >
                        View
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          
          {receipts.length >= filter.limit && (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
              <Button variant="outlined" onClick={handleLoadMore}>
                Load More
              </Button>
            </Box>
          )}
        </>
      )}
      
      <ReceiptDetailDialog
        receipt={selectedReceipt}
        open={detailOpen}
        onClose={() => setDetailOpen(false)}
      />
    </Container>
  );
};

export default ReceiptsTab; 