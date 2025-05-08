import { getSeededData } from './seedData';
import { 
  ActivityEvent, 
  DagNodeInfo, 
  FederationOverview,
  MemberInfo,
  PolicyInfo,
  QuorumInfo,
  ProposalSubmission,
  ProposalResult
} from '../api/observabilityApi';

/**
 * Demo API service that simulates real API responses using seeded data
 */
class DemoApiService {
  private data = getSeededData();
  
  // Helpers
  private delay<T>(data: T, delayMs: number = 500): Promise<T> {
    return new Promise(resolve => setTimeout(() => resolve(data), delayMs));
  }
  
  private findMemberById(id: string): MemberInfo | undefined {
    return [...this.data.cooperatives, ...this.data.communities].find(
      member => member.id === id
    ) as MemberInfo | undefined;
  }

  // DAG View API
  async getDagView(scopeType: string, scopeId: string, limit: number = 50): Promise<DagNodeInfo[]> {
    console.log(`Demo API: Getting DAG view for ${scopeType}/${scopeId}`);
    
    // Filter nodes by scope
    let filteredNodes = this.data.dagNodes.filter(node => {
      if (scopeType === 'federation' && node.federation_id === scopeId) {
        return true;
      }
      return node.scope_type === scopeType && node.scope_id === scopeId;
    });
    
    // Sort by timestamp (most recent first) and limit
    filteredNodes = filteredNodes
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
      .slice(0, limit);
      
    return this.delay(filteredNodes);
  }
  
  // Policy Inspector API
  async getPolicy(scopeType: string, scopeId: string): Promise<PolicyInfo> {
    console.log(`Demo API: Getting policy for ${scopeType}/${scopeId}`);
    
    // For demo purposes, we'll return the same policy for any scope
    return this.delay(this.data.policy as PolicyInfo);
  }
  
  // Quorum Validator API
  async validateQuorum(cid: string, showSigners: boolean = true): Promise<QuorumInfo> {
    console.log(`Demo API: Validating quorum for ${cid}`);
    
    // Find a completed proposal with several votes for demo
    const completedProposal = this.data.proposals.find(p => p.status === 'completed');
    
    if (!completedProposal) {
      throw new Error('No completed proposals found');
    }
    
    // Generate a mock quorum validation result
    const result: QuorumInfo = {
      cid: completedProposal.cid,
      is_valid: true,
      required_signers: this.data.cooperatives.map(c => c.did),
      actual_signers: completedProposal.votes.map(vote => ({
        did: vote.voter,
        role: this.data.users.find(u => u.did === vote.voter)?.organization.startsWith('coop') 
          ? 'cooperative' : 'community',
        scope: this.data.users.find(u => u.did === vote.voter)?.organization || '',
      })),
      node: {
        timestamp: completedProposal.created_at,
        author: completedProposal.submitter,
      }
    };
    
    // Check if the mock quorum is valid based on voting threshold
    if (completedProposal.voting_threshold === 'unanimous' && 
        result.actual_signers.length < result.required_signers.length) {
      result.is_valid = false;
      result.error_message = 'Unanimous voting required, but not all signers participated';
    } else if (completedProposal.voting_threshold === 'supermajority') {
      const approveVotes = completedProposal.votes.filter(v => v.decision === 'approve').length;
      const totalVotes = completedProposal.votes.length;
      if (approveVotes / totalVotes < 0.66) {
        result.is_valid = false;
        result.error_message = 'Supermajority threshold not met';
      }
    } else if (completedProposal.voting_threshold === 'majority') {
      const approveVotes = completedProposal.votes.filter(v => v.decision === 'approve').length;
      const rejectVotes = completedProposal.votes.filter(v => v.decision === 'reject').length;
      if (approveVotes <= rejectVotes) {
        result.is_valid = false;
        result.error_message = 'Majority threshold not met';
      }
    }
    
    return this.delay(result);
  }
  
  // Activity Log API
  async getActivityLog(scopeType: string, scopeId: string, limit: number = 50): Promise<ActivityEvent[]> {
    console.log(`Demo API: Getting activity log for ${scopeType}/${scopeId}`);
    
    // Filter activities by scope
    let filteredActivities = this.data.activities;
    
    if (scopeType !== 'federation' || scopeId !== this.data.federation.id) {
      // For specific scopes, only show activities relevant to that scope
      filteredActivities = filteredActivities.filter(activity => {
        if (activity.details?.scope_type === scopeType && activity.details?.scope_id === scopeId) {
          return true;
        }
        if (activity.details?.member_id === scopeId) {
          return true;
        }
        return false;
      });
    }
    
    // Limit the results
    filteredActivities = filteredActivities.slice(0, limit);
    
    return this.delay(filteredActivities);
  }
  
