use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use kayton_abi::{
    KayAbiResult, KayCapsuleData, KayCapsuleSpec, KayContext, KayContextId, KayContextVTable,
    KayError, KayErrorCode, KayHostSlot, KayRawHandle, KayValueInfo, KayValueKind,
};
use kayton_api::{KayCtx, KayExtension, KayResult};
use thiserror::Error;

static CONTEXTS: OnceLock<Mutex<HashMap<KayContextId, Arc<ContextInner>>>> = OnceLock::new();
static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

fn contexts() -> &'static Mutex<HashMap<KayContextId, Arc<ContextInner>>> {
    CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_context<T>(
    id: KayContextId,
    f: impl FnOnce(Arc<ContextInner>) -> KayAbiResult<T>,
) -> KayAbiResult<T> {
    let ctx_arc = {
        let guard = contexts().lock().unwrap();
        guard
            .get(&id)
            .cloned()
            .ok_or_else(|| error(KayErrorCode::NotFound, "context not found"))?
    };
    f(ctx_arc)
}

fn error(code: KayErrorCode, message: &str) -> KayError {
    KayError::new(code, Some(message.to_string()))
}

#[derive(Debug, Error)]
pub enum HostError {
    #[error("unknown handle {0}")]
    UnknownHandle(KayRawHandle),
    #[error("extension `{0}` already registered")]
    DuplicateExtension(String),
    #[error("extension slot {0} not found")]
    UnknownSlot(KayHostSlot),
}

impl From<HostError> for KayError {
    fn from(value: HostError) -> Self {
        match value {
            HostError::UnknownHandle(handle) => error(
                KayErrorCode::NotFound,
                &format!("handle {handle} not found"),
            ),
            HostError::DuplicateExtension(name) => {
                KayError::new(KayErrorCode::AlreadyExists, Some(name))
            }
            HostError::UnknownSlot(slot) => {
                error(KayErrorCode::NotFound, &format!("slot {slot} not found"))
            }
        }
    }
}

#[derive(Default)]
struct ContextInner {
    handles: Mutex<HashMap<KayRawHandle, HandleEntry>>,
    next_handle: AtomicU64,
    extensions: Mutex<Vec<KayExtension>>,
    name_to_slot: Mutex<HashMap<String, KayHostSlot>>,
}

impl ContextInner {
    fn alloc_value(&self, value: StoredValue) -> KayAbiResult<KayRawHandle> {
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        let mut handles = self.handles.lock().unwrap();
        handles.insert(handle, HandleEntry { value, refs: 1 });
        Ok(handle)
    }

    fn inc_ref(&self, handle: KayRawHandle) -> KayAbiResult<()> {
        let mut handles = self.handles.lock().unwrap();
        let entry = handles
            .get_mut(&handle)
            .ok_or_else(|| error(KayErrorCode::NotFound, "handle not found"))?;
        entry.refs += 1;
        Ok(())
    }

    fn dec_ref(&self, handle: KayRawHandle) -> KayAbiResult<()> {
        let mut handles = self.handles.lock().unwrap();
        let entry = handles
            .get_mut(&handle)
            .ok_or_else(|| error(KayErrorCode::NotFound, "handle not found"))?;
        if entry.refs == 0 {
            return Err(error(
                KayErrorCode::GeneralFailure,
                "invalid refcount state",
            ));
        }
        entry.refs -= 1;
        if entry.refs == 0 {
            handles.remove(&handle);
        }
        Ok(())
    }

    fn inspect(&self, handle: KayRawHandle) -> KayAbiResult<KayValueInfo> {
        let handles = self.handles.lock().unwrap();
        let entry = handles
            .get(&handle)
            .ok_or_else(|| error(KayErrorCode::NotFound, "handle not found"))?;
        Ok(KayValueInfo {
            kind: entry.value.describe(),
        })
    }

