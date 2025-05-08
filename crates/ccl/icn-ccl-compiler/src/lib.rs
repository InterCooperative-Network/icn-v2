#![deny(unsafe_code)]

use icn_ccl_parser::{parse_ccl, CclModule, CclStmt};
use sha2::{Digest, Sha256};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, Function, FunctionSection, ImportSection, Instruction, MemorySection, MemoryType, Module,
    TypeSection,
};

/// Order must match `icn-runtime` host-function registration.
/// Signatures based on user proposal:
/// 0: host_log_message(ptr: i32, len: i32)
/// 1: host_anchor_to_dag(bytes: i32) -> For now, will pass ptr/len for consistency if logging CID
/// 2: host_check_resource_authorization(token_ptr: i32, token_len: i32, amount: i64) -> i32
/// 3: host_record_resource_usage(token_ptr: i32, token_len: i32, amount: i64)
const ABI: &[(&str, &str)] = &[
    ("icn", "host_log_message"),                 // 0
    ("icn", "host_anchor_to_dag"),               // 1
    ("icn", "host_check_resource_authorization"),// 2
    ("icn", "host_record_resource_usage"),       // 3
];

/// Returned by `compile()` – deterministic Wasm plus its SHA-256 hex digest.
#[derive(Debug)]
pub struct WasmArtifact {
    pub wasm:     Vec<u8>,
    pub hash_hex: String,
}

// Helper for string constants in the Wasm data section
struct StringPool {
    data: DataSection,
    offset: u32, 
    active_segment_data: Vec<u8>, 
}

impl StringPool {
    fn new() -> Self {
        Self {
            data: DataSection::new(),
            offset: 0,
            active_segment_data: Vec::new(),
        }
    }

    fn intern(&mut self, s: &str) -> (u32, u32) {
        let ptr = self.offset;
        let bytes = s.as_bytes();
        self.active_segment_data.extend_from_slice(bytes);
        self.offset += bytes.len() as u32;
        (ptr, bytes.len() as u32)
    }

    fn finalize_segment(&mut self) {
        if !self.active_segment_data.is_empty() {
            self.data.active(
                0, // Memory index 0 (as u32 now)
                &ConstExpr::i32_const(0), // Offset where this segment starts in memory
                self.active_segment_data.drain(..).collect::<Vec<u8>>(), 
            );
            // self.offset = self.active_segment_data.len() as u32; // Not needed after drain
        }
    }
}

