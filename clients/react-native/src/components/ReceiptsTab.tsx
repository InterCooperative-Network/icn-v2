import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  FlatList,
  StyleSheet,
  TouchableOpacity,
  TextInput,
  ScrollView,
  Modal,
  Share,
} from 'react-native';
import { useNavigation } from '@react-navigation/native';
import DateTimePicker from '@react-native-community/datetimepicker';
import { Picker } from '@react-native-picker/picker';
import { IcnWallet } from '../native/icn_wallet';

// Type definitions based on the Rust FFI interface
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

// Component for a single receipt card
const ReceiptCard = ({ receipt, onPress }: { receipt: SerializedReceipt; onPress: () => void }) => {
  return (
    <TouchableOpacity style={styles.card} onPress={onPress}>
      <View style={styles.cardHeader}>
        <Text style={styles.cardTitle}>Receipt {shortenString(receipt.id)}</Text>
        <Text style={styles.statusBadge(receipt.status)}>
          {receipt.status}
        </Text>
      </View>
      
      <View style={styles.cardContent}>
        <Text style={styles.cardLabel}>Module:</Text>
        <Text style={styles.cardValue}>{receipt.module_cid ? shortenString(receipt.module_cid) : 'N/A'}</Text>
        
        <Text style={styles.cardLabel}>Federation:</Text>
        <Text style={styles.cardValue}>{shortenString(receipt.federation_did)}</Text>
        
        <Text style={styles.cardLabel}>Scope:</Text>
        <Text style={styles.cardValue}>{receipt.scope}</Text>
        
        <Text style={styles.cardLabel}>Date:</Text>
        <Text style={styles.cardValue}>{formatDate(receipt.execution_timestamp)}</Text>
      </View>
      
      <Text style={styles.viewDetails}>View Details →</Text>
    </TouchableOpacity>
  );
};

// Component for the receipt detail view
const ReceiptDetails = ({ 
  receipt, 
  visible, 
  onClose 
}: { 
  receipt: SerializedReceipt | null; 
  visible: boolean; 
  onClose: () => void;
}) => {
  if (!receipt) return null;
  
  const handleShare = async () => {
    try {
      await Share.share({
        message: receipt.json_vc,
        title: `Execution Receipt: ${receipt.id}`,
      });
    } catch (error) {
      console.error('Error sharing receipt:', error);
    }
  };

  return (
    <Modal
      animationType="slide"
      transparent={false}
      visible={visible}
      onRequestClose={onClose}
    >
      <View style={styles.detailContainer}>
        <View style={styles.detailHeader}>
          <Text style={styles.detailTitle}>Receipt Details</Text>
          <TouchableOpacity onPress={onClose}>
            <Text style={styles.closeButton}>Close</Text>
          </TouchableOpacity>
        </View>
        
        <ScrollView style={styles.detailScroll}>
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>ID:</Text>
            <Text style={styles.detailValue}>{receipt.id}</Text>
          </View>
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>CID:</Text>
            <Text style={styles.detailValue}>{receipt.cid}</Text>
          </View>
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>Federation:</Text>
            <Text style={styles.detailValue}>{receipt.federation_did}</Text>
          </View>
          
          {receipt.module_cid && (
            <View style={styles.detailItem}>
              <Text style={styles.detailLabel}>Module CID:</Text>
              <Text style={styles.detailValue}>{receipt.module_cid}</Text>
            </View>
          )}
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>Status:</Text>
            <Text style={[styles.detailValue, styles.statusText(receipt.status)]}>
              {receipt.status}
            </Text>
          </View>
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>Scope:</Text>
            <Text style={styles.detailValue}>{receipt.scope}</Text>
          </View>
          
          {receipt.submitter && (
            <View style={styles.detailItem}>
              <Text style={styles.detailLabel}>Submitter:</Text>
              <Text style={styles.detailValue}>{receipt.submitter}</Text>
            </View>
          )}
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>Execution Date:</Text>
            <Text style={styles.detailValue}>{formatDate(receipt.execution_timestamp)}</Text>
          </View>
          
          {receipt.result_summary && (
            <View style={styles.detailItem}>
              <Text style={styles.detailLabel}>Result:</Text>
              <Text style={styles.detailValue}>{receipt.result_summary}</Text>
            </View>
          )}
          
          {receipt.source_event_id && (
            <View style={styles.detailItem}>
              <Text style={styles.detailLabel}>Source Event:</Text>
              <Text style={styles.detailValue}>{receipt.source_event_id}</Text>
            </View>
          )}
          
          <View style={styles.detailItem}>
            <Text style={styles.detailLabel}>Added to Wallet:</Text>
            <Text style={styles.detailValue}>{formatDate(receipt.wallet_stored_at)}</Text>
          </View>
          
          <View style={styles.jsonSection}>
            <Text style={styles.jsonTitle}>Verifiable Credential JSON:</Text>
            <ScrollView style={styles.jsonContainer} horizontal={true}>
              <Text style={styles.jsonText}>
                {JSON.stringify(JSON.parse(receipt.json_vc), null, 2)}
              </Text>
            </ScrollView>
          </View>
          
          <TouchableOpacity 
            style={styles.shareButton} 
            onPress={handleShare}
          >
            <Text style={styles.shareButtonText}>Share Receipt</Text>
          </TouchableOpacity>
          
          <View style={{ height: 40 }} />
        </ScrollView>
      </View>
    </Modal>
  );
};

