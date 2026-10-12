// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use futures::task::AtomicWaker;
use v8::fast_api::CFunction;

use crate::OpDecl;
use crate::ResourceId;
use crate::error::JsStackFrame;
use crate::gotham_state::GothamState;
use crate::io::ResourceTable;
use crate::ops_metrics::OpMetricsFn;
use crate::runtime::JsRuntimeState;
use crate::runtime::OpDriverImpl;
use crate::runtime::UnrefedOps;

pub type PromiseId = i32;
pub type OpId = u16;

#[cfg(debug_assertions)]
thread_local! {
  static CURRENT_OP: std::cell::Cell<Option<&'static OpDecl>> = None.into();
}

#[cfg(debug_assertions)]
pub struct ReentrancyGuard {}

#[cfg(debug_assertions)]
impl Drop for ReentrancyGuard {
  fn drop(&mut self) {
    CURRENT_OP.with(|f| f.set(None));
  }
}

/// Creates an op re-entrancy check for the given [`OpDecl`].
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reentrancy_check(decl: &'static OpDecl) -> Option<ReentrancyGuard> {
  if decl.is_reentrant {
    return None;
  }

  let current = CURRENT_OP.with(|f| f.get());
  if let Some(current) = current {
    panic!(
      "op {} was not marked as #[op2(reentrant)], but re-entrantly invoked op {}",
      current.name, decl.name
    );
  }
  CURRENT_OP.with(|f| f.set(Some(decl)));
  Some(ReentrancyGuard {})
}

#[derive(Clone, Copy)]
pub struct OpMetadata {
  /// A description of the op for use in sanitizer output.
  pub sanitizer_details: Option<&'static str>,
  /// The fix for the issue described in `sanitizer_details`.
  pub sanitizer_fix: Option<&'static str>,
}

impl OpMetadata {
  pub const fn default() -> Self {
    Self {
      sanitizer_details: None,
      sanitizer_fix: None,
    }
  }
}

/// The parts of an op's context that are identical for every op in a realm.
///
/// Stored once per realm (boxed, inside [`OpCtxs`]) and shared by pointer from
/// every [`OpCtx`], instead of being cloned ~700 times.
pub struct OpCommonCtx {
  /// A stashed Isolate that ops can make use of. This is a raw isolate pointer,
  /// and as such, is extremely dangerous to use. It is filled in once the
  /// isolate has been created (the op contexts are built before it).
  isolate: Cell<v8::UnsafeRawIsolatePtr>,
  state: Rc<RefCell<OpState>>,
  op_driver: Rc<OpDriverImpl>,
  runtime_state: *const JsRuntimeState,
  enable_stack_trace: bool,
}

/// Per-op context.
///
// Note: This struct is allocated once per op per realm and stored in a
// contiguous array, so its size is multiplied by the number of registered ops
// (~700 in a full Deno build). Everything that is identical across ops lives in
// the shared `OpCommonCtx`, and the `OpDecl` is borrowed from the realm's
// declaration storage (static tables where possible) rather than copied in.
pub struct OpCtx {
  /// The id for this op. Will be identical across realms.
  pub id: OpId,

  /// Points at a declaration owned (or borrowed from static memory) by the
  /// [`OpCtxs`] that owns this `OpCtx`. That storage is never mutated or
  /// reallocated after the op contexts are created, so the pointee outlives
  /// every `OpCtx`.
  decl: *const OpDecl,

  /// Points at the `common` box owned by the [`OpCtxs`] that owns this `OpCtx`.
  /// Same lifetime argument as `decl`.
  common: *const OpCommonCtx,

  /// The fast-call overload passed to `FunctionTemplate::build_fast`. Stored
  /// here (rather than synthesized on the stack) because V8 150.x keeps the raw
  /// `CFunction` pointers directly inside `FunctionTemplateInfo`, so the slice
  /// must outlive every template built from this op. `OpCtx` lives until isolate
  /// disposal, which satisfies that.
  ///
  /// The single element is also the op's `CFunctionInfo` source (see
  /// [`OpCtx::fast_fn_info`]).
  pub(crate) fast_fn_overloads: Option<[CFunction; 1]>,
  pub(crate) metrics_fn: Option<OpMetricsFn>,
}

impl OpCtx {
  fn new(
    id: OpId,
    decl: *const OpDecl,
    common: *const OpCommonCtx,
    metrics_fn: Option<OpMetricsFn>,
  ) -> Self {
    // SAFETY: `decl` points into the caller's decl array, which outlives us.
    let decl_ref = unsafe { &*decl };
    // If we want metrics for this function, create the fastcall `CFunctionInfo` from the metrics
    // `CFunction`. For some extremely fast ops, the parameter list may change for the metrics
    // version and require a slightly different set of arguments (for example, it may need the fastcall
    // callback information to get the `OpCtx`).
    let fast_fn_info = if metrics_fn.is_some() {
      decl_ref.fast_fn_with_metrics
    } else {
      decl_ref.fast_fn
    };

    Self {
      id,
      decl,
      common,
      fast_fn_overloads: fast_fn_info.map(|f| [f]),
      metrics_fn,
    }
  }