pub fn compile(source: &str, caller_scope: &str) -> Result<WasmArtifact, CompileError> {
    let ast = parse_ccl(source)?;
    let wasm_bytes = lower_to_wasm(&ast, caller_scope)?;
    let hash_hex = format!("{:x}", Sha256::digest(&wasm_bytes));
    Ok(WasmArtifact {
        wasm: wasm_bytes,
        hash_hex,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// lowering: AST ➜ minimal Wasm module
// ──────────────────────────────────────────────────────────────────────────────
fn lower_to_wasm(ast: &CclModule, _scope: &str) -> Result<Vec<u8>, CompileError> {
    let mut module = Module::new();
    let mut string_pool = StringPool::new();

    // ---------- section: types ------------------------------------------------
    let mut types = TypeSection::new();
    // Type 0: () -> ()
    let type_idx_void_void = types.len();
    types.function([], []);
    // Type 1: (ptr: i32, len: i32) -> ()
    let type_idx_ptr_len_void = types.len();
    types.function([wasm_encoder::ValType::I32, wasm_encoder::ValType::I32], []);
    // Type 2: (ptr: i32, len: i32, amount: i64) -> i32
    let type_idx_ptr_len_i64_i32 = types.len();
    types.function([wasm_encoder::ValType::I32, wasm_encoder::ValType::I32, wasm_encoder::ValType::I64], [wasm_encoder::ValType::I32]);
    // Type 3: (val: i32) -> ()  (For host_anchor_to_dag if it takes bytes directly)
    let type_idx_i32_void = types.len();
    types.function([wasm_encoder::ValType::I32], []);
     // Type 4: (ptr: i32, len: i32, amount: i64) -> () (for record_resource_usage, assuming no return needed for Call)
    let type_idx_ptr_len_i64_void = types.len();
    types.function([wasm_encoder::ValType::I32, wasm_encoder::ValType::I32, wasm_encoder::ValType::I64], []);


    module.section(&types);

    // ---------- section: imports ---------------------------------------------
    let mut imports = ImportSection::new();
    // ABI: (module, name, type_index_for_signature)
    imports.import(ABI[0].0, ABI[0].1, EntityType::Function(type_idx_ptr_len_void)); // host_log_message
    imports.import(ABI[1].0, ABI[1].1, EntityType::Function(type_idx_i32_void));   // host_anchor_to_dag (assuming i32 bytes for now)
    imports.import(ABI[2].0, ABI[2].1, EntityType::Function(type_idx_ptr_len_i64_i32)); // host_check_resource_authorization
    imports.import(ABI[3].0, ABI[3].1, EntityType::Function(type_idx_ptr_len_i64_void)); // host_record_resource_usage
    module.section(&imports);

    // ---------- section: functions ---------------------------------------------
    let mut func_sec = FunctionSection::new();
    // Create placeholder for function bodies that will be defined in the Code section
    for _stmt in &ast.stmts {
        func_sec.function(type_idx_void_void); 
    }
    module.section(&func_sec); // Moved func_sec here

    // ---------- section: memory ----------------------------------------------
    let mut mem_sec = MemorySection::new();
    mem_sec.memory(MemoryType {
        minimum: 1, 
        maximum: None,
        memory64: false,
        shared: false,
    });
    module.section(&mem_sec);
    
    // ---------- section: code ------------------------------------------------
    let mut code_sec = CodeSection::new();
    for stmt in &ast.stmts {
        let mut f = Function::new(vec![]); 
        match stmt {
            CclStmt::PerformMeteredAction { resource, amount } => {
                let log_msg = format!("PerformMeteredAction for resource: {}", resource);
                let (msg_ptr, msg_len) = string_pool.intern(&log_msg);
                f.instruction(&Instruction::I32Const(msg_ptr as i32));
                f.instruction(&Instruction::I32Const(msg_len as i32));
                f.instruction(&Instruction::Call(0)); 

                let (tok_ptr, tok_len) = string_pool.intern(resource);
                f.instruction(&Instruction::I32Const(tok_ptr as i32));
                f.instruction(&Instruction::I32Const(tok_len as i32));
                f.instruction(&Instruction::I64Const(*amount as i64));
                f.instruction(&Instruction::Call(2)); 
                f.instruction(&Instruction::Drop);    

                f.instruction(&Instruction::I32Const(tok_ptr as i32)); 
                f.instruction(&Instruction::I32Const(tok_len as i32));
                f.instruction(&Instruction::I64Const(*amount as i64));
                f.instruction(&Instruction::Call(3)); 
            }
            CclStmt::MintToken { token, amount } => {
                let log_msg = format!("MintToken: {} amount: {}", token, amount);
                let (msg_ptr, msg_len) = string_pool.intern(&log_msg);
                f.instruction(&Instruction::I32Const(msg_ptr as i32));
                f.instruction(&Instruction::I32Const(msg_len as i32));
                f.instruction(&Instruction::Call(0)); 
            }
            CclStmt::TransferResource { token, to, amount } => {
                let log_msg = format!("TransferResource: {} to {} amount: {}", token, to, amount);
                let (msg_ptr, msg_len) = string_pool.intern(&log_msg);
                f.instruction(&Instruction::I32Const(msg_ptr as i32));
                f.instruction(&Instruction::I32Const(msg_len as i32));
                f.instruction(&Instruction::Call(0));
            }
            CclStmt::AnchorData { cid, bytes } => {
                let log_msg = format!("AnchorData for cid: {}", cid);
                let (msg_ptr, msg_len) = string_pool.intern(&log_msg);
                f.instruction(&Instruction::I32Const(msg_ptr as i32));
                f.instruction(&Instruction::I32Const(msg_len as i32));
                f.instruction(&Instruction::Call(0)); 

                f.instruction(&Instruction::I32Const(*bytes as i32)); 
                f.instruction(&Instruction::Call(1)); 
            }
        }
        f.instruction(&Instruction::End);
        code_sec.function(&f);
    }
    module.section(&code_sec); // Moved code_sec here

    // ---------- section: data ------------------------------------------------
    string_pool.finalize_segment();
    module.section(&string_pool.data);

    Ok(module.finish())
}

// ──────────────────────────────────────────────────────────────────────────────
#[derive(thiserror::Error, Debug)]
pub enum CompileError {
    #[error(transparent)]
    Parse(#[from] icn_ccl_parser::CclError),
    #[error("lowering error: {0}")]
    Lowering(String),
} 