use std::any::Any;
use std::sync::Arc;

pub type KayRawHandle = u64;
pub type KayContextId = u64;
pub type KayHostSlot = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KayStatus {
    Ok,
    Err,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KayErrorCode {
    GeneralFailure,
    TypeMismatch,
    NotFound,
    AlreadyExists,
    InvalidArgument,
    Panic,
}

#[derive(Debug, Clone)]
pub struct KayError {
    pub code: KayErrorCode,
    pub message: Option<String>,
}

impl KayError {
    pub fn new(code: KayErrorCode, message: impl Into<Option<String>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type KayAbiResult<T> = Result<T, KayError>;

#[derive(Debug, Clone, Copy)]
pub struct KayContext {
    pub id: KayContextId,
    pub vtable: &'static KayContextVTable,
}

#[derive(Debug, Clone)]
pub struct KayValueInfo {
    pub kind: KayValueKind,
}

#[derive(Debug, Clone)]
pub enum KayValueKind {
    Int(i64),
    Bool(bool),
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Unit,
    Capsule { tag: &'static str },
}

#[derive(Debug, Clone)]
pub struct KayCapsuleSpec {
    pub tag: &'static str,
    pub payload: Arc<dyn Any + Send + Sync>,
}

#[derive(Debug, Clone)]
pub struct KayCapsuleData {
    pub tag: &'static str,
    pub payload: Arc<dyn Any + Send + Sync>,
}

#[derive(Debug)]
pub struct KayContextVTable {
    pub alloc_int: fn(KayContextId, i64) -> KayAbiResult<KayRawHandle>,
    pub alloc_bool: fn(KayContextId, bool) -> KayAbiResult<KayRawHandle>,
    pub alloc_string: fn(KayContextId, Arc<str>) -> KayAbiResult<KayRawHandle>,
    pub alloc_bytes: fn(KayContextId, Arc<[u8]>) -> KayAbiResult<KayRawHandle>,
    pub alloc_unit: fn(KayContextId) -> KayAbiResult<KayRawHandle>,
    pub inc_ref: fn(KayContextId, KayRawHandle) -> KayAbiResult<()>,
    pub dec_ref: fn(KayContextId, KayRawHandle) -> KayAbiResult<()>,
    pub inspect: fn(KayContextId, KayRawHandle) -> KayAbiResult<KayValueInfo>,
    pub call_host: fn(KayContextId, KayHostSlot, Vec<KayRawHandle>) -> KayAbiResult<KayRawHandle>,
    pub call_host_dynamic:
        fn(KayContextId, String, Vec<KayRawHandle>) -> KayAbiResult<KayRawHandle>,
    pub new_capsule: fn(KayContextId, KayCapsuleSpec) -> KayAbiResult<KayRawHandle>,
    pub capsule_data: fn(KayContextId, KayRawHandle) -> KayAbiResult<KayCapsuleData>,
}
