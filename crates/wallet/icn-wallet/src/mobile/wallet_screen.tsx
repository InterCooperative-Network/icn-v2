import React, { useState } from 'react';
import { View, Text, StyleSheet, TouchableOpacity, ScrollView, Share, Alert } from 'react-native';
import { VerificationCard } from './verification_card';
import { CredentialImport } from './credential_import';
import { VerificationReport, generateCredentialShareLink } from './bindings/verification';

export const WalletScreen: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'credentials' | 'import'>('credentials');
  const [credentials, setCredentials] = useState<{report: VerificationReport, rawJson: string}[]>([]);

  // Handle successful credential import
  const handleImportSuccess = (report: VerificationReport, rawJson: string) => {
    setCredentials([{ report, rawJson }, ...credentials]);
    setActiveTab('credentials');
  };

  // Delete a credential from the list
  const handleDeleteCredential = (index: number) => {
    Alert.alert(
      'Delete Credential',
      'Are you sure you want to delete this credential from your wallet?',
      [
        { text: 'Cancel', style: 'cancel' },
        { 
          text: 'Delete', 
          style: 'destructive',
          onPress: () => {
            const newCredentials = [...credentials];
            newCredentials.splice(index, 1);
            setCredentials(newCredentials);
          }
        }
      ]
    );
  };

  // Share a credential with someone else
  const handleShareCredential = async (rawJson: string) => {
    try {
      const shareLink = generateCredentialShareLink(rawJson);
      
      await Share.share({
        message: 'Check out this ICN Dispatch Credential:',
        url: shareLink,
      });
    } catch (error) {
      Alert.alert('Error', 'Failed to share credential');
    }
  };

  // View issuer details (in a real app, this would show more info)
  const handleViewIssuer = (issuerDid: string) => {
    Alert.alert(
      'Issuer Details',
      `DID: ${issuerDid}\n\nThis would show more information about the issuer in a full implementation.`
    );
  };

  // View policy details (in a real app, this would show more info)
  const handleViewPolicy = (policyVersion: string) => {
    Alert.alert(
      'Trust Policy Details',
      `Policy ID: ${policyVersion}\n\nThis would show more information about the trust policy in a full implementation.`
    );
  };

  return (
    <View style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>ICN Wallet</Text>
      </View>

      <View style={styles.tabBar}>
        <TouchableOpacity
          style={[styles.tab, activeTab === 'credentials' && styles.activeTab]}
          onPress={() => setActiveTab('credentials')}
        >
          <Text style={[styles.tabText, activeTab === 'credentials' && styles.activeTabText]}>
            My Credentials
          </Text>
        </TouchableOpacity>
        
        <TouchableOpacity
          style={[styles.tab, activeTab === 'import' && styles.activeTab]}
          onPress={() => setActiveTab('import')}
        >
          <Text style={[styles.tabText, activeTab === 'import' && styles.activeTabText]}>
            Import
          </Text>
        </TouchableOpacity>
      </View>

      {activeTab === 'credentials' ? (
        credentials.length > 0 ? (
          <ScrollView style={styles.contentContainer}>
            {credentials.map((cred, index) => (
              <View key={index} style={styles.credentialContainer}>
                <VerificationCard
                  report={cred.report}
                  onViewIssuer={() => handleViewIssuer(cred.report.issuer_did)}
                  onViewPolicy={() => handleViewPolicy(cred.report.policy_version)}
                  onViewDetails={() => {
                    Alert.alert(
                      'Full Credential',
                      `This would show the complete credential details in a full implementation.`
                    );
                  }}
                />
                <View style={styles.credentialActions}>
                  <TouchableOpacity
                    style={styles.actionButton}
                    onPress={() => handleShareCredential(cred.rawJson)}
                  >
                    <Text style={styles.actionButtonText}>Share</Text>
                  </TouchableOpacity>
                  
                  <TouchableOpacity
                    style={[styles.actionButton, styles.deleteButton]}
                    onPress={() => handleDeleteCredential(index)}
                  >
                    <Text style={styles.deleteButtonText}>Delete</Text>
                  </TouchableOpacity>
                </View>
              </View>
            ))}
          </ScrollView>
        ) : (
          <View style={styles.emptyState}>
            <Text style={styles.emptyStateText}>
              You don't have any credentials yet.
            </Text>
            <TouchableOpacity
              style={styles.importButton}
              onPress={() => setActiveTab('import')}
            >
              <Text style={styles.importButtonText}>Import a Credential</Text>
            </TouchableOpacity>
          </View>
        )
      ) : (
        <CredentialImport
          onImportSuccess={(report) => handleImportSuccess(report, '{}')} // In a real app, store the raw JSON
        />
      )}
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#F0F0F5',
  },
  header: {
    backgroundColor: '#007AFF',
    paddingTop: 60,
    paddingBottom: 20,
    paddingHorizontal: 16,
  },
  title: {
    fontSize: 28,
    fontWeight: 'bold',
    color: '#FFFFFF',
  },
  tabBar: {
    flexDirection: 'row',
    backgroundColor: '#FFFFFF',
    borderBottomWidth: 1,
    borderBottomColor: '#EEEEEE',
  },
  tab: {
    flex: 1,
    paddingVertical: 16,
    alignItems: 'center',
  },
  activeTab: {
    borderBottomWidth: 2,
    borderBottomColor: '#007AFF',
  },
  tabText: {
    fontSize: 16,
    color: '#666666',
  },
  activeTabText: {
    color: '#007AFF',
    fontWeight: '600',
  },
  contentContainer: {
    flex: 1,
    padding: 16,
  },
  emptyState: {
    flex: 1,
    padding: 16,
    justifyContent: 'center',
    alignItems: 'center',
  },
  emptyStateText: {
    fontSize: 16,
    color: '#666666',
    textAlign: 'center',
    marginBottom: 24,
  },
  importButton: {
    backgroundColor: '#007AFF',
    paddingVertical: 12,
    paddingHorizontal: 24,
    borderRadius: 8,
  },
  importButtonText: {
    color: '#FFFFFF',
    fontWeight: 'bold',
    fontSize: 16,
  },
  credentialContainer: {
    marginBottom: 16,
  },
  credentialActions: {
    flexDirection: 'row',
    justifyContent: 'flex-end',
    marginTop: 8,
    paddingHorizontal: 8,
  },
  actionButton: {
    paddingVertical: 8,
    paddingHorizontal: 16,
    borderRadius: 4,
    backgroundColor: '#007AFF',
    marginLeft: 8,
  },
  actionButtonText: {
    color: '#FFFFFF',
    fontWeight: '600',
    fontSize: 14,
  },
  deleteButton: {
    backgroundColor: '#FF3B30',
  },
  deleteButtonText: {
    color: '#FFFFFF',
    fontWeight: '600',
    fontSize: 14,
  },
}); 