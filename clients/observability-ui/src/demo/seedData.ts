// Mock data for demo federation
import { v4 as uuidv4 } from 'uuid';

// Generate a random DID key
const generateDid = () => `did:key:z${Math.random().toString(36).substring(2, 15)}${Math.random().toString(36).substring(2, 15)}`;

// Generate a random CID
const generateCid = () => `bafy${Math.random().toString(36).substring(2, 15)}${Math.random().toString(36).substring(2, 15)}`;

// Mock federation data
export const federation = {
  id: 'fed-democoop-network',
  name: 'DemoCoop Network',
  description: 'A demo federation for the InterCooperative Network',
  head: generateCid(),
  created_at: new Date(Date.now() - 90 * 24 * 60 * 60 * 1000).toISOString(), // 90 days ago
};

// Mock cooperative members
export const cooperatives = [
  {
    id: 'coop-techworkers',
    name: 'Tech Workers Cooperative',
    type: 'Cooperative',
    did: generateDid(),
    created_at: new Date(Date.now() - 85 * 24 * 60 * 60 * 1000).toISOString(),
    latest_head: generateCid(),
    latest_timestamp: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: 'coop-foodgrowers',
    name: 'Food Growers Collective',
    type: 'Cooperative',
    did: generateDid(),
    created_at: new Date(Date.now() - 80 * 24 * 60 * 60 * 1000).toISOString(),
    latest_head: generateCid(),
    latest_timestamp: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: 'coop-creativecommons',
    name: 'Creative Commons Cooperative',
    type: 'Cooperative',
    did: generateDid(),
    created_at: new Date(Date.now() - 75 * 24 * 60 * 60 * 1000).toISOString(),
    latest_head: generateCid(),
    latest_timestamp: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
  },
];

// Mock community members
export const communities = [
  {
    id: 'com-localdevs',
    name: 'Local Developers Community',
    type: 'Community',
    did: generateDid(),
    created_at: new Date(Date.now() - 70 * 24 * 60 * 60 * 1000).toISOString(),
    latest_head: generateCid(),
    latest_timestamp: new Date(Date.now() - 4 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: 'com-opensourcers',
    name: 'Open Source Contributors',
    type: 'Community',
    did: generateDid(),
    created_at: new Date(Date.now() - 65 * 24 * 60 * 60 * 1000).toISOString(),
    latest_head: generateCid(),
    latest_timestamp: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000).toISOString(),
  },
];

// Mock users (federation representatives)
export const users = [
  {
    id: 'user1',
    name: 'Maria Rodriguez',
    did: generateDid(),
    key_file: '/home/users/maria/key.jwk',
    organization: cooperatives[0].id,
  },
  {
    id: 'user2',
    name: 'James Chen',
    did: generateDid(),
    key_file: '/home/users/james/key.jwk',
    organization: cooperatives[1].id,
  },
  {
    id: 'user3',
    name: 'Serena Johnson',
    did: generateDid(),
    key_file: '/home/users/serena/key.jwk',
    organization: cooperatives[2].id,
  },
  {
    id: 'user4',
    name: 'Omar Hassan',
    did: generateDid(),
    key_file: '/home/users/omar/key.jwk',
    organization: communities[0].id,
  },
  {
    id: 'user5',
    name: 'Ling Wei',
    did: generateDid(),
    key_file: '/home/users/ling/key.jwk',
    organization: communities[1].id,
  },
];

// Mock policy data
export const policy = {
  cid: generateCid(),
  timestamp: new Date(Date.now() - 60 * 24 * 60 * 60 * 1000).toISOString(),
  content: {
    version: '1.0',
    name: 'Federation Governance Policy',
    quorum_rules: {
      proposals: 'majority',
      policy_changes: 'supermajority',
      member_addition: 'majority',
      member_removal: 'supermajority',
    },
    voting_durations: {
      standard: 7 * 24 * 60 * 60, // 7 days in seconds
      emergency: 24 * 60 * 60, // 1 day in seconds
    },
    roles: {
      admin: {
        description: 'Federation administrator',
        capabilities: ['propose', 'vote', 'execute'],
      },
      member: {
        description: 'Federation member',
        capabilities: ['propose', 'vote'],
      },
      observer: {
        description: 'Federation observer',
        capabilities: ['propose'],
      },
    },
  },
  update_trail: [
    {
      cid: generateCid(),
      timestamp: new Date(Date.now() - 60 * 24 * 60 * 60 * 1000).toISOString(),
      proposer: users[0].did,
      votes: [
        {
          cid: generateCid(),
          voter: users[0].did,
          decision: 'approve',
          reason: 'Initial policy creation',
        },
        {
          cid: generateCid(),
          voter: users[1].did,
          decision: 'approve',
          reason: 'Looks good to me',
        },
        {
          cid: generateCid(),
          voter: users[2].did,
          decision: 'approve',
          reason: 'Well structured policy',
        },
      ],
    },
  ],
};