  #[inline(always)]
  pub fn decl(&self) -> &OpDecl {
    // SAFETY: the decl array owned by our `OpCtxs` outlives this `OpCtx`.
    unsafe { &*self.decl }
  }

  #[inline(always)]
  pub fn common(&self) -> &OpCommonCtx {
    // SAFETY: the common context owned by our `OpCtxs` outlives this `OpCtx`.
    unsafe { &*self.common }
  }

  /// The op state shared by every op in this realm.
  #[doc(hidden)]
  #[inline(always)]
  pub fn state(&self) -> &Rc<RefCell<OpState>> {
    &self.common().state
  }

  /// A stashed Isolate that ops can make use of. This is a raw isolate pointer,
  /// and as such, is extremely dangerous to use.
  #[doc(hidden)]
  #[inline(always)]
  pub fn isolate(&self) -> v8::UnsafeRawIsolatePtr {
    self.common().isolate.get()
  }

  #[doc(hidden)]
  #[inline(always)]
  pub fn enable_stack_trace(&self) -> bool {
    self.common().enable_stack_trace
  }

  /// The `CFunctionInfo`-bearing fastcall used to build this op's template.
  #[inline(always)]
  pub(crate) fn fast_fn_info(&self) -> Option<CFunction> {
    self.fast_fn_overloads.map(|f| f[0])
  }

  #[inline(always)]
  pub const fn metrics_enabled(&self) -> bool {
    self.metrics_fn.is_some()
  }

  /// Generates four external references for each op. If an op does not have a fastcall, it generates
  /// "null" slots to avoid changing the size of the external references array.
  pub fn external_references(&self) -> [v8::ExternalReference; 4] {
    extern "C" fn placeholder() {}

    let ctx_ptr = v8::ExternalReference {
      pointer: self as *const OpCtx as _,
    };
    let null = v8::ExternalReference {
      pointer: placeholder as _,
    };
    let decl = self.decl();

    if self.metrics_enabled() {
      let slow_fn = v8::ExternalReference {
        function: decl.slow_fn_with_metrics,
      };
      if let (Some(fast_fn), Some(fast_fn_info)) =
        (decl.fast_fn_with_metrics, self.fast_fn_info())
      {
        let fast_fn = v8::ExternalReference {
          pointer: fast_fn.address() as _,
        };
        let fast_info = v8::ExternalReference {
          type_info: fast_fn_info.type_info(),
        };
        [ctx_ptr, slow_fn, fast_fn, fast_info]
      } else {
        [ctx_ptr, slow_fn, null, null]
      }
    } else {
      let slow_fn = v8::ExternalReference {
        function: decl.slow_fn,
      };
      if let (Some(fast_fn), Some(fast_fn_info)) =
        (decl.fast_fn, self.fast_fn_info())
      {
        let fast_fn = v8::ExternalReference {
          pointer: fast_fn.address() as _,
        };
        let fast_info = v8::ExternalReference {
          type_info: fast_fn_info.type_info(),
        };
        [ctx_ptr, slow_fn, fast_fn, fast_info]
      } else {
        [ctx_ptr, slow_fn, null, null]
      }
    }
  }

  pub(crate) fn op_driver(&self) -> &OpDriverImpl {
    &self.common().op_driver
  }

  /// Get the [`JsRuntimeState`] for this op.
  pub(crate) fn runtime_state(&self) -> &JsRuntimeState {
    // SAFETY: JsRuntimeState outlives OpCtx
    unsafe { &*self.common().runtime_state }
  }
}

/// The op table of a realm: the op contexts plus the two allocations they point
/// into.
///
/// The `decls` storage and the `common` box must not be mutated or dropped
/// while `ctxs` is alive — every [`OpCtx`] holds a raw pointer into them. Every
/// variant of [`OpDeclStorage`] keeps its declarations at a fixed address
/// (static memory or a heap buffer), so moving this struct into the
/// `ContextState` does not invalidate the pointers.
pub struct OpCtxs {
  ctxs: Box<[OpCtx]>,
  #[allow(
    dead_code,
    reason = "storage for the decls borrowed by `ctxs` (via raw pointer)"
  )]
  decls: Vec<OpDeclStorage>,
  common: Box<OpCommonCtx>,
}

/// A contiguous run of op declarations, in op-registration order.
///
/// Declarations that already live in static memory (deno_core's `BUILTIN_OPS`,
/// an extension's method tables, and any extension whose op table is
/// `Cow::Borrowed` — which `extension!` emits for every extension, generic or
/// not, a generic one getting one static table per monomorphization) are
/// borrowed rather than copied into the realm. The owned variant covers the
/// cases where a declaration genuinely cannot be static: extension middleware
/// (`Extension::middleware`) rewrites decls at startup, `ops_fn` appends to
/// the table, and method constructors get their name patched from the
/// enclosing `OpMethodDecl`.
pub enum OpDeclStorage {
  Static(&'static [OpDecl]),
  Owned(Vec<OpDecl>),
}

impl OpDeclStorage {
  pub fn as_slice(&self) -> &[OpDecl] {
    match self {
      Self::Static(decls) => decls,
      Self::Owned(decls) => decls,
    }
  }

