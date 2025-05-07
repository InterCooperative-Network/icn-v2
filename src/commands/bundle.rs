use clap::{Args, Subcommand, ValueHint, ArgAction};
use std::path::PathBuf;
use std::fs::{self, File};
use std::io::{self, BufReader};
use serde_json;
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::bundle::TrustBundle;
use icn_types::Cid;
use icn_types::anchor::AnchorRef;
use icn_types::QuorumProof;
use icn_types::governance::QuorumConfig;
use icn_types::dag::DagStore;
use icn_identity_core::did::DidKey;
use icn_core_types::did::Did;
use chrono::{DateTime, Utc};
use crate::context::MutableDagStore;
use std::error::Error as StdError;
use serde_ipld_dagcbor;
use std::sync::Arc;

// ... Rest of existing code ...

pub async fn handle_bundle_command(
    context: &mut CliContext,
    cmd: &BundleCommands,
) -> CliResult {
    if context.verbose {
        println!("Handling Bundle command: {:?}", cmd);
    }
    match cmd {
        BundleCommands::Create(args) => handle_create_bundle(context, args).await,
        BundleCommands::Anchor(args) => handle_anchor_bundle(context, args).await,
        BundleCommands::Show(args) => handle_show_bundle(context, args).await,
        BundleCommands::Verify(args) => handle_verify_bundle(context, args).await,
        BundleCommands::Export(args) => handle_export_bundle(context, args).await,
    }
}

async fn handle_anchor_bundle(context: &mut CliContext, args: &AnchorBundleArgs) -> CliResult {
    if context.verbose {
        println!("Executing bundle anchor with args: {:?}", args);
    }

    // 1. Read & parse bundle
    let bundle_json_bytes = fs::read(&args.bundle_file)
        .map_err(|e| CliError::Io(e))?;
    let bundle: TrustBundle = serde_json::from_slice(&bundle_json_bytes)
        .map_err(|e| CliError::Json(e))?;

    // 2. Load key & optional check of author DID
    let signer_key = context._get_key(args.key_file.as_deref())?;

    if let Some(expected_author_did_str) = &args.author_did {
        let expected_author_did = Did::from_string(expected_author_did_str)
            .map_err(|e| CliError::InvalidArgument(format!("Invalid author DID format '{}': {}", expected_author_did_str, e)))?;
        if signer_key.did() != &expected_author_did {
            return Err(CliError::InvalidArgument(
                format!("Provided author DID '{}' does not match signing key's DID '{}'", expected_author_did_str, signer_key.did())
            ));
        }
    }

    // 3. Get DAG store
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 4. Create a DagNode for the anchor and sign it
    let node = bundle.to_dag_node(signer_key.did().clone())
        .map_err(|e| CliError::Other(Box::new(e)))?;
    
    // 5. Sign the node with our key
    let node_bytes = serde_ipld_dagcbor::to_vec(&node)
        .map_err(|e| CliError::SerializationError(e.to_string()))?;
    let signature = signer_key.signing_key().sign(&node_bytes);
    
    let signed_node = icn_types::dag::SignedDagNode {
        node,
        signature,
        cid: None
    };
    
    // 6. Add the node to the DAG store
    let anchor_cid = dag_store.add_node(signed_node).await
        .map_err(|e| CliError::Dag(e))?;

    // 7. Output CID
    let cid_str = anchor_cid.to_string();
    if let Some(output_path) = &args.output {
        fs::write(output_path, cid_str.as_bytes())
            .map_err(|e| CliError::Io(e))?;
        println!("TrustBundle anchored successfully. Anchor CID: {}", cid_str);
        println!("CID saved to: {}", output_path.display());
    } else {
        println!("TrustBundle anchored successfully. Anchor CID: {}", cid_str);
    }

    Ok(())
}

