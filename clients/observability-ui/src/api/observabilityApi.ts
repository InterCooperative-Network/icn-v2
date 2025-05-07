import axios from 'axios';

// Base API URL - this would be replaced with actual API endpoint
const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:3001/api';

// Define types that match the Rust structures
export interface DagNodeInfo {
  cid: string;
  timestamp: string;
  signer_did: string;
  payload_type: string;
  payload_preview: string;
  parent_cids: string[];
  scope_type: string;
  scope_id?: string;
  federation_id: string;
}

export interface PolicyInfo {
  cid: string;
  timestamp: string;
  content: any;
  update_trail: PolicyUpdateInfo[];
}

export interface PolicyUpdateInfo {
  cid: string;
  timestamp: string;
  proposer: string;
  votes: VoteInfo[];
}

export interface VoteInfo {
  cid: string;
  voter: string;
  decision: string;
  reason?: string;
}

export interface QuorumInfo {
  cid: string;
  is_valid: boolean;
  error_message?: string;
  required_signers: string[];
  actual_signers: SignerInfo[];
  node: {
    timestamp: string;
    author: string;
  };
}

export interface SignerInfo {
  did: string;
  role?: string;
  scope?: string;
}

export interface ActivityEvent {
  activity_type: string;
  cid: string;
  timestamp: string;
  actor: string;
  description: string;
  details?: any;
}

export interface FederationOverview {
  federation: {
    id: string;
    description?: string;
    head?: string;
  };
  members: {
    cooperatives: {
      count: number;
      items: MemberInfo[];
    };
    communities: {
      count: number;
      items: MemberInfo[];
    };
  };
}

export interface MemberInfo {
  id: string;
  type: string;
  name?: string;
  latest_head?: string;
  latest_timestamp?: string;
}

// Proposal types for submission
export interface ProposalSubmission {
  key_file: string;
  federation: string;
  title: string;
  description: string;
  proposal_type: string;
  voting_threshold: string;
  voting_duration: string;
  execution_cid?: string;
  thread_cid?: string;
  parameters?: string;
  scope_type?: string;
  scope_id?: string;
}

export interface ProposalResult {
  proposal_id: string;
  cid: string;
  title: string;
  submitter: string;
  status: string;
  message: string;
}

// Define API service
const observabilityApi = {
  // DAG View API
  async getDagView(scopeType: string, scopeId: string, limit: number = 50): Promise<DagNodeInfo[]> {
    try {
      const response = await axios.get(`${API_BASE_URL}/dag-view`, {
        params: { scope_type: scopeType, scope_id: scopeId, limit, output: 'json' }
      });
      return response.data.nodes;
    } catch (error) {
      console.error('Error fetching DAG view:', error);
      throw error;
    }
  },

  // Policy Inspector API
  async getPolicy(scopeType: string, scopeId: string): Promise<PolicyInfo> {
    try {
      const response = await axios.get(`${API_BASE_URL}/inspect-policy`, {
        params: { scope_type: scopeType, scope_id: scopeId, output: 'json' }
      });
      return response.data.policy;
    } catch (error) {
      console.error('Error fetching policy:', error);
      throw error;
    }
  },

  // Quorum Validator API
  async validateQuorum(cid: string, showSigners: boolean = true): Promise<QuorumInfo> {
    try {
      const response = await axios.get(`${API_BASE_URL}/validate-quorum`, {
        params: { cid, show_signers: showSigners, output: 'json' }
      });
      return response.data.quorum;
    } catch (error) {
      console.error('Error validating quorum:', error);
      throw error;
    }
  },

  // Activity Log API
  async getActivityLog(scopeType: string, scopeId: string, limit: number = 50): Promise<ActivityEvent[]> {
    try {
      const response = await axios.get(`${API_BASE_URL}/activity-log`, {
        params: { scope_type: scopeType, scope_id: scopeId, limit, output: 'json' }
      });
      return response.data.activities;
    } catch (error) {
      console.error('Error fetching activity log:', error);
      throw error;
    }
  },

  // Federation Overview API
  async getFederationOverview(federationId: string): Promise<FederationOverview> {
    try {
      const response = await axios.get(`${API_BASE_URL}/federation-overview`, {
        params: { federation_id: federationId, output: 'json' }
      });
      return response.data;
    } catch (error) {
      console.error('Error fetching federation overview:', error);
      throw error;
    }
  },

  // Submit Proposal API
  async submitProposal(proposal: ProposalSubmission): Promise<ProposalResult> {
    try {
      const response = await axios.post(`${API_BASE_URL}/submit-proposal`, proposal);
      return response.data;
    } catch (error) {
      console.error('Error submitting proposal:', error);
      throw error;
    }
  },

  // Get Proposal List API
  async getProposals(
    scopeType: string, 
    scopeId: string, 
    status?: string, 
    limit: number = 20
  ): Promise<any[]> {
    try {
      const response = await axios.get(`${API_BASE_URL}/proposals`, {
        params: { 
          scope_type: scopeType, 
          scope_id: scopeId, 
          status, 
          limit,
          output: 'json' 
        }
      });
      return response.data.proposals;
    } catch (error) {
      console.error('Error fetching proposals:', error);
      throw error;
    }
  },

  // Get Proposal Details API
  async getProposalDetails(proposalId: string): Promise<any> {
    try {
      const response = await axios.get(`${API_BASE_URL}/proposal-details`, {
        params: { proposal_id: proposalId, output: 'json' }
      });
      return response.data.proposal;
    } catch (error) {
      console.error('Error fetching proposal details:', error);
      throw error;
    }
  },

  // Vote on Proposal API
  async voteOnProposal(
    proposalId: string, 
    keyFile: string, 
    decision: string, 
    reason?: string
  ): Promise<any> {
    try {
      const response = await axios.post(`${API_BASE_URL}/vote`, {
        proposal_id: proposalId,
        key_file: keyFile,
        decision,
        reason
      });
      return response.data;
    } catch (error) {
      console.error('Error voting on proposal:', error);
      throw error;
    }
  }
};

export default observabilityApi; 