  // Federation Overview API
  async getFederationOverview(federationId: string): Promise<FederationOverview> {
    console.log(`Demo API: Getting federation overview for ${federationId}`);
    
    // Return seeded federation overview
    const overview: FederationOverview = {
      federation: {
        id: this.data.federation.id,
        description: this.data.federation.description,
        head: this.data.federation.head,
      },
      members: {
        cooperatives: {
          count: this.data.cooperatives.length,
          items: this.data.cooperatives as MemberInfo[],
        },
        communities: {
          count: this.data.communities.length,
          items: this.data.communities as MemberInfo[],
        },
      },
    };
    
    return this.delay(overview);
  }
  
  // Submit Proposal API
  async submitProposal(proposal: ProposalSubmission): Promise<ProposalResult> {
    console.log('Demo API: Submitting proposal', proposal);
    
    // Generate a new proposal ID
    const proposalId = `proposal-${Math.random().toString(36).substring(2, 10)}`;
    
    // Create a simulated result
    const result: ProposalResult = {
      proposal_id: proposalId,
      cid: `bafy${Math.random().toString(36).substring(2, 15)}${Math.random().toString(36).substring(2, 15)}`,
      title: proposal.title,
      submitter: this.data.users[0].did, // Use the first user as the submitter for demo
      status: 'draft',
      message: 'Proposal submitted successfully',
    };
    
    // Add the new proposal to our seed data for demo persistence
    const newProposal = {
      id: proposalId,
      cid: result.cid,
      title: proposal.title,
      description: proposal.description,
      proposal_type: proposal.proposal_type,
      status: 'active',
      submitter: this.data.users[0].did,
      submitter_name: this.data.users[0].name,
      organization: this.data.users[0].organization,
      voting_threshold: proposal.voting_threshold,
      voting_duration: proposal.voting_duration,
      voting_start_time: new Date().toISOString(),
      voting_end_time: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
      votes: [],
      parameters: proposal.parameters,
      scope_type: proposal.scope_type || 'federation',
      scope_id: proposal.scope_id || this.data.federation.id,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    
    this.data.proposals.push(newProposal);
    
    // Also add corresponding activity
    this.data.activities.unshift({
      activity_type: 'Proposal Submitted',
      cid: result.cid,
      timestamp: new Date().toISOString(),
      actor: this.data.users[0].did,
      description: `Proposal submitted: ${proposal.title}`,
      details: {
        proposal_id: proposalId,
        proposal_cid: result.cid,
        proposal_title: proposal.title,
        proposal_type: proposal.proposal_type,
        scope_type: proposal.scope_type || 'federation',
        scope_id: proposal.scope_id || this.data.federation.id,
      },
    });
    
    // Also create DAG node
    this.data.dagNodes.push({
      cid: result.cid,
      timestamp: new Date().toISOString(),
      signer_did: this.data.users[0].did,
      payload_type: 'Proposal',
      payload_preview: proposal.title,
      parent_cids: [this.data.dagNodes[0].cid], // Use federation node as parent
      scope_type: proposal.scope_type || 'federation',
      scope_id: proposal.scope_id || this.data.federation.id,
      federation_id: this.data.federation.id,
    });
    
    return this.delay(result, 1000); // Longer delay to simulate processing
  }
  
  // Get Proposals List API
  async getProposals(
    scopeType: string, 
    scopeId: string, 
    status?: string, 
    limit: number = 20
  ): Promise<any[]> {
    console.log(`Demo API: Getting proposals for ${scopeType}/${scopeId}`);
    
    // Filter proposals by scope and status
    let filteredProposals = this.data.proposals.filter(proposal => {
      // Match scope
      if (proposal.scope_type !== scopeType || proposal.scope_id !== scopeId) {
        return false;
      }
      
      // Match status if provided
      if (status && proposal.status !== status) {
        return false;
      }
      
      return true;
    });
    
    // Sort by created_at (newest first) and limit
    filteredProposals = filteredProposals
      .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
      .slice(0, limit);
      
    return this.delay(filteredProposals);
  }
  
  // Get Proposal Details API
  async getProposalDetails(proposalId: string): Promise<any> {
    console.log(`Demo API: Getting proposal details for ${proposalId}`);
    
    // Find the proposal by ID
    const proposal = this.data.proposals.find(p => p.id === proposalId);
    
    if (!proposal) {
      throw new Error(`Proposal with ID ${proposalId} not found`);
    }
    
    return this.delay(proposal);
  }
  
  // Vote on Proposal API
  async voteOnProposal(
    proposalId: string, 
    keyFile: string, 
    decision: string, 
    reason?: string
  ): Promise<any> {
    console.log(`Demo API: Voting on proposal ${proposalId}`);
    
    // Find the proposal by ID
    const proposalIndex = this.data.proposals.findIndex(p => p.id === proposalId);
    
    if (proposalIndex === -1) {
      throw new Error(`Proposal with ID ${proposalId} not found`);
    }
    
    const proposal = this.data.proposals[proposalIndex];
    
    // Check if proposal is active
    if (proposal.status !== 'active') {
      throw new Error(`Proposal is not active, current status: ${proposal.status}`);
    }
    
    // Check if voting period is still open
    if (new Date(proposal.voting_end_time).getTime() < Date.now()) {
      throw new Error('Voting period has ended');
    }
    
    // Use a random user that hasn't voted yet
    const voters = proposal.votes.map(v => v.voter);
    const availableUsers = this.data.users.filter(u => !voters.includes(u.did));
    
    if (availableUsers.length === 0) {
      throw new Error('All users have already voted on this proposal');
    }
    
    const voter = availableUsers[0];
    
    // Create the vote
    const voteCid = `bafy${Math.random().toString(36).substring(2, 15)}${Math.random().toString(36).substring(2, 15)}`;
    const voteTimestamp = new Date().toISOString();
    
    const vote = {
      cid: voteCid,
      voter: voter.did,
      voter_name: voter.name,
      decision,
      reason: reason || `I ${decision === 'approve' ? 'support' : 'oppose'} this proposal`,
      timestamp: voteTimestamp,
    };
    
    // Add the vote to the proposal
    this.data.proposals[proposalIndex].votes.push(vote);
    this.data.proposals[proposalIndex].updated_at = voteTimestamp;
    
    // Add vote activity
    this.data.activities.unshift({
      activity_type: 'Vote Cast',
      cid: voteCid,
      timestamp: voteTimestamp,
      actor: voter.did,
      description: `Vote ${decision} on proposal: ${proposal.title}`,
      details: {
        proposal_id: proposalId,
        proposal_cid: proposal.cid,
        voter: voter.did,
        decision,
        reason: vote.reason,
      },
    });
    
    // Add vote DAG node
    this.data.dagNodes.push({
      cid: voteCid,
      timestamp: voteTimestamp,
      signer_did: voter.did,
      payload_type: 'Vote',
      payload_preview: `Vote: ${decision} - ${vote.reason.substring(0, 30)}...`,
      parent_cids: [proposal.cid], // Proposal as parent
      scope_type: proposal.scope_type,
      scope_id: proposal.scope_id,
      federation_id: this.data.federation.id,
    });
    
    // Check if proposal should be completed based on votes
    const approveVotes = proposal.votes.filter(v => v.decision === 'approve').length;
    const rejectVotes = proposal.votes.filter(v => v.decision === 'reject').length;
    const totalVotes = proposal.votes.length;
    
    // If all users have voted or we have a clear majority/supermajority
    if (totalVotes === this.data.users.length || 
        (proposal.voting_threshold === 'majority' && (approveVotes > this.data.users.length / 2 || 
                                                     rejectVotes > this.data.users.length / 2)) ||
        (proposal.voting_threshold === 'supermajority' && (approveVotes > this.data.users.length * 0.66 || 
                                                          rejectVotes > this.data.users.length * 0.66))) {
      
      this.data.proposals[proposalIndex].status = 'completed';
      
      // Add completion activity
      this.data.activities.unshift({
        activity_type: 'Proposal Completed',
        cid: `bafy${Math.random().toString(36).substring(2, 15)}`,
        timestamp: new Date().toISOString(),
        actor: proposal.submitter,
        description: `Proposal completed: ${proposal.title}`,
        details: {
          proposal_id: proposalId,
          proposal_cid: proposal.cid,
          proposal_title: proposal.title,
          outcome: approveVotes > rejectVotes ? 'approved' : 'rejected',
        },
      });
    }
    
    return this.delay({
      success: true,
      message: 'Vote recorded successfully',
      vote: {
        proposal_id: proposalId,
        voter: voter.did,
        voter_name: voter.name,
        decision,
        cid: voteCid,
        timestamp: voteTimestamp,
      },
    }, 1000);
  }
}

export default new DemoApiService(); 