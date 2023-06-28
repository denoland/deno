// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::error::GetErrorClassFn;
use crate::gotham_state::GothamState;
use crate::resources::ResourceTable;
use crate::runtime::ContextState;
use crate::runtime::JsRuntimeState;
use crate::OpDecl;
use crate::OpsTracker;
use anyhow::Error;
use futures::task::AtomicWaker;
use futures::Future;
use pin_project::pin_project;
use serde::Serialize;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr::NonNull;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use v8::fast_api::CFunctionInfo;
use v8::fast_api::CTypeInfo;

pub type PromiseId = i32;
pub type OpId = u16;

#[pin_project]
pub struct OpCall<F: Future<Output = OpResult>> {
  promise_id: PromiseId,
  op_id: OpId,
  /// Future is not necessarily Unpin, so we need to pin_project.
  #[pin]
  fut: F,
}

impl<F: Future<Output = OpResult>> OpCall<F> {
  /// Wraps a future; the inner future is polled the usual way (lazily).
  pub fn new(op_ctx: &OpCtx, promise_id: PromiseId, fut: F) -> Self {
    Self {
      op_id: op_ctx.id,
      promise_id,
      fut,
    }
  }
}

impl<F: Future<Output = OpResult>> Future for OpCall<F> {
  type Output = (PromiseId, OpId, OpResult);

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let promise_id = self.promise_id;
    let op_id = self.op_id;
    let fut = self.project().fut;
    fut.poll(cx).map(move |res| (promise_id, op_id, res))
  }
}

pub enum OpResult {
  Ok(serde_v8::SerializablePkg),
  Err(OpError),
}

impl OpResult {
  pub fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    match self {
      Self::Ok(x) => x.to_v8(scope),
      Self::Err(err) => serde_v8::to_v8(scope, err),
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpError {
  #[serde(rename = "$err_class_name")]
  class_name: &'static str,
  message: String,
  code: Option<&'static str>,
}

impl OpError {
  pub fn new(get_class: GetErrorClassFn, err: Error) -> Self {
    Self {
      class_name: (get_class)(&err),
      message: format!("{err:#}"),
      code: crate::error_codes::get_error_code(&err),
    }
  }
}

pub fn to_op_result<R: Serialize + 'static>(
  get_class: GetErrorClassFn,
  result: Result<R, Error>,
) -> OpResult {
  match result {
    Ok(v) => OpResult::Ok(v.into()),
    Err(err) => OpResult::Err(OpError::new(get_class, err)),
  }
}

/// Per-op context.
///
// Note: We don't worry too much about the size of this struct because it's allocated once per realm, and is
// stored in a contiguous array.
pub struct OpCtx {
  pub id: OpId,
  pub state: Rc<RefCell<OpState>>,
  pub decl: Rc<OpDecl>,
  pub fast_fn_c_info: Option<NonNull<v8::fast_api::CFunctionInfo>>,
  pub runtime_state: Weak<RefCell<JsRuntimeState>>,
  pub(crate) context_state: Rc<RefCell<ContextState>>,
  /// If the last fast op failed, stores the error to be picked up by the slow op.
  pub(crate) last_fast_error: UnsafeCell<Option<AnyError>>,
}

impl OpCtx {
  pub(crate) fn new(
    id: OpId,
    context_state: Rc<RefCell<ContextState>>,
    decl: Rc<OpDecl>,
    state: Rc<RefCell<OpState>>,
    runtime_state: Weak<RefCell<JsRuntimeState>>,
  ) -> Self {
    let mut fast_fn_c_info = None;

    if let Some(fast_fn) = &decl.fast_fn {
      let args = CTypeInfo::new_from_slice(fast_fn.args);
      let ret = CTypeInfo::new(fast_fn.return_type);

      // SAFETY: all arguments are coming from the trait and they have
      // static lifetime
      let c_fn = unsafe {
        CFunctionInfo::new(args.as_ptr(), fast_fn.args.len(), ret.as_ptr())
      };
      fast_fn_c_info = Some(c_fn);
    }

    OpCtx {
      id,
      state,
      runtime_state,
      decl,
      context_state,
      fast_fn_c_info,
      last_fast_error: UnsafeCell::new(None),
    }
  }

  /// This takes the last error from an [`OpCtx`], assuming that no other code anywhere
  /// can hold a `&mut` to the last_fast_error field.
  ///
  /// # Safety
  ///
  /// Must only be called from op implementations.
  #[inline(always)]
  pub unsafe fn unsafely_take_last_error_for_ops_only(
    &self,
  ) -> Option<AnyError> {
    let opt_mut = &mut *self.last_fast_error.get();
    opt_mut.take()
  }

  /// This set the last error for an [`OpCtx`], assuming that no other code anywhere
  /// can hold a `&mut` to the last_fast_error field.
  ///
  /// # Safety
  ///
  /// Must only be called from op implementations.
  #[inline(always)]
  pub unsafe fn unsafely_set_last_error_for_ops_only(&self, error: AnyError) {
    let opt_mut = &mut *self.last_fast_error.get();
    *opt_mut = Some(error);
  }
}

/// Maintains the resources and ops inside a JS runtime.
pub struct OpState {
  pub resource_table: ResourceTable,
  pub get_error_class_fn: GetErrorClassFn,
  pub tracker: OpsTracker,
  pub last_fast_op_error: Option<AnyError>,
  pub(crate) gotham_state: GothamState,
  pub waker: Arc<AtomicWaker>,
}

impl OpState {
  pub fn new(ops_count: usize) -> OpState {
    OpState {
      resource_table: Default::default(),
      get_error_class_fn: &|_| "Error",
      gotham_state: Default::default(),
      last_fast_op_error: None,
      tracker: OpsTracker::new(ops_count),
      waker: Arc::new(AtomicWaker::new()),
    }
  }

  /// Clear all user-provided resources and state.
  pub(crate) fn clear(&mut self) {
    std::mem::take(&mut self.gotham_state);
    std::mem::take(&mut self.resource_table);
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
