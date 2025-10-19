use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use kayton_abi::{
    KayAbiResult, KayCapsuleData, KayCapsuleSpec, KayContext, KayHostSlot, KayRawHandle,
    KayValueInfo,
};
use thiserror::Error;

pub type KayResult<T> = Result<T, KayError>;

pub use kayton_abi::{
    KayError, KayError as KayAbiError, KayErrorCode, KayErrorCode as KayErrorCodeAbi, KayValueKind,
    KayValueKind as KayValueKindAbi,
};

#[derive(Debug, Error)]
pub enum KayApiError {
    #[error("type mismatch: expected {expected}, found {found:?}")]
    TypeMismatch {
        expected: &'static str,
        found: KayValueKind,
    },
    #[error("capsule tag mismatch: expected {expected}, found {found}")]
    CapsuleTagMismatch {
        expected: &'static str,
        found: &'static str,
    },
}

impl From<KayApiError> for KayError {
    fn from(value: KayApiError) -> Self {
        match value {
            KayApiError::TypeMismatch { expected, found } => KayError::new(
                KayErrorCode::TypeMismatch,
                format!("expected {expected}, found {found:?}"),
            ),
            KayApiError::CapsuleTagMismatch { expected, found } => KayError::new(
                KayErrorCode::TypeMismatch,
                format!("capsule tag mismatch: expected {expected}, found {found}"),
            ),
        }
    }
}

#[derive(Clone)]
pub struct KayCtx {
    raw: KayContext,
}

impl KayCtx {
    pub fn from_raw(raw: KayContext) -> Self {
        Self { raw }
    }

    pub fn raw(&self) -> KayContext {
        self.raw
    }