async fn handle_show_bundle(context: &mut CliContext, args: &ShowBundleArgs) -> CliResult {
    if context.verbose {
        println!("Executing bundle show for CID: {}, raw_node: {}", args.cid, args.raw_node);
    }

    // 1. Parse the CID
    let anchor_cid = Cid::try_from(args.cid.as_str())
        .map_err(|e| CliError::InvalidArgument(format!("Invalid anchor CID format '{}': {}", args.cid, e)))?;

    // 2. Open the DAG store
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 3. Raw-node vs. resolved bundle
    let output_string = if args.raw_node {
        let node = dag_store.get_node(&anchor_cid).await
            .map_err(|e| CliError::Dag(e))?;
        serde_json::to_string_pretty(&node)
            .map_err(|e| CliError::Json(e))?
    } else {
        // Use TrustBundle::from_dag directly
        let bundle = TrustBundle::from_dag(&anchor_cid, &mut dag_store).await
            .map_err(|e| CliError::Other(Box::new(e)))?;
        serde_json::to_string_pretty(&bundle)
            .map_err(|e| CliError::Json(e))?
    };

    // 4. Output (to console only for show, as per current ShowBundleArgs)
    println!("{}", output_string);

    Ok(())
}

async fn handle_verify_bundle(context: &mut CliContext, args: &VerifyBundleArgs) -> CliResult {
    if context.verbose {
        println!("Verifying bundle with args: {:?}", args);
    }

    // 1. Parse Anchor CID
    let anchor_cid = Cid::try_from(args.cid.as_str())
        .map_err(|e| CliError::InvalidArgument(format!("Invalid anchor CID '{}': {}", args.cid, e)))?;

    // 2. Load QuorumConfig file (now mandatory)
    let quorum_config_file = File::open(&args.quorum_config)
        .map_err(|e| CliError::Io(e))?;
    let reader = BufReader::new(quorum_config_file);
    let quorum_cfg: QuorumConfig = serde_json::from_reader(reader)
        .map_err(|e| CliError::Json(e))?;

    // 3. Open DAG store
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 4. Load the bundle with TrustBundle::from_dag
    if context.verbose {
        println!("Attempting to load bundle {} from DAG...", anchor_cid);
    }
    
    let bundle = TrustBundle::from_dag(&anchor_cid, &mut dag_store).await
        .map_err(|e| CliError::Other(Box::new(e)))?;
    
    if context.verbose {
        println!("Bundle loaded successfully.");
        println!("Verifying bundle against quorum config...");
    }

    // 5. Run verification
    // Use the inner Arc<dyn DagStore> directly since that's what verify expects
    match bundle.verify(&*dag_store.inner, &quorum_cfg).await {
        Ok(_) => {
            println!("✅ Bundle {} verified successfully.", anchor_cid);
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Bundle {} verification failed: {}", anchor_cid, e);
            Err(CliError::Other(Box::new(e))) 
        }
    }
}

async fn handle_export_bundle(context: &mut CliContext, args: &ExportBundleArgs) -> CliResult {
    if context.verbose {
        println!("Exporting bundle with args: {:?}", args);
    }

    // 1. Parse Anchor CID
    let anchor_cid = Cid::try_from(args.cid.as_str())
        .map_err(|e| CliError::InvalidArgument(format!("Invalid anchor CID '{}': {}", args.cid, e)))?;

    // 2. Open DAG store
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 3. Load the bundle with TrustBundle::from_dag
    if context.verbose {
        println!("Attempting to load bundle {} from DAG for export...", anchor_cid);
    }
    let bundle = TrustBundle::from_dag(&anchor_cid, &mut dag_store).await
        .map_err(|e| CliError::Other(Box::new(e)))?;

    // 4. Serialize to pretty JSON
    let json_output = serde_json::to_string_pretty(&bundle)
        .map_err(|e| CliError::Json(e))?;

    // 5. Write to file or stdout
    if let Some(output_path) = &args.output {
        fs::write(output_path, json_output.as_bytes())
            .map_err(|e| CliError::Io(e))?;
        if context.verbose {
            println!("Bundle {} exported successfully to {}.", anchor_cid, output_path.display());
        }
        println!("Exported bundle {} to {}.", anchor_cid, output_path.display());
    } else {
        println!("{}", json_output);
    }

    Ok(())
} 