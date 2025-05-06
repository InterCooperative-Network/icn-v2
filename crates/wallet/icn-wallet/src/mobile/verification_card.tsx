import React, { useEffect, useState } from 'react';
import { View, Text, StyleSheet, TouchableOpacity, Linking } from 'react-native';
import { VerificationReport } from '../bindings/verification';

interface VerificationCardProps {
  report: VerificationReport;
  onViewDetails?: () => void;
  onViewIssuer?: () => void;
  onViewPolicy?: () => void;
}

export const VerificationCard: React.FC<VerificationCardProps> = ({
  report,
  onViewDetails,
  onViewIssuer,
  onViewPolicy,
}) => {
  // Determine the verification status badge color and text
  const getBadgeInfo = () => {
    if (!report.signature_valid) {
      return { color: '#FF3B30', text: 'Invalid Signature', icon: '❌' };
    }
    if (report.is_revoked) {
      return { color: '#FF3B30', text: 'Revoked', icon: '❌' };
    }
    if (!report.is_trusted) {
      return { color: '#FF9500', text: 'Untrusted Issuer', icon: '⚠️' };
    }
    if (!report.lineage_verified && report.policy_version !== 'local') {
      return { color: '#FF9500', text: 'Policy Lineage Issue', icon: '⚠️' };
    }
    return { color: '#34C759', text: 'Valid', icon: '✅' };
  };

  const badge = getBadgeInfo();

  // Extract DID type (scheduler, worker, requestor) from DID string
  const getEntityRole = (did: string) => {
    if (did.includes('scheduler')) return 'Scheduler';
    if (did.includes('worker')) return 'Worker';
    if (did.includes('requestor')) return 'Requestor';
    return 'Unknown Role';
  };

  return (
    <View style={styles.card}>
      {/* Status badge */}
      <View style={[styles.badge, { backgroundColor: badge.color }]}>
        <Text style={styles.badgeText}>{badge.icon} {badge.text}</Text>
      </View>

      {/* Issuer information */}
      <View style={styles.section}>
        <Text style={styles.label}>Issuer</Text>
        <Text style={styles.value}>{report.issuer_did}</Text>
        <Text style={styles.subLabel}>{getEntityRole(report.issuer_did)}</Text>
        <TouchableOpacity style={styles.button} onPress={onViewIssuer}>
          <Text style={styles.buttonText}>View Issuer Profile</Text>
        </TouchableOpacity>
      </View>

      {/* Trust policy information */}
      <View style={styles.section}>
        <Text style={styles.label}>Trust Policy</Text>
        <Text style={styles.value}>{report.policy_version}</Text>
        <TouchableOpacity style={styles.button} onPress={onViewPolicy}>
          <Text style={styles.buttonText}>View Policy Details</Text>
        </TouchableOpacity>
      </View>

      {/* Additional verification information */}
      <View style={styles.section}>
        <Text style={styles.label}>Verification Details</Text>
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Signature Valid:</Text>
          <Text style={styles.rowValue}>{report.signature_valid ? '✅' : '❌'}</Text>
        </View>
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Is Trusted:</Text>
          <Text style={styles.rowValue}>{report.is_trusted ? '✅' : '❌'}</Text>
        </View>
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Is Revoked:</Text>
          <Text style={styles.rowValue}>{report.is_revoked ? '❌' : '✅'}</Text>
        </View>
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Lineage Verified:</Text>
          <Text style={styles.rowValue}>{report.lineage_verified ? '✅' : '❌'}</Text>
        </View>
        {report.error && (
          <View style={styles.errorContainer}>
            <Text style={styles.errorText}>{report.error}</Text>
          </View>
        )}
      </View>

      {/* View details button */}
      <TouchableOpacity style={styles.viewDetails} onPress={onViewDetails}>
        <Text style={styles.viewDetailsText}>View Full Details</Text>
      </TouchableOpacity>
    </View>
  );
};

const styles = StyleSheet.create({
  card: {
    backgroundColor: '#FFFFFF',
    borderRadius: 12,
    padding: 16,
    marginVertical: 8,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 4,
    elevation: 3,
  },
  badge: {
    alignSelf: 'flex-start',
    borderRadius: 16,
    paddingVertical: 4,
    paddingHorizontal: 12,
    marginBottom: 12,
  },
  badgeText: {
    color: '#FFFFFF',
    fontWeight: 'bold',
    fontSize: 14,
  },
  section: {
    marginBottom: 16,
    paddingBottom: 16,
    borderBottomWidth: 1,
    borderBottomColor: '#EEEEEE',
  },
  label: {
    fontSize: 16,
    fontWeight: 'bold',
    color: '#333333',
    marginBottom: 4,
  },
  value: {
    fontSize: 14,
    color: '#666666',
    marginBottom: 8,
    fontFamily: 'monospace',
  },
  subLabel: {
    fontSize: 14,
    color: '#999999',
    marginBottom: 8,
  },
  button: {
    backgroundColor: '#007AFF',
    borderRadius: 8,
    paddingVertical: 8,
    paddingHorizontal: 12,
    alignSelf: 'flex-start',
  },
  buttonText: {
    color: '#FFFFFF',
    fontWeight: '600',
    fontSize: 14,
  },
  row: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginBottom: 4,
  },
  rowLabel: {
    fontSize: 14,
    color: '#333333',
  },
  rowValue: {
    fontSize: 14,
    color: '#666666',
  },
  errorContainer: {
    backgroundColor: '#FFEEEE',
    padding: 8,
    borderRadius: 4,
    marginTop: 8,
  },
  errorText: {
    color: '#FF3B30',
    fontSize: 14,
  },
  viewDetails: {
    alignItems: 'center',
    padding: 8,
  },
  viewDetailsText: {
    color: '#007AFF',
    fontWeight: '600',
    fontSize: 14,
  },
}); 