    pub fn alloc_int(&self, value: i64) -> KayResult<KayHandle> {
        self.invoke(|ctx| (ctx.vtable.alloc_int)(ctx.id, value))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn alloc_bool(&self, value: bool) -> KayResult<KayHandle> {
        self.invoke(|ctx| (ctx.vtable.alloc_bool)(ctx.id, value))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn alloc_string(&self, value: impl Into<Arc<str>>) -> KayResult<KayHandle> {
        let arc = value.into();
        self.invoke(|ctx| (ctx.vtable.alloc_string)(ctx.id, arc.clone()))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn alloc_bytes(&self, value: impl Into<Arc<[u8]>>) -> KayResult<KayHandle> {
        let arc = value.into();
        self.invoke(|ctx| (ctx.vtable.alloc_bytes)(ctx.id, arc.clone()))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn alloc_unit(&self) -> KayResult<KayHandle> {
        self.invoke(|ctx| (ctx.vtable.alloc_unit)(ctx.id))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn inc_ref(&self, raw: KayRawHandle) -> KayResult<()> {
        self.invoke(|ctx| (ctx.vtable.inc_ref)(ctx.id, raw))
    }

    pub fn dec_ref(&self, raw: KayRawHandle) -> KayResult<()> {
        self.invoke(|ctx| (ctx.vtable.dec_ref)(ctx.id, raw))
    }

    pub fn inspect(&self, handle: KayRawHandle) -> KayResult<KayValueInfo> {
        self.invoke(|ctx| (ctx.vtable.inspect)(ctx.id, handle))
    }

    pub fn call_slot(&self, slot: KayHostSlot, args: &[KayHandle]) -> KayResult<KayHandle> {
        let raw_args = args.iter().map(|h| h.raw).collect::<Vec<_>>();
        self.invoke(|ctx| (ctx.vtable.call_host)(ctx.id, slot, raw_args))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn call_dynamic(&self, name: &str, args: &[KayHandle]) -> KayResult<KayHandle> {
        let raw_args = args.iter().map(|h| h.raw).collect::<Vec<_>>();
        self.invoke(|ctx| (ctx.vtable.call_host_dynamic)(ctx.id, name.to_string(), raw_args))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn new_capsule(
        &self,
        tag: &'static str,
        payload: Arc<dyn Any + Send + Sync>,
    ) -> KayResult<KayHandle> {
        let spec = KayCapsuleSpec { tag, payload };
        self.invoke(|ctx| (ctx.vtable.new_capsule)(ctx.id, spec))
            .map(|raw| KayHandle::new(self.clone(), raw))
    }

    pub fn capsule_data(&self, handle: KayRawHandle) -> KayResult<KayCapsuleData> {
        self.invoke(|ctx| (ctx.vtable.capsule_data)(ctx.id, handle))
    }

    pub fn handle_from_raw(&self, raw: KayRawHandle) -> KayHandle {
        KayHandle::new(self.clone(), raw)
    }

    pub fn clone_raw(&self, raw: KayRawHandle) -> KayResult<KayHandle> {
        self.inc_ref(raw).map(|_| KayHandle::new(self.clone(), raw))
    }

    fn invoke<T>(&self, f: impl FnOnce(KayContext) -> KayAbiResult<T>) -> KayResult<T> {
        f(self.raw)
    }
}

pub struct KayHandle {
    ctx: KayCtx,
    raw: KayRawHandle,
}

impl KayHandle {
    fn new(ctx: KayCtx, raw: KayRawHandle) -> Self {
        Self { ctx, raw }
    }

    pub fn ctx(&self) -> &KayCtx {
        &self.ctx
    }

    pub fn raw(&self) -> KayRawHandle {
        self.raw
    }

    pub fn describe(&self) -> KayResult<KayValueKind> {
        self.ctx.inspect(self.raw).map(|info| info.kind)
    }

    pub fn into_any(self) -> KayAny {
        KayAny { handle: self }
    }
}

impl Clone for KayHandle {
    fn clone(&self) -> Self {
        if let Err(err) = self.ctx.inc_ref(self.raw) {
            panic!("failed to clone handle: {err:?}");
        }
        Self {
            ctx: self.ctx.clone(),
            raw: self.raw,
        }
    }
}

impl Drop for KayHandle {
    fn drop(&mut self) {
        let _ = self.ctx.dec_ref(self.raw);
    }
}

impl fmt::Debug for KayHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KayHandle").field("raw", &self.raw).finish()
    }
}

#[derive(Clone)]
pub struct KayAny {
    handle: KayHandle,
}

impl KayAny {
    pub fn handle(&self) -> &KayHandle {
        &self.handle
    }

    pub fn describe(&self) -> KayResult<KayValueKind> {
        self.handle.describe()
    }
}

impl From<KayHandle> for KayAny {
    fn from(handle: KayHandle) -> Self {
        Self { handle }
    }
}

pub struct KayUnit;

#[derive(Clone)]
pub struct KayStr {
    handle: KayHandle,
}

impl KayStr {
    pub fn new(handle: KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::String(_) => Ok(Self { handle }),
            other => Err(KayApiError::TypeMismatch {
                expected: "string",
                found: other,
            }
            .into()),
        }
    }

    pub fn as_borrowed(&self) -> KayResult<KayBorrowedStr> {
        match self.handle.describe()? {
            KayValueKind::String(arc) => Ok(KayBorrowedStr { inner: arc }),
            other => Err(KayApiError::TypeMismatch {
                expected: "string",
                found: other,
            }
            .into()),
        }
    }

    pub fn to_string(&self) -> KayResult<String> {
        self.as_borrowed().map(|view| view.as_ref().to_string())
    }

    pub fn handle(&self) -> &KayHandle {
        &self.handle
    }
}

#[derive(Clone)]
pub struct KayBorrowedStr {
    inner: Arc<str>,
}

impl Deref for KayBorrowedStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<str> for KayBorrowedStr {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

#[derive(Clone)]
pub struct KayBytes {
    handle: KayHandle,
}

impl KayBytes {
    pub fn new(handle: KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::Bytes(_) => Ok(Self { handle }),
            other => Err(KayApiError::TypeMismatch {
                expected: "bytes",
                found: other,
            }
            .into()),
        }
    }

    pub fn as_borrowed(&self) -> KayResult<KayBorrowedBytes> {
        match self.handle.describe()? {
            KayValueKind::Bytes(data) => Ok(KayBorrowedBytes { inner: data }),
            other => Err(KayApiError::TypeMismatch {
                expected: "bytes",
                found: other,
            }
            .into()),
        }
    }
}

#[derive(Clone)]
pub struct KayBorrowedBytes {
    inner: Arc<[u8]>,
}

impl Deref for KayBorrowedBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<[u8]> for KayBorrowedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

#[derive(Clone)]
pub struct KayCapsule {
    handle: KayHandle,
    tag: &'static str,
}

impl KayCapsule {
    pub fn new<T>(ctx: &KayCtx, value: T, tag: &'static str) -> KayResult<Self>
    where
        T: Any + Send + Sync + 'static,
    {
        let payload: Arc<dyn Any + Send + Sync> = Arc::new(value);
        let handle = ctx.new_capsule(tag, payload)?;
        Ok(Self { handle, tag })
    }

    pub fn from_handle(handle: KayHandle) -> KayResult<Self> {
        let ctx = handle.ctx.clone();
        let data = ctx.capsule_data(handle.raw())?;
        Ok(Self {
            handle,
            tag: data.tag,
        })
    }

    pub fn tag(&self) -> &'static str {
        self.tag
    }

    pub fn downcast_arc<T>(&self, expected_tag: &'static str) -> KayResult<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        let data = self.handle.ctx.capsule_data(self.handle.raw())?;
        if data.tag != expected_tag {
            return Err(KayApiError::CapsuleTagMismatch {
                expected: expected_tag,
                found: data.tag,
            }
            .into());
        }
        Arc::downcast::<T>(data.payload).map_err(|_| {
            KayError::new(
                KayErrorCode::TypeMismatch,
                Some("capsule downcast failed".to_string()),
            )
        })
    }
}

pub trait ToKay {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle>;
}

pub trait FromKay: Sized {
    fn from_kay(ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self>;
}

impl ToKay for KayHandle {
    fn to_kay(self, _ctx: &KayCtx) -> KayResult<KayHandle> {
        Ok(self)
    }
}

impl FromKay for KayHandle {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        Ok(handle.clone())
    }
}

impl ToKay for KayAny {
    fn to_kay(self, _ctx: &KayCtx) -> KayResult<KayHandle> {
        Ok(self.handle)
    }
}

impl FromKay for KayAny {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        Ok(handle.clone().into_any())
    }
}

impl ToKay for () {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle> {
        ctx.alloc_unit()
    }
}

impl FromKay for () {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::Unit => Ok(()),
            other => Err(KayApiError::TypeMismatch {
                expected: "unit",
                found: other,
            }
            .into()),
        }
    }
}

impl ToKay for i64 {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle> {
        ctx.alloc_int(self)
    }
}

impl FromKay for i64 {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::Int(value) => Ok(value),
            other => Err(KayApiError::TypeMismatch {
                expected: "int",
                found: other,
            }
            .into()),
        }
    }
}

