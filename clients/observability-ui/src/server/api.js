const express = require('express');
const { exec } = require('child_process');
const cors = require('cors');
const app = express();
const port = process.env.PORT || 3001;

// Enable CORS for all routes
app.use(cors());
app.use(express.json());

// Helper function to execute ICN CLI commands
const executeIcnCommand = (command) => {
  return new Promise((resolve, reject) => {
    console.log(`Executing command: ${command}`);
    exec(command, (error, stdout, stderr) => {
      if (error) {
        console.error(`Error executing command: ${error.message}`);
        return reject(error);
      }
      if (stderr) {
        console.error(`Command stderr: ${stderr}`);
      }
      try {
        // Try to parse the output as JSON
        const result = JSON.parse(stdout);
        resolve(result);
      } catch (e) {
        // If not JSON, return the raw output
        resolve({ raw: stdout });
      }
    });
  });
};

// API Routes

// DAG View
app.get('/api/dag-view', async (req, res) => {
  try {
    const { scope_type, scope_id, limit = 50, output = 'json' } = req.query;
    const command = `icn observe dag-view --scope-type ${scope_type} --scope-id ${scope_id} --limit ${limit} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Policy Inspector
app.get('/api/inspect-policy', async (req, res) => {
  try {
    const { scope_type, scope_id, output = 'json' } = req.query;
    const command = `icn observe inspect-policy --scope-type ${scope_type} --scope-id ${scope_id} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Quorum Validator
app.get('/api/validate-quorum', async (req, res) => {
  try {
    const { cid, show_signers = true, output = 'json' } = req.query;
    const signerFlag = show_signers === 'true' || show_signers === true ? '--show-signers' : '';
    const command = `icn observe validate-quorum --cid ${cid} ${signerFlag} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Activity Log
app.get('/api/activity-log', async (req, res) => {
  try {
    const { scope_type, scope_id, limit = 50, output = 'json' } = req.query;
    const command = `icn observe activity-log --scope-type ${scope_type} --scope-id ${scope_id} --limit ${limit} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Federation Overview
app.get('/api/federation-overview', async (req, res) => {
  try {
    const { federation_id, output = 'json' } = req.query;
    const command = `icn observe federation-overview --federation-id ${federation_id} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Submit Proposal
app.post('/api/submit-proposal', async (req, res) => {
  try {
    const { 
      key_file,
      federation,
      title,
      description,
      proposal_type = 'textProposal',
      voting_threshold = 'majority',
      voting_duration = '86400',
      execution_cid,
      thread_cid,
      parameters,
      scope_type,
      scope_id
    } = req.body;

    if (!key_file || !federation || !title || !description) {
      return res.status(400).json({ 
        error: 'Missing required fields: key_file, federation, title, and description are required' 
      });
    }

    // Build command based on parameters
    let command = `icn proposal submit --key-file ${key_file} --federation ${federation} --title "${title}" ` +
      `--description "${description}" --proposal-type ${proposal_type} ` +
      `--voting-threshold ${voting_threshold} --voting-duration ${voting_duration}`;

    // Add optional parameters
    if (execution_cid) command += ` --execution-cid ${execution_cid}`;
    if (thread_cid) command += ` --thread-cid ${thread_cid}`;
    if (parameters) command += ` --parameters '${parameters}'`;
    if (scope_type && scope_id) {
      // For scoped proposals
      command += ` --scope-type ${scope_type} --scope-id ${scope_id}`;
    }

    // Execute the command
    const result = await executeIcnCommand(command);
    
    // Format the response
    res.json({
      proposal_id: result.id || result.proposal_id || 'unknown',
      cid: result.cid || 'unknown',
      title: title,
      submitter: result.submitter || 'unknown',
      status: result.status || 'draft',
      message: 'Proposal submitted successfully'
    });
  } catch (error) {
    console.error('Error submitting proposal:', error);
    res.status(500).json({ error: error.message });
  }
});

// Get Proposals List
app.get('/api/proposals', async (req, res) => {
  try {
    const { scope_type, scope_id, status, limit = 20, output = 'json' } = req.query;
    
    let command = `icn proposal list --output ${output}`;
    
    // Add optional parameters
    if (scope_type) command += ` --scope-type ${scope_type}`;
    if (scope_id) command += ` --scope-id ${scope_id}`;
    if (status) command += ` --status ${status}`;
    if (limit) command += ` --limit ${limit}`;
    
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Get Proposal Details
app.get('/api/proposal-details', async (req, res) => {
  try {
    const { proposal_id, output = 'json' } = req.query;
    
    if (!proposal_id) {
      return res.status(400).json({ error: 'Missing required parameter: proposal_id' });
    }
    
    const command = `icn proposal show ${proposal_id} --output ${output}`;
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Vote on Proposal
app.post('/api/vote', async (req, res) => {
  try {
    const { proposal_id, key_file, decision, reason } = req.body;
    
    if (!proposal_id || !key_file || !decision) {
      return res.status(400).json({ 
        error: 'Missing required fields: proposal_id, key_file, and decision are required' 
      });
    }
    
    let command = `icn proposal vote ${proposal_id} --key-file ${key_file} --decision ${decision}`;
    
    if (reason) {
      command += ` --reason "${reason}"`;
    }
    
    const result = await executeIcnCommand(command);
    res.json(result);
  } catch (error) {
    console.error('Error voting on proposal:', error);
    res.status(500).json({ error: error.message });
  }
});

// Start the server
app.listen(port, () => {
  console.log(`ICN Observability API server running at http://localhost:${port}`);
});

module.exports = app; // For testing purposes 