// Filter panel component
const FilterPanel = ({ 
  filter, 
  setFilter, 
  applyFilter 
}: { 
  filter: FilterState; 
  setFilter: (filter: FilterState) => void; 
  applyFilter: () => void;
}) => {
  const [showStartDate, setShowStartDate] = useState(false);
  const [showEndDate, setShowEndDate] = useState(false);
  
  const statusOptions = ['Any', 'Completed', 'Pending', 'Failed'];
  const scopeOptions = ['Any', 'Federation', 'MeshCompute', 'Cooperative', 'Custom'];
  
  return (
    <View style={styles.filterContainer}>
      <Text style={styles.filterTitle}>Filter Receipts</Text>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>Federation DID:</Text>
        <TextInput
          style={styles.filterInput}
          placeholder="Enter DID"
          value={filter.federation_did}
          onChangeText={(text) => setFilter({ ...filter, federation_did: text })}
        />
      </View>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>Module CID:</Text>
        <TextInput
          style={styles.filterInput}
          placeholder="Enter CID"
          value={filter.module_cid}
          onChangeText={(text) => setFilter({ ...filter, module_cid: text })}
        />
      </View>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>Status:</Text>
        <View style={styles.pickerContainer}>
          <Picker
            selectedValue={filter.status || 'Any'}
            style={styles.picker}
            onValueChange={(value) => 
              setFilter({ ...filter, status: value === 'Any' ? undefined : value })
            }
          >
            {statusOptions.map((option) => (
              <Picker.Item key={option} label={option} value={option} />
            ))}
          </Picker>
        </View>
      </View>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>Scope:</Text>
        <View style={styles.pickerContainer}>
          <Picker
            selectedValue={filter.scope || 'Any'}
            style={styles.picker}
            onValueChange={(value) => 
              setFilter({ ...filter, scope: value === 'Any' ? undefined : value })
            }
          >
            {scopeOptions.map((option) => (
              <Picker.Item key={option} label={option} value={option} />
            ))}
          </Picker>
        </View>
      </View>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>Start Date:</Text>
        <TouchableOpacity 
          style={styles.dateButton}
          onPress={() => setShowStartDate(true)}
        >
          <Text>
            {filter.start_time 
              ? formatDate(filter.start_time) 
              : 'Select Start Date'}
          </Text>
        </TouchableOpacity>
        
        {showStartDate && (
          <DateTimePicker
            value={new Date(filter.start_time ? filter.start_time * 1000 : Date.now())}
            mode="date"
            onChange={(event, date) => {
              setShowStartDate(false);
              if (date) {
                setFilter({ 
                  ...filter, 
                  start_time: Math.floor(date.getTime() / 1000) 
                });
              }
            }}
          />
        )}
      </View>
      
      <View style={styles.filterRow}>
        <Text style={styles.filterLabel}>End Date:</Text>
        <TouchableOpacity 
          style={styles.dateButton}
          onPress={() => setShowEndDate(true)}
        >
          <Text>
            {filter.end_time 
              ? formatDate(filter.end_time) 
              : 'Select End Date'}
          </Text>
        </TouchableOpacity>
        
        {showEndDate && (
          <DateTimePicker
            value={new Date(filter.end_time ? filter.end_time * 1000 : Date.now())}
            mode="date"
            onChange={(event, date) => {
              setShowEndDate(false);
              if (date) {
                setFilter({ 
                  ...filter, 
                  end_time: Math.floor(date.getTime() / 1000) 
                });
              }
            }}
          />
        )}
      </View>
      
      <TouchableOpacity 
        style={styles.applyButton}
        onPress={applyFilter}
      >
        <Text style={styles.applyButtonText}>Apply Filters</Text>
      </TouchableOpacity>
      
      <TouchableOpacity 
        style={styles.clearButton}
        onPress={() => {
          setFilter({ 
            limit: 20, 
            offset: 0 
          });
          applyFilter();
        }}
      >
        <Text style={styles.clearButtonText}>Clear All Filters</Text>
      </TouchableOpacity>
    </View>
  );
};