impl ToKay for bool {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle> {
        ctx.alloc_bool(self)
    }
}

impl FromKay for bool {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::Bool(value) => Ok(value),
            other => Err(KayApiError::TypeMismatch {
                expected: "bool",
                found: other,
            }
            .into()),
        }
    }
}

impl ToKay for String {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle> {
        ctx.alloc_string(Arc::<str>::from(self))
    }
}

impl ToKay for &str {
    fn to_kay(self, ctx: &KayCtx) -> KayResult<KayHandle> {
        ctx.alloc_string(Arc::<str>::from(self))
    }
}

impl FromKay for String {
    fn from_kay(_ctx: &KayCtx, handle: &KayHandle) -> KayResult<Self> {
        match handle.describe()? {
            KayValueKind::String(value) => Ok(value.deref().to_string()),
            other => Err(KayApiError::TypeMismatch {
                expected: "string",
                found: other,
            }
            .into()),
        }
    }
}

pub struct HandleScope<'ctx> {
    ctx: &'ctx KayCtx,
    handles: RefCell<Vec<KayRawHandle>>,
}

impl<'ctx> HandleScope<'ctx> {
    pub fn new(ctx: &'ctx KayCtx) -> Self {
        Self {
            ctx,
            handles: RefCell::new(Vec::new()),
        }
    }

    pub fn track<'scope>(&'scope self, handle: KayHandle) -> KayScopedHandle<'scope, 'ctx> {
        let raw = handle.raw;
        std::mem::forget(handle);
        self.handles.borrow_mut().push(raw);
        KayScopedHandle { scope: self, raw }
    }
}

impl Drop for HandleScope<'_> {
    fn drop(&mut self) {
        if let Ok(mut handles) = self.handles.try_borrow_mut() {
            for raw in handles.drain(..) {
                let _ = self.ctx.dec_ref(raw);
            }
        }
    }
}

pub struct KayScopedHandle<'scope, 'ctx> {
    scope: &'scope HandleScope<'ctx>,
    raw: KayRawHandle,
}

impl<'scope, 'ctx> KayScopedHandle<'scope, 'ctx> {
    pub fn to_handle(&self) -> KayResult<KayHandle> {
        self.scope.ctx.clone_raw(self.raw)
    }
}

#[derive(Clone, Copy)]
pub struct KayExtension {
    pub name: &'static str,
    pub callable: fn(&KayCtx, &[KayHandle]) -> KayResult<KayHandle>,
    pub min_arity: usize,
    pub max_arity: Option<usize>,
    pub doc: &'static str,
}

impl KayExtension {
    pub const fn new(
        name: &'static str,
        callable: fn(&KayCtx, &[KayHandle]) -> KayResult<KayHandle>,
        min_arity: usize,
        max_arity: Option<usize>,
        doc: &'static str,
    ) -> Self {
        Self {
            name,
            callable,
            min_arity,
            max_arity,
            doc,
        }
    }

    pub fn call(&self, ctx: &KayCtx, args: &[KayHandle]) -> KayResult<KayHandle> {
        if args.len() < self.min_arity {
            return Err(KayError::new(
                KayErrorCode::InvalidArgument,
                format!("expected at least {} arguments", self.min_arity),
            ));
        }
        if let Some(max) = self.max_arity {
            if args.len() > max {
                return Err(KayError::new(
                    KayErrorCode::InvalidArgument,
                    format!("expected at most {max} arguments"),
                ));
            }
        }
        (self.callable)(ctx, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_str_deref() {
        let arc: Arc<str> = Arc::from("hello");
        let view = KayBorrowedStr { inner: arc.clone() };
        assert_eq!(&*view, "hello");
        assert_eq!(view.as_ref(), "hello");
        assert_eq!(view.to_string(), "hello".to_string());
        assert_eq!(Arc::strong_count(&arc), 2);
    }
}
