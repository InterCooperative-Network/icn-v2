use clap::{Args, Subcommand, ValueHint, ArgAction};
use std::path::PathBuf;
use std::fs::{self, File};
use std::io::BufReader;
use serde_json;
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::bundle::TrustBundle;
use icn_types::Cid;
use icn_types::anchor::AnchorRef;
use icn_types::QuorumProof;
use icn_identity_core::did::{Did, DidKey};
use chrono::{DateTime, Utc};

// Placeholder for imports that will be needed by handlers
// use icn_types::{TrustBundle, AnchorRef};
// use icn_core_types::{Cid, Did, QuorumProof};
// use icn_identity_core::did::DidKey;
// use std::fs;
// use serde_json;

#[derive(Subcommand, Debug, Clone)]
pub enum BundleCommands {
    /// Create a new TrustBundle locally from components.
    Create(CreateBundleArgs),
    /// Anchor a locally created TrustBundle to the DAG.
    Anchor(AnchorBundleArgs),
    /// Show details of an anchored TrustBundle from the DAG.
    Show(ShowBundleArgs),
    /// Verify an anchored TrustBundle from the DAG (proofs, anchors).
    Verify(VerifyBundleArgs),
    /// Export an anchored TrustBundle from the DAG to a local file.
    Export(ExportBundleArgs),
}

#[derive(Args, Debug, Clone)]
pub struct CreateBundleArgs {
    /// CID of the state data associated with this bundle.
    #[arg(long)]
    pub state_cid: String,

    /// Path to a JSON file containing the QuorumProof for the state.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub state_proof_file: PathBuf,

    /// Previous anchor references (format: "cid:object_type:timestamp_rfc3339" or "cid"). Can be specified multiple times.
    #[arg(long, value_delimiter = ',')]
    pub prev_anchors: Vec<String>,

    /// Optional path to a JSON file containing metadata for the bundle.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub metadata_file: Option<PathBuf>,