// Mock proposals
export const proposals = [
  {
    id: `proposal-${uuidv4().substring(0, 8)}`,
    cid: generateCid(),
    title: 'Add New Renewable Energy Cooperative',
    description: 'Proposal to add a new renewable energy cooperative to our federation.',
    proposal_type: 'memberAddition',
    status: 'active',
    submitter: users[0].did,
    submitter_name: users[0].name,
    organization: users[0].organization,
    voting_threshold: 'majority',
    voting_duration: 7 * 24 * 60 * 60, // 7 days in seconds
    voting_start_time: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
    voting_end_time: new Date(Date.now() + 4 * 24 * 60 * 60 * 1000).toISOString(),
    votes: [
      {
        cid: generateCid(),
        voter: users[0].did,
        voter_name: users[0].name,
        decision: 'approve',
        reason: 'Great addition to our federation',
        timestamp: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
      },
      {
        cid: generateCid(),
        voter: users[1].did,
        voter_name: users[1].name,
        decision: 'approve',
        reason: 'Will bring valuable expertise',
        timestamp: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
      },
    ],
    parameters: JSON.stringify({
      new_member: {
        id: 'coop-renewables',
        name: 'Renewable Energy Cooperative',
        type: 'Cooperative',
        description: 'A cooperative focused on renewable energy solutions',
      },
    }, null, 2),
    scope_type: 'federation',
    scope_id: federation.id,
    created_at: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
    updated_at: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: `proposal-${uuidv4().substring(0, 8)}`,
    cid: generateCid(),
    title: 'Community Garden Initiative',
    description: 'Proposal to start a community garden project across all member organizations.',
    proposal_type: 'textProposal',
    status: 'active',
    submitter: users[1].did,
    submitter_name: users[1].name,
    organization: users[1].organization,
    voting_threshold: 'majority',
    voting_duration: 5 * 24 * 60 * 60, // 5 days in seconds
    voting_start_time: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
    voting_end_time: new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString(),
    votes: [
      {
        cid: generateCid(),
        voter: users[1].did,
        voter_name: users[1].name,
        decision: 'approve',
        reason: 'As the proposer, I support this initiative',
        timestamp: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
      },
    ],
    parameters: JSON.stringify({
      project_duration: '6 months',
      required_resources: {
        land: 'Each member provides minimum 100 sq ft space',
        seeds: 'Federation budget will cover initial seeds',
        tools: 'Each member organization to provide tools',
      },
    }, null, 2),
    scope_type: 'federation',
    scope_id: federation.id,
    created_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
    updated_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: `proposal-${uuidv4().substring(0, 8)}`,
    cid: generateCid(),
    title: 'Software Licensing Policy Update',
    description: 'Proposal to update our software licensing policy to prioritize copyleft licenses.',
    proposal_type: 'configChange',
    status: 'completed',
    submitter: users[2].did,
    submitter_name: users[2].name,
    organization: users[2].organization,
    voting_threshold: 'supermajority',
    voting_duration: 10 * 24 * 60 * 60, // 10 days in seconds
    voting_start_time: new Date(Date.now() - 15 * 24 * 60 * 60 * 1000).toISOString(),
    voting_end_time: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000).toISOString(),
    votes: [
      {
        cid: generateCid(),
        voter: users[0].did,
        voter_name: users[0].name,
        decision: 'approve',
        reason: 'Better aligns with our cooperative values',
        timestamp: new Date(Date.now() - 14 * 24 * 60 * 60 * 1000).toISOString(),
      },
      {
        cid: generateCid(),
        voter: users[1].did,
        voter_name: users[1].name,
        decision: 'approve',
        reason: 'Supports our open-source commitment',
        timestamp: new Date(Date.now() - 13 * 24 * 60 * 60 * 1000).toISOString(),
      },
      {
        cid: generateCid(),
        voter: users[2].did,
        voter_name: users[2].name,
        decision: 'approve',
        reason: 'As the proposer, I fully support this change',
        timestamp: new Date(Date.now() - 15 * 24 * 60 * 60 * 1000).toISOString(),
      },
      {
        cid: generateCid(),
        voter: users[3].did,
        voter_name: users[3].name,
        decision: 'approve',
        reason: 'This will benefit all our development work',
        timestamp: new Date(Date.now() - 12 * 24 * 60 * 60 * 1000).toISOString(),
      },
      {
        cid: generateCid(),
        voter: users[4].did,
        voter_name: users[4].name,
        decision: 'reject',
        reason: 'Concerned about compatibility with existing projects',
        timestamp: new Date(Date.now() - 10 * 24 * 60 * 60 * 1000).toISOString(),
      },
    ],
    parameters: JSON.stringify({
      new_policy: {
        preferred_licenses: ['GPL-3.0', 'AGPL-3.0', 'MPL-2.0'],
        acceptable_licenses: ['Apache-2.0', 'MIT', 'BSD-3-Clause'],
        discouraged_licenses: ['Proprietary', 'Custom Non-Free'],
      },
    }, null, 2),
    scope_type: 'federation',
    scope_id: federation.id,
    created_at: new Date(Date.now() - 15 * 24 * 60 * 60 * 1000).toISOString(),
    updated_at: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: `proposal-${uuidv4().substring(0, 8)}`,
    cid: generateCid(),
    title: 'Local Developer Hackathon',
    description: 'Proposal to organize a hackathon focused on cooperative technologies.',
    proposal_type: 'textProposal',
    status: 'active',
    submitter: users[3].did,
    submitter_name: users[3].name,
    organization: users[3].organization,
    voting_threshold: 'majority',
    voting_duration: 5 * 24 * 60 * 60, // 5 days in seconds
    voting_start_time: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
    voting_end_time: new Date(Date.now() + 4 * 24 * 60 * 60 * 1000).toISOString(),
    votes: [
      {
        cid: generateCid(),
        voter: users[3].did,
        voter_name: users[3].name,
        decision: 'approve',
        reason: 'As the proposer, I believe this will strengthen our community',
        timestamp: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
      },
    ],
    parameters: JSON.stringify({
      event_details: {
        date: 'Next month, 15-16th',
        location: 'Community Center',
        budget: '2000 credits from community fund',
        tracks: ['Cooperative Governance Tools', 'Decentralized Storage', 'Mobile Accessibility'],
      },
    }, null, 2),
    scope_type: 'community',
    scope_id: communities[0].id,
    created_at: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
    updated_at: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
  },
];