    fn capsule_data(&self, handle: KayRawHandle) -> KayAbiResult<KayCapsuleData> {
        let handles = self.handles.lock().unwrap();
        let entry = handles
            .get(&handle)
            .ok_or_else(|| error(KayErrorCode::NotFound, "handle not found"))?;
        match &entry.value {
            StoredValue::Capsule { tag, payload } => Ok(KayCapsuleData {
                tag,
                payload: Arc::clone(payload),
            }),
            _ => Err(error(KayErrorCode::TypeMismatch, "value is not a capsule")),
        }
    }

    fn register_extension(&self, extension: KayExtension) -> KayAbiResult<KayHostSlot> {
        let mut names = self.name_to_slot.lock().unwrap();
        if names.contains_key(extension.name) {
            return Err(HostError::DuplicateExtension(extension.name.to_string()).into());
        }
        let mut exts = self.extensions.lock().unwrap();
        let slot = exts.len() as KayHostSlot;
        exts.push(extension);
        names.insert(extension.name.to_string(), slot);
        Ok(slot)
    }

    fn extension_by_slot(&self, slot: KayHostSlot) -> KayAbiResult<KayExtension> {
        let exts = self.extensions.lock().unwrap();
        exts.get(slot as usize)
            .copied()
            .ok_or_else(|| HostError::UnknownSlot(slot).into())
    }

    fn extension_by_name(&self, name: &str) -> KayAbiResult<(KayHostSlot, KayExtension)> {
        let names = self.name_to_slot.lock().unwrap();
        let slot = *names
            .get(name)
            .ok_or_else(|| error(KayErrorCode::NotFound, "extension not found"))?;
        drop(names);
        let extension = self.extension_by_slot(slot)?;
        Ok((slot, extension))
    }
}

#[derive(Clone)]
struct HandleEntry {
    value: StoredValue,
    refs: usize,
}

#[derive(Clone)]
enum StoredValue {
    Int(i64),
    Bool(bool),
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Unit,
    Capsule {
        tag: &'static str,
        payload: Arc<dyn std::any::Any + Send + Sync>,
    },
}

impl StoredValue {
    fn describe(&self) -> KayValueKind {
        match self {
            StoredValue::Int(value) => KayValueKind::Int(*value),
            StoredValue::Bool(value) => KayValueKind::Bool(*value),
            StoredValue::String(value) => KayValueKind::String(value.clone()),
            StoredValue::Bytes(value) => KayValueKind::Bytes(value.clone()),
            StoredValue::Unit => KayValueKind::Unit,
            StoredValue::Capsule { tag, .. } => KayValueKind::Capsule { tag },
        }
    }
}

fn alloc_int(id: KayContextId, value: i64) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| ctx.alloc_value(StoredValue::Int(value)))
}

fn alloc_bool(id: KayContextId, value: bool) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| ctx.alloc_value(StoredValue::Bool(value)))
}

fn alloc_string(id: KayContextId, value: Arc<str>) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| ctx.alloc_value(StoredValue::String(value)))
}

fn alloc_bytes(id: KayContextId, value: Arc<[u8]>) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| ctx.alloc_value(StoredValue::Bytes(value)))
}

fn alloc_unit(id: KayContextId) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| ctx.alloc_value(StoredValue::Unit))
}

fn inc_ref(id: KayContextId, handle: KayRawHandle) -> KayAbiResult<()> {
    with_context(id, |ctx| ctx.inc_ref(handle))
}

fn dec_ref(id: KayContextId, handle: KayRawHandle) -> KayAbiResult<()> {
    with_context(id, |ctx| ctx.dec_ref(handle))
}

fn inspect(id: KayContextId, handle: KayRawHandle) -> KayAbiResult<KayValueInfo> {
    with_context(id, |ctx| ctx.inspect(handle))
}

fn call_host(
    id: KayContextId,
    slot: KayHostSlot,
    args: Vec<KayRawHandle>,
) -> KayAbiResult<KayRawHandle> {
    with_context(id, move |ctx| {
        let extension = ctx.extension_by_slot(slot)?;
        invoke_extension(id, ctx, extension, args)
    })
}

fn call_host_dynamic(
    id: KayContextId,
    name: String,
    args: Vec<KayRawHandle>,
) -> KayAbiResult<KayRawHandle> {
    with_context(id, move |ctx| {
        let (_, extension) = ctx.extension_by_name(&name)?;
        invoke_extension(id, ctx, extension, args)
    })
}