    /// Output file path to save the created TrustBundle (JSON format).
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct AnchorBundleArgs {
    /// Path to the local TrustBundle file (JSON format) to be anchored.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub bundle_file: PathBuf,

    /// Path to the JWK file for signing the anchor DAG node. If not provided, uses default key.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub key_file: Option<PathBuf>,
    
    /// DID of the author anchoring this bundle. If provided, it must match the DID of the key from key_file.
    #[arg(long)]
    pub author_did: Option<String>,

    /// Optional path to the DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,

    /// Optional output file path to save the resulting anchor CID.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ShowBundleArgs {
    /// CID of the anchored TrustBundle to show.
    #[arg(long)]
    pub cid: String,

    /// Optional path to the DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
    
    /// Show the raw anchor DAG node instead of the resolved TrustBundle content.
    #[arg(long, action = ArgAction::SetTrue)]
    pub raw_node: bool,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyBundleArgs {
    /// CID of the anchored TrustBundle to verify.
    #[arg(long)]
    pub cid: String,

    /// Optional path to the DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ExportBundleArgs {
    /// CID of the anchored TrustBundle to export.
    #[arg(long)]
    pub cid: String,

    /// Output file path to save the exported TrustBundle (JSON format).
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: PathBuf,

    /// Optional path to the DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

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

fn parse_anchor_ref_str(anchor_str: &str) -> Result<AnchorRef, CliError> {
    let parts: Vec<&str> = anchor_str.splitn(3, ':').collect();
    let cid_str = parts.get(0).ok_or_else(|| CliError::InvalidArgument(format!("Invalid anchor string (missing CID): {}", anchor_str)))?;
    let cid = Cid::try_from(*cid_str)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid CID '{}' in anchor string: {}", cid_str, e)))?;
    let object_type = parts.get(1).map(|s| s.to_string());
    let timestamp_str = parts.get(2);
    let timestamp = match timestamp_str {
        Some(ts_str) => DateTime::parse_from_rfc3339(ts_str)
            .map_err(|e| CliError::InvalidArgument(format!("Invalid timestamp format '{}' in anchor string (expected RFC3339): {}", ts_str, e)))?
            .with_timezone(&Utc),
        None => Utc::now(),
    };
    Ok(AnchorRef { cid, object_type, timestamp })
}

async fn handle_create_bundle(context: &mut CliContext, args: &CreateBundleArgs) -> CliResult {
    println!("Executing bundle create with args: {:?}", args);

    // 1. Parse state_cid
    let state_cid = Cid::try_from(args.state_cid.as_str())
        .map_err(|e| CliError::InvalidArgument(format!("Invalid state_cid format '{}': {}", args.state_cid, e)))?;

    // 2. Read and parse state_proof_file
    let proof_file = File::open(&args.state_proof_file)
        .map_err(|e| CliError::Io(e))?;
    let reader = BufReader::new(proof_file);
    let state_proof: QuorumProof = serde_json::from_reader(reader)
        .map_err(|e| CliError::Json(e))?;

    // 3. Parse prev_anchors
    let mut prev_anchors_vec = Vec::new();
    for anchor_str in &args.prev_anchors {
        prev_anchors_vec.push(parse_anchor_ref_str(anchor_str)?);
    }

    // 4. Optionally read metadata_file
    let metadata: Option<serde_json::Value> = match &args.metadata_file {
        Some(path) => {
            let meta_file = File::open(path).map_err(|e| CliError::Io(e))?;
            let meta_reader = BufReader::new(meta_file);
            Some(serde_json::from_reader(meta_reader).map_err(|e| CliError::Json(e))?)
        }
        None => None,
    };

    // 5. Call TrustBundle::new(...)
    let trust_bundle = TrustBundle::new(state_cid, state_proof, prev_anchors_vec, metadata);

    // 6. Write to args.output
    let output_file = File::create(&args.output).map_err(|e| CliError::Io(e))?;
    serde_json::to_writer_pretty(output_file, &trust_bundle)
        .map_err(|e| CliError::Json(e))?;

    println!("TrustBundle created successfully and saved to: {}", args.output.display());
    Ok(())
}

async fn handle_anchor_bundle(context: &mut CliContext, args: &AnchorBundleArgs) -> CliResult {
    if context.verbose {
        println!("Executing bundle anchor with args: {:?}", args);
    }

    // 1. Read & parse bundle
    let bundle_json_bytes = fs::read(&args.bundle_file)
        .map_err(|e| CliError::Io(e))?; // Simplified error mapping for now
    let bundle: TrustBundle = serde_json::from_slice(&bundle_json_bytes)
        .map_err(|e| CliError::Json(e))?; // Simplified error mapping

    // 2. Load key & optional check of author DID
    // CliContext::_get_key takes Option<&Path>. If args.key_file is None, it uses default.
    let signer_key = context._get_key(args.key_file.as_deref())?;

    if let Some(expected_author_did_str) = &args.author_did {
        let expected_author_did = Did::parse(expected_author_did_str)
            .map_err(|e| CliError::InvalidArgument(format!("Invalid author DID format '{}': {}", expected_author_did_str, e)))?;
        if signer_key.did() != &expected_author_did {
            return Err(CliError::InvalidArgument(
                format!("Provided author DID '{}' does not match signing key's DID '{}'", expected_author_did_str, signer_key.did())
            ));
        }
    }
    // The author for the DagNode will be signer_key.did()

    // 3. Get DAG store
    // Pass dag_dir from args to get_dag_store
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 4. Anchor bundle
    // The anchor_to_dag method from icn_types::bundle::TrustBundle needs to be used.
    // It requires the author DID (from signer_key) and the signing key itself.
    // The current signature of anchor_to_dag in icn-types is:
    // pub async fn anchor_to_dag(&self, author: Did, signing_key: &SigningKey, dag_store: &mut impl DagStore) -> Result<Cid, TrustBundleError>
    // DidKey provides access to SigningKey.

    let anchor_cid = bundle.anchor_to_dag(
        signer_key.did().clone(), 
        signer_key.signing_key(), // Assuming DidKey has a method to get the ed25519_dalek::SigningKey
        &mut *dag_store // Deref Arc and pass mutable reference
    ).await.map_err(|e| CliError::Other(Box::new(e)))?; // Map TrustBundleError to CliError

    // 5. Output CID
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
    // Note: get_dag_store now takes Option<&Path>, so we pass args.dag_dir.as_deref()
    let mut dag_store = context.get_dag_store(args.dag_dir.as_deref())?;

    // 3. Raw-node vs. resolved bundle
    let output_string = if args.raw_node {
        let node = dag_store.get_node(&anchor_cid).await
            .map_err(|e| CliError::Dag(e))?; // Assuming CliError::Dag can take DagError
        serde_json::to_string_pretty(&node)
            .map_err(|e| CliError::Json(e))?
    } else {
        // This relies on TrustBundle::from_dag being implemented correctly in icn-types
        let bundle = TrustBundle::from_dag(&anchor_cid, &mut *dag_store).await
            .map_err(|e| CliError::Other(Box::new(e)))?; // Map TrustBundleError
        serde_json::to_string_pretty(&bundle)
            .map_err(|e| CliError::Json(e))?
    };

    // 4. Output (to console only for show, as per current ShowBundleArgs)
    println!("{}", output_string);

    Ok(())
}

async fn handle_verify_bundle(_context: &mut CliContext, args: &VerifyBundleArgs) -> CliResult {
    println!("Executing bundle verify with args: {:?}", args);
    // TODO: Implement logic to get DAG store, call TrustBundle::from_dag, then TrustBundle::verify_anchors and verify proof.
    Err(CliError::Unimplemented("bundle verify".to_string()))
}

async fn handle_export_bundle(_context: &mut CliContext, args: &ExportBundleArgs) -> CliResult {
    println!("Executing bundle export with args: {:?}", args);
    // TODO: Implement logic to get DAG store, call TrustBundle::from_dag, serialize, and save.
    Err(CliError::Unimplemented("bundle export".to_string()))
} 