// Typings for the verification report structure from Rust
export interface VerificationReport {
  issuer_did: string;
  signature_valid: boolean;
  is_trusted: boolean;
  is_revoked: boolean;
  policy_version: string;
  lineage_verified: boolean;
  overall_valid: boolean;
  capability_match?: boolean;
  error?: string;
  timestamp: string;
}

// Import the FFI functions from the generated bindings
import { ICN_WALLET } from './icn_wallet_generated';

/**
 * Verify a dispatch credential JSON string
 * 
 * This function calls into the Rust verification code via UniFFI
 * 
 * @param credentialJson The JSON string of the credential to verify
 * @returns A JSON string containing the verification report
 */
export function verifyCredential(credentialJson: string): string {
  return ICN_WALLET.verify_credential(credentialJson);
}

/**
 * Parse a verification report JSON string into a typed object
 * 
 * @param reportJson The JSON string returned by verifyCredential
 * @returns A typed VerificationReport object
 */
export function parseVerificationReport(reportJson: string): VerificationReport {
  return JSON.parse(reportJson) as VerificationReport;
}

/**
 * Utility function to check if a credential is valid
 * 
 * @param report The verification report
 * @returns true if the credential is completely valid, false otherwise
 */
export function isCredentialValid(report: VerificationReport): boolean {
  return (
    report.signature_valid &&
    report.is_trusted &&
    !report.is_revoked &&
    (report.lineage_verified || report.policy_version === 'local') &&
    report.overall_valid
  );
}

/**
 * Generate a human-readable verification status message
 * 
 * @param report The verification report
 * @returns A string describing the status of the credential
 */
export function getVerificationStatusMessage(report: VerificationReport): string {
  if (!report.signature_valid) {
    return 'Invalid signature - this credential has been tampered with';
  }
  
  if (report.is_revoked) {
    return 'This credential has been revoked and is no longer valid';
  }
  
  if (!report.is_trusted) {
    return 'The issuer of this credential is not trusted in your current policy';
  }
  
  if (!report.lineage_verified && report.policy_version !== 'local') {
    return 'Policy lineage verification failed - trust chain broken';
  }
  
  if (report.error) {
    return `Verification error: ${report.error}`;
  }
  
  return 'Valid credential - this dispatch was properly authorized';
}

/**
 * Extract the entity type (Scheduler, Worker, Requestor) from a DID
 * 
 * @param did The DID string to analyze
 * @returns The entity role as a string
 */
export function getEntityRoleFromDid(did: string): string {
  if (did.includes('scheduler')) return 'Scheduler';
  if (did.includes('worker')) return 'Worker';
  if (did.includes('requestor')) return 'Requestor';
  return 'Unknown Role';
}

/**
 * Generate a deep link for sharing a credential
 * 
 * @param credentialJson The credential JSON to share
 * @returns A sharable ICN deep link URL
 */
export function generateCredentialShareLink(credentialJson: string): string {
  // Base64 encode the credential to avoid URL encoding issues
  const encodedCredential = btoa(credentialJson);
  return `icn://dispatch?credential=${encodedCredential}`;
} 