// Main Receipts Tab Component
const ReceiptsTab = () => {
  const [receipts, setReceipts] = useState<SerializedReceipt[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<FilterState>({ limit: 20, offset: 0 });
  const [showFilter, setShowFilter] = useState(false);
  const [selectedReceipt, setSelectedReceipt] = useState<SerializedReceipt | null>(null);
  const [showDetail, setShowDetail] = useState(false);
  
  const loadReceipts = async () => {
    setLoading(true);
    try {
      const results = await IcnWallet.listReceipts(
        filter.federation_did,
        filter.module_cid,
        filter.scope,
        filter.status,
        filter.submitter_did,
        filter.start_time,
        filter.end_time,
        filter.limit,
        filter.offset
      );
      setReceipts(results);
    } catch (error) {
      console.error('Error loading receipts:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadReceipts();
  }, []);
  
  const handleViewReceipt = (receipt: SerializedReceipt) => {
    setSelectedReceipt(receipt);
    setShowDetail(true);
  };
  
  const handleApplyFilter = () => {
    setShowFilter(false);
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
    <View style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Execution Receipts</Text>
        <TouchableOpacity 
          style={styles.filterButton}
          onPress={() => setShowFilter(!showFilter)}
        >
          <Text style={styles.filterButtonText}>
            {showFilter ? 'Hide Filters' : 'Show Filters'}
          </Text>
        </TouchableOpacity>
      </View>
      
      {showFilter && (
        <FilterPanel 
          filter={filter} 
          setFilter={setFilter} 
          applyFilter={handleApplyFilter} 
        />
      )}
      
      {loading ? (
        <View style={styles.loadingContainer}>
          <Text>Loading receipts...</Text>
        </View>
      ) : receipts.length === 0 ? (
        <View style={styles.emptyContainer}>
          <Text style={styles.emptyText}>No receipts found</Text>
          <Text style={styles.emptySubtext}>
            Try adjusting your filters or check back after executing some modules
          </Text>
        </View>
      ) : (
        <FlatList
          data={receipts}
          keyExtractor={item => item.id}
          renderItem={({ item }) => (
            <ReceiptCard 
              receipt={item} 
              onPress={() => handleViewReceipt(item)} 
            />
          )}
          contentContainerStyle={styles.listContent}
          ListFooterComponent={
            receipts.length >= filter.limit ? (
              <TouchableOpacity 
                style={styles.loadMoreButton}
                onPress={handleLoadMore}
              >
                <Text style={styles.loadMoreText}>Load More</Text>
              </TouchableOpacity>
            ) : null
          }
        />
      )}
      
      <ReceiptDetails
        receipt={selectedReceipt}
        visible={showDetail}
        onClose={() => setShowDetail(false)}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#F5F7FA',
  },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: 16,
    backgroundColor: '#FFFFFF',
    borderBottomWidth: 1,
    borderBottomColor: '#E5E9F2',
  },
  title: {
    fontSize: 20,
    fontWeight: '600',
    color: '#2E3A59',
  },
  filterButton: {
    backgroundColor: '#4D7CFE',
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 6,
  },
  filterButtonText: {
    color: '#FFFFFF',
    fontWeight: '500',
  },
  loadingContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  emptyContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 20,
  },
  emptyText: {
    fontSize: 18,
    fontWeight: '600',
    color: '#2E3A59',
    marginBottom: 12,
  },
  emptySubtext: {
    fontSize: 14,
    color: '#8492A6',
    textAlign: 'center',
  },
  listContent: {
    padding: 16,
  },
  card: {
    backgroundColor: '#FFFFFF',
    borderRadius: 8,
    padding: 16,
    marginBottom: 12,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 4,
    elevation: 2,
  },
  cardHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 12,
  },
  cardTitle: {
    fontSize: 16,
    fontWeight: '600',
    color: '#2E3A59',
  },
  statusBadge: (status: string) => ({
    color: status.toLowerCase().includes('completed') ? '#00C48C' : 
           status.toLowerCase().includes('failed') ? '#FF647C' : '#FFA26B',
    fontSize: 14,
    fontWeight: '500',
    padding: 4,
    borderRadius: 4,
    overflow: 'hidden',
  }),
  cardContent: {
    marginBottom: 12,
  },
  cardLabel: {
    fontSize: 14,
    color: '#8492A6',
    marginBottom: 2,
  },
  cardValue: {
    fontSize: 14,
    color: '#2E3A59',
    marginBottom: 8,
  },
  viewDetails: {
    color: '#4D7CFE',
    fontSize: 14,
    fontWeight: '500',
    alignSelf: 'flex-end',
  },
  loadMoreButton: {
    backgroundColor: '#FFFFFF',
    borderWidth: 1,
    borderColor: '#4D7CFE',
    borderRadius: 6,
    padding: 12,
    alignItems: 'center',
    marginTop: 16,
  },
  loadMoreText: {
    color: '#4D7CFE',
    fontWeight: '500',
  },
  filterContainer: {
    backgroundColor: '#FFFFFF',
    padding: 16,
    borderBottomWidth: 1,
    borderBottomColor: '#E5E9F2',
  },
  filterTitle: {
    fontSize: 16,
    fontWeight: '600',
    color: '#2E3A59',
    marginBottom: 16,
  },
  filterRow: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 12,
  },
  filterLabel: {
    width: 120,
    fontSize: 14,
    color: '#2E3A59',
  },
  filterInput: {
    flex: 1,
    height: 40,
    borderWidth: 1,
    borderColor: '#E5E9F2',
    borderRadius: 6,
    paddingHorizontal: 12,
    backgroundColor: '#FFFFFF',
  },
  pickerContainer: {
    flex: 1,
    height: 40,
    borderWidth: 1,
    borderColor: '#E5E9F2',
    borderRadius: 6,
    overflow: 'hidden',
    backgroundColor: '#FFFFFF',
  },
  picker: {
    height: 40,
    width: '100%',
  },
  dateButton: {
    flex: 1,
    height: 40,
    borderWidth: 1,
    borderColor: '#E5E9F2',
    borderRadius: 6,
    paddingHorizontal: 12,
    justifyContent: 'center',
    backgroundColor: '#FFFFFF',
  },
  applyButton: {
    backgroundColor: '#4D7CFE',
    borderRadius: 6,
    padding: 12,
    alignItems: 'center',
    marginTop: 16,
  },
  applyButtonText: {
    color: '#FFFFFF',
    fontWeight: '500',
  },
  clearButton: {
    backgroundColor: '#FFFFFF',
    borderWidth: 1,
    borderColor: '#FF647C',
    borderRadius: 6,
    padding: 12,
    alignItems: 'center',
    marginTop: 8,
  },
  clearButtonText: {
    color: '#FF647C',
    fontWeight: '500',
  },
  detailContainer: {
    flex: 1,
    backgroundColor: '#F5F7FA',
  },
  detailHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: 16,
    backgroundColor: '#FFFFFF',
    borderBottomWidth: 1,
    borderBottomColor: '#E5E9F2',
  },
  detailTitle: {
    fontSize: 20,
    fontWeight: '600',
    color: '#2E3A59',
  },
  closeButton: {
    color: '#4D7CFE',
    fontSize: 16,
    fontWeight: '500',
  },
  detailScroll: {
    padding: 16,
  },
  detailItem: {
    marginBottom: 16,
  },
  detailLabel: {
    fontSize: 14,
    color: '#8492A6',
    marginBottom: 4,
  },
  detailValue: {
    fontSize: 16,
    color: '#2E3A59',
  },
  statusText: (status: string) => ({
    color: status.toLowerCase().includes('completed') ? '#00C48C' : 
           status.toLowerCase().includes('failed') ? '#FF647C' : '#FFA26B',
    fontWeight: '500',
  }),
  jsonSection: {
    marginTop: 16,
    marginBottom: 24,
  },
  jsonTitle: {
    fontSize: 16,
    fontWeight: '600',
    color: '#2E3A59',
    marginBottom: 8,
  },
  jsonContainer: {
    backgroundColor: '#F0F2F5',
    padding: 12,
    borderRadius: 6,
    maxHeight: 300,
  },
  jsonText: {
    fontFamily: 'monospace',
    fontSize: 12,
  },
  shareButton: {
    backgroundColor: '#4D7CFE',
    borderRadius: 6,
    padding: 12,
    alignItems: 'center',
  },
  shareButtonText: {
    color: '#FFFFFF',
    fontWeight: '500',
  },
});

export default ReceiptsTab; 