// Mock DAG nodes
export const generateDagNodes = () => {
  const nodes = [];
  
  // Federation creation node
  nodes.push({
    cid: generateCid(),
    timestamp: federation.created_at,
    signer_did: users[0].did,
    payload_type: 'Federation',
    payload_preview: `Federation creation: ${federation.name}`,
    parent_cids: [],
    scope_type: 'federation',
    scope_id: federation.id,
    federation_id: federation.id,
  });
  
  // Cooperative join nodes
  cooperatives.forEach(coop => {
    nodes.push({
      cid: generateCid(),
      timestamp: coop.created_at,
      signer_did: users.find(u => u.organization === coop.id)?.did || coop.did,
      payload_type: 'MemberJoin',
      payload_preview: `Cooperative joined: ${coop.name}`,
      parent_cids: [nodes[0].cid], // Federation node as parent
      scope_type: 'federation',
      scope_id: federation.id,
      federation_id: federation.id,
    });
  });
  
  // Community join nodes
  communities.forEach(comm => {
    nodes.push({
      cid: generateCid(),
      timestamp: comm.created_at,
      signer_did: users.find(u => u.organization === comm.id)?.did || comm.did,
      payload_type: 'MemberJoin',
      payload_preview: `Community joined: ${comm.name}`,
      parent_cids: [nodes[0].cid], // Federation node as parent
      scope_type: 'federation',
      scope_id: federation.id,
      federation_id: federation.id,
    });
  });
  
  // Policy creation node
  nodes.push({
    cid: policy.cid,
    timestamp: policy.timestamp,
    signer_did: users[0].did,
    payload_type: 'Policy',
    payload_preview: 'Federation Governance Policy v1.0',
    parent_cids: [nodes[0].cid], // Federation node as parent
    scope_type: 'federation',
    scope_id: federation.id,
    federation_id: federation.id,
  });
  
  // Proposal nodes and vote nodes
  proposals.forEach(proposal => {
    // Proposal node
    const proposalNode = {
      cid: proposal.cid,
      timestamp: proposal.created_at,
      signer_did: proposal.submitter,
      payload_type: 'Proposal',
      payload_preview: proposal.title,
      parent_cids: [nodes[Math.floor(Math.random() * (nodes.length - 1)) + 1].cid], // Random parent
      scope_type: proposal.scope_type,
      scope_id: proposal.scope_id,
      federation_id: federation.id,
    };
    nodes.push(proposalNode);
    
    // Vote nodes
    proposal.votes.forEach(vote => {
      nodes.push({
        cid: vote.cid,
        timestamp: vote.timestamp,
        signer_did: vote.voter,
        payload_type: 'Vote',
        payload_preview: `Vote: ${vote.decision} - ${vote.reason.substring(0, 30)}...`,
        parent_cids: [proposalNode.cid], // Proposal as parent
        scope_type: proposal.scope_type,
        scope_id: proposal.scope_id,
        federation_id: federation.id,
      });
    });
  });
  
  return nodes;
};

