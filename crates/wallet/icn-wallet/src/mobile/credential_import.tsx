import React, { useEffect, useState } from 'react';
import { View, Text, StyleSheet, TouchableOpacity, Platform, Linking, ActivityIndicator } from 'react-native';
import * as DocumentPicker from 'expo-document-picker';
import { BarCodeScanner } from 'expo-barcode-scanner';
import * as FileSystem from 'expo-file-system';
import { verifyCredential } from '../bindings/verification';
import { VerificationReport } from '../bindings/verification';
import { VerificationCard } from './verification_card';

interface CredentialImportProps {
  onImportSuccess: (report: VerificationReport) => void;
}

export const CredentialImport: React.FC<CredentialImportProps> = ({ onImportSuccess }) => {
  const [hasPermission, setHasPermission] = useState<boolean | null>(null);
  const [scanning, setScanning] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Request camera permissions for QR scanning
  useEffect(() => {
    (async () => {
      const { status } = await BarCodeScanner.requestPermissionsAsync();
      setHasPermission(status === 'granted');
    })();

    // Set up deep link handler
    const handleDeepLink = async (event: { url: string }) => {
      try {
        const url = event.url;
        if (url.startsWith('icn://dispatch')) {
          setLoading(true);
          setError(null);
          
          // Extract the credential data from URL parameters
          const credentialData = extractCredentialFromUrl(url);
          if (credentialData) {
            await processCredential(credentialData);
          } else {
            setError('Invalid dispatch URL format');
          }
        }
      } catch (e) {
        setError(`Error processing deep link: ${e.message}`);
      } finally {
        setLoading(false);
      }
    };

    // Add event listener for deep links
    Linking.addEventListener('url', handleDeepLink);

    // Check for initial URL (app opened via deep link)
    Linking.getInitialURL().then((initialUrl) => {
      if (initialUrl) {
        handleDeepLink({ url: initialUrl });
      }
    });

    // Clean up
    return () => {
      // Remove event listener when component unmounts
      // Note: In newer versions of React Native, this may need to be updated
      Linking.removeEventListener('url', handleDeepLink);
    };
  }, []);

  // Extract credential data from a deep link URL
  const extractCredentialFromUrl = (url: string): string | null => {
    try {
      const urlObj = new URL(url);
      const params = new URLSearchParams(urlObj.search);
      
      // Check for different possible parameter names
      const credential = params.get('credential') || params.get('vc') || params.get('data');
      
      if (credential) {
        // It might be base64 encoded
        try {
          return atob(credential);
        } catch {
          // If not base64, return as is
          return credential;
        }
      }
      
      return null;
    } catch (e) {
      console.error('Error parsing URL:', e);
      return null;
    }
  };

  // Handle QR code scanning
  const handleBarCodeScanned = async ({ type, data }) => {
    setScanning(false);
    setLoading(true);
    setError(null);
    
    try {
      // Check if it's a deep link or raw JSON
      if (data.startsWith('icn://')) {
        const credentialData = extractCredentialFromUrl(data);
        if (credentialData) {
          await processCredential(credentialData);
        } else {
          setError('Invalid QR code format');
        }
      } else {
        // Assume it's raw JSON data
        await processCredential(data);
      }
    } catch (e) {
      setError(`Error processing QR code: ${e.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Handle file import
  const handleFileImport = async () => {
    try {
      const result = await DocumentPicker.getDocumentAsync({
        type: 'application/json',
        copyToCacheDirectory: true,
      });
      
      if (result.type === 'success') {
        setLoading(true);
        setError(null);
        
        const fileContent = await FileSystem.readAsStringAsync(result.uri);
        await processCredential(fileContent);
      }
    } catch (e) {
      setError(`Error importing file: ${e.message}`);
    } finally {
      setLoading(false);
    }
  };

  // Process the credential data and verify it
  const processCredential = async (credentialJson: string) => {
    try {
      // Call our Rust verification function through UniFFI
      const reportJson = await verifyCredential(credentialJson);
      const report = JSON.parse(reportJson) as VerificationReport;
      
      if (report.error) {
        setError(`Verification error: ${report.error}`);
      } else {
        // Call success callback with the verification report
        onImportSuccess(report);
      }
    } catch (e) {
      setError(`Error verifying credential: ${e.message}`);
    }
  };

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Import Dispatch Credential</Text>
      
      {error && (
        <View style={styles.errorContainer}>
          <Text style={styles.errorText}>{error}</Text>
        </View>
      )}
      
      {loading ? (
        <View style={styles.loadingContainer}>
          <ActivityIndicator size="large" color="#007AFF" />
          <Text style={styles.loadingText}>Verifying credential...</Text>
        </View>
      ) : scanning ? (
        <View style={styles.scannerContainer}>
          {hasPermission === null ? (
            <Text>Requesting camera permission...</Text>
          ) : hasPermission === false ? (
            <Text>No access to camera</Text>
          ) : (
            <>
              <BarCodeScanner
                onBarCodeScanned={handleBarCodeScanned}
                style={styles.scanner}
              />
              <TouchableOpacity 
                style={styles.cancelButton}
                onPress={() => setScanning(false)}
              >
                <Text style={styles.cancelButtonText}>Cancel</Text>
              </TouchableOpacity>
            </>
          )}
        </View>
      ) : (
        <View style={styles.optionsContainer}>
          <Text style={styles.infoText}>
            Import an ICN dispatch credential to verify its authenticity and trust status.
          </Text>
          
          <TouchableOpacity
            style={styles.importButton}
            onPress={() => setScanning(true)}
          >
            <Text style={styles.importButtonText}>📷 Scan QR Code</Text>
          </TouchableOpacity>
          
          <TouchableOpacity
            style={styles.importButton}
            onPress={handleFileImport}
          >
            <Text style={styles.importButtonText}>📄 Import from File</Text>
          </TouchableOpacity>
          
          <Text style={styles.noteText}>
            You can also receive credentials via ICN deep links that will automatically open this app.
          </Text>
        </View>
      )}
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    padding: 16,
    backgroundColor: '#F9F9F9',
  },
  title: {
    fontSize: 24,
    fontWeight: 'bold',
    marginBottom: 16,
    color: '#333333',
  },
  errorContainer: {
    backgroundColor: '#FFEEEE',
    padding: 12,
    borderRadius: 8,
    marginBottom: 16,
  },
  errorText: {
    color: '#FF3B30',
    fontSize: 14,
  },
  loadingContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  loadingText: {
    marginTop: 16,
    fontSize: 16,
    color: '#666666',
  },
  scannerContainer: {
    flex: 1,
    position: 'relative',
  },
  scanner: {
    ...StyleSheet.absoluteFillObject,
  },
  cancelButton: {
    position: 'absolute',
    bottom: 20,
    alignSelf: 'center',
    backgroundColor: '#FFF',
    padding: 16,
    borderRadius: 8,
  },
  cancelButtonText: {
    fontSize: 16,
    fontWeight: 'bold',
    color: '#FF3B30',
  },
  optionsContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 16,
  },
  infoText: {
    fontSize: 16,
    color: '#666666',
    textAlign: 'center',
    marginBottom: 24,
  },
  importButton: {
    backgroundColor: '#007AFF',
    paddingVertical: 16,
    paddingHorizontal: 24,
    borderRadius: 12,
    marginBottom: 16,
    width: '100%',
    alignItems: 'center',
  },
  importButtonText: {
    color: '#FFFFFF',
    fontWeight: 'bold',
    fontSize: 16,
  },
  noteText: {
    fontSize: 14,
    color: '#999999',
    textAlign: 'center',
    marginTop: 16,
  },
}); 