fn new_capsule(id: KayContextId, spec: KayCapsuleSpec) -> KayAbiResult<KayRawHandle> {
    with_context(id, |ctx| {
        ctx.alloc_value(StoredValue::Capsule {
            tag: spec.tag,
            payload: spec.payload,
        })
    })
}

fn capsule_data(id: KayContextId, handle: KayRawHandle) -> KayAbiResult<KayCapsuleData> {
    with_context(id, |ctx| ctx.capsule_data(handle))
}

fn invoke_extension(
    id: KayContextId,
    _ctx: Arc<ContextInner>,
    extension: KayExtension,
    args: Vec<KayRawHandle>,
) -> KayAbiResult<KayRawHandle> {
    let api_ctx = KayCtx::from_raw(KayContext {
        id,
        vtable: &VTABLE,
    });
    let mut handles = Vec::with_capacity(args.len());
    for raw in args {
        let handle = api_ctx.clone_raw(raw)?;
        handles.push(handle);
    }
    let result = extension.call(&api_ctx, &handles)?;
    let raw = result.raw();
    std::mem::forget(result);
    Ok(raw)
}

static VTABLE: KayContextVTable = KayContextVTable {
    alloc_int,
    alloc_bool,
    alloc_string,
    alloc_bytes,
    alloc_unit,
    inc_ref,
    dec_ref,
    inspect,
    call_host,
    call_host_dynamic,
    new_capsule,
    capsule_data,
};

pub struct KayHost {
    context: KayContext,
}

impl Default for KayHost {
    fn default() -> Self {
        Self::new()
    }
}

impl KayHost {
    pub fn new() -> Self {
        let id = NEXT_CONTEXT_ID.fetch_add(1, Ordering::SeqCst);
        let inner = Arc::new(ContextInner::default());
        contexts().lock().unwrap().insert(id, inner);
        Self {
            context: KayContext {
                id,
                vtable: &VTABLE,
            },
        }
    }

    pub fn context(&self) -> KayContext {
        self.context
    }

    pub fn api_ctx(&self) -> KayCtx {
        KayCtx::from_raw(self.context)
    }

    pub fn register_extension(&self, extension: KayExtension) -> KayResult<KayHostSlot> {
        with_context(self.context.id, |ctx| ctx.register_extension(extension))
    }

    pub fn register_extensions(&self, extensions: &[KayExtension]) -> KayResult<()> {
        for extension in extensions {
            self.register_extension(*extension)?;
        }
        Ok(())
    }

    pub fn resolve(&self, name: &str) -> Option<KayHostSlot> {
        with_context(self.context.id, |ctx| {
            let names = ctx.name_to_slot.lock().unwrap();
            Ok(names.get(name).copied())
        })
        .ok()
        .flatten()
    }
}

impl Drop for KayHost {
    fn drop(&mut self) {
        if let Some(map) = CONTEXTS.get() {
            if let Ok(mut guard) = map.lock() {
                guard.remove(&self.context.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kayton_api::{FromKay, KayHandle, ToKay};

    fn sample_extension() -> KayExtension {
        fn add_one(ctx: &KayCtx, args: &[KayHandle]) -> KayResult<KayHandle> {
            let value = i64::from_kay(ctx, &args[0])?;
            (value + 1).to_kay(ctx)
        }
        KayExtension::new("test.add_one", add_one, 1, Some(1), "adds one")
    }

    #[test]
    fn registers_and_calls_extension() {
        let host = KayHost::new();
        host.register_extension(sample_extension())
            .expect("register");
        let ctx = host.api_ctx();
        let input = 41_i64.to_kay(&ctx).expect("alloc");
        let raw = call_host(ctx.raw().id, 0, vec![input.raw()]).expect("call");
        let result = ctx.clone_raw(raw).expect("clone");
        let value = i64::from_kay(&ctx, &result).expect("from_kay");
        assert_eq!(value, 42);
    }
}