// Generate activity events
export const generateActivityEvents = () => {
  const activities = [];
  
  // Federation creation activity
  activities.push({
    activity_type: 'Federation Created',
    cid: generateCid(),
    timestamp: federation.created_at,
    actor: users[0].did,
    description: `Federation "${federation.name}" was created`,
    details: {
      federation_id: federation.id,
      federation_name: federation.name,
      description: federation.description,
    },
  });
  
  // Member join activities
  [...cooperatives, ...communities].forEach(member => {
    activities.push({
      activity_type: 'Member Joined',
      cid: generateCid(),
      timestamp: member.created_at,
      actor: users.find(u => u.organization === member.id)?.did || member.did,
      description: `${member.type} "${member.name}" joined the federation`,
      details: {
        member_id: member.id,
        member_name: member.name,
        member_type: member.type,
      },
    });
  });
  
  // Policy creation activity
  activities.push({
    activity_type: 'Policy Changed',
    cid: policy.cid,
    timestamp: policy.timestamp,
    actor: users[0].did,
    description: 'Federation Governance Policy was established',
    details: {
      policy_cid: policy.cid,
      policy_name: policy.content.name,
      policy_version: policy.content.version,
    },
  });
  
  // Proposal activities
  proposals.forEach(proposal => {
    activities.push({
      activity_type: 'Proposal Submitted',
      cid: proposal.cid,
      timestamp: proposal.created_at,
      actor: proposal.submitter,
      description: `Proposal submitted: ${proposal.title}`,
      details: {
        proposal_id: proposal.id,
        proposal_cid: proposal.cid,
        proposal_title: proposal.title,
        proposal_type: proposal.proposal_type,
        scope_type: proposal.scope_type,
        scope_id: proposal.scope_id,
      },
    });
    
    // Vote activities
    proposal.votes.forEach(vote => {
      activities.push({
        activity_type: 'Vote Cast',
        cid: vote.cid,
        timestamp: vote.timestamp,
        actor: vote.voter,
        description: `Vote ${vote.decision} on proposal: ${proposal.title}`,
        details: {
          proposal_id: proposal.id,
          proposal_cid: proposal.cid,
          voter: vote.voter,
          decision: vote.decision,
          reason: vote.reason,
        },
      });
    });
    
    // Completed proposal activity
    if (proposal.status === 'completed') {
      activities.push({
        activity_type: 'Proposal Completed',
        cid: generateCid(),
        timestamp: proposal.updated_at,
        actor: proposal.submitter,
        description: `Proposal completed: ${proposal.title}`,
        details: {
          proposal_id: proposal.id,
          proposal_cid: proposal.cid,
          proposal_title: proposal.title,
          outcome: proposal.votes.filter(v => v.decision === 'approve').length > 
                  proposal.votes.filter(v => v.decision === 'reject').length ? 'approved' : 'rejected',
        },
      });
    }
  });
  
  // Sort activities by timestamp (newest first)
  return activities.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
};

// Export functions to get seeded data
export const getSeededData = () => ({
  federation,
  cooperatives,
  communities,
  users,
  policy,
  proposals,
  dagNodes: generateDagNodes(),
  activities: generateActivityEvents(),
}); 