  pub fn len(&self) -> usize {
    self.as_slice().len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn iter(&self) -> std::slice::Iter<'_, OpDecl> {
    self.as_slice().iter()
  }
}

impl OpCtxs {
  pub(crate) fn new(
    decls: Vec<OpDeclStorage>,
    state: Rc<RefCell<OpState>>,
    op_driver: Rc<OpDriverImpl>,
    runtime_state: *const JsRuntimeState,
    enable_stack_trace: bool,
    mut op_info: impl FnMut(usize, &OpDecl) -> (OpId, Option<OpMetricsFn>),
  ) -> Self {
    let common = Box::new(OpCommonCtx {
      isolate: Cell::new(v8::UnsafeRawIsolatePtr::null()),
      state,
      op_driver,
      runtime_state,
      enable_stack_trace,
    });
    let common_ptr = common.as_ref() as *const OpCommonCtx;
    let ctxs = decls
      .iter()
      .flat_map(|storage| storage.as_slice().iter())
      .enumerate()
      .map(|(index, decl)| {
        let (id, metrics_fn) = op_info(index, decl);
        OpCtx::new(id, decl as *const OpDecl, common_ptr, metrics_fn)
      })
      .collect::<Vec<_>>()
      .into_boxed_slice();
    Self {
      ctxs,
      decls,
      common,
    }
  }

  /// Hand the isolate pointer to every op once the isolate exists.
  pub(crate) fn set_isolate(&self, isolate: v8::UnsafeRawIsolatePtr) {
    self.common.isolate.set(isolate);
  }
}

impl Deref for OpCtxs {
  type Target = [OpCtx];
  #[inline(always)]
  fn deref(&self) -> &[OpCtx] {
    &self.ctxs
  }
}

/// Allows an embedder to track operations which should
/// keep the event loop alive.
#[derive(Debug, Clone)]
pub struct ExternalOpsTracker {
  counter: Arc<AtomicUsize>,
}

impl ExternalOpsTracker {
  pub fn ref_op(&self) {
    self.counter.fetch_add(1, Ordering::Relaxed);
  }

  pub fn unref_op(&self) {
    let _ =
      self
        .counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
          if x == 0 { None } else { Some(x - 1) }
        });
  }

  pub(crate) fn has_pending_ops(&self) -> bool {
    self.counter.load(Ordering::Relaxed) > 0
  }
}

pub type OpStackTraceCallback = Box<dyn Fn(Vec<JsStackFrame>)>;

/// Maintains the resources and ops inside a JS runtime.
pub struct OpState {
  pub resource_table: ResourceTable,
  pub(crate) gotham_state: GothamState,
  pub waker: Arc<AtomicWaker>,
  pub external_ops_tracker: ExternalOpsTracker,
  pub op_stack_trace_callback: Option<OpStackTraceCallback>,
  /// Reference to the unrefered ops state in `ContextState`.
  pub(crate) unrefed_ops: UnrefedOps,
  /// Resources that are not referenced by the event loop. All async
  /// resource ops on these resources will not keep the event loop alive.
  ///
  /// Used to implement `uv_ref` and `uv_unref` methods for Node compat.
  pub(crate) unrefed_resources: HashSet<ResourceId>,
}

impl OpState {
  pub fn new(op_stack_trace_callback: Option<OpStackTraceCallback>) -> OpState {
    OpState {
      resource_table: Default::default(),
      gotham_state: Default::default(),
      waker: Arc::new(AtomicWaker::new()),
      external_ops_tracker: ExternalOpsTracker {
        counter: Arc::new(AtomicUsize::new(0)),
      },
      op_stack_trace_callback,
      unrefed_ops: Default::default(),
      unrefed_resources: Default::default(),
    }
  }

  /// Clear all user-provided resources and state.
  pub(crate) fn clear(&mut self) {
    std::mem::take(&mut self.gotham_state);
    std::mem::take(&mut self.resource_table);
  }

  // Silly but improves readability.
  pub fn uv_unref(&mut self, resource_id: ResourceId) {
    self.unrefed_resources.insert(resource_id);
  }

  pub fn uv_ref(&mut self, resource_id: ResourceId) {
    self.unrefed_resources.remove(&resource_id);
  }

  pub fn has_ref(&self, resource_id: ResourceId) -> bool {
    !self.unrefed_resources.contains(&resource_id)
  }
}

impl Deref for OpState {
  type Target = GothamState;

  fn deref(&self) -> &Self::Target {
    &self.gotham_state
  }
}

impl DerefMut for OpState {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.gotham_state
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// `OpCtx` is instantiated once per op per realm (~700 ops in a full Deno
  /// build), so growth here is multiplied by the op count. Everything that is
  /// the same for every op belongs in `OpCommonCtx` instead.
  #[test]
  fn op_ctx_stays_small() {
    assert!(
      std::mem::size_of::<OpCtx>() <= 64,
      "OpCtx grew to {} bytes; anything shared by every op belongs in OpCommonCtx",
      std::mem::size_of::<OpCtx>()
    );
  }
}
