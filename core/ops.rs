// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::gotham_state::GothamState;
use crate::resources::ResourceTable;
use crate::runtime::GetErrorClassFn;
use crate::runtime::JsRuntimeState;
use crate::OpDecl;
use crate::OpsTracker;
use anyhow::Error;
use futures::future::MaybeDone;
use futures::Future;
use futures::FutureExt;
use pin_project::pin_project;
use serde::Serialize;
use std::cell::RefCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::rc::Weak;
use v8::fast_api::CFunctionInfo;
use v8::fast_api::CTypeInfo;

pub type RealmIdx = u16;
pub type PromiseId = i32;
pub type OpId = u16;

#[pin_project]
pub struct OpCall {
  realm_idx: RealmIdx,
  promise_id: PromiseId,
  op_id: OpId,
  /// Future is not necessarily Unpin, so we need to pin_project.
  #[pin]
  fut: MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>>,
}

impl OpCall {
  /// Wraps a future; the inner future is polled the usual way (lazily).
  pub fn pending(
    op_ctx: &OpCtx,
    promise_id: PromiseId,
    fut: Pin<Box<dyn Future<Output = OpResult> + 'static>>,
  ) -> Self {
    Self {
      realm_idx: op_ctx.realm_idx,
      op_id: op_ctx.id,
      promise_id,
      fut: MaybeDone::Future(fut),
    }
  }

  /// Create a future by specifying its output. This is basically the same as
  /// `async { value }` or `futures::future::ready(value)`.
  pub fn ready(op_ctx: &OpCtx, promise_id: PromiseId, value: OpResult) -> Self {
    Self {
      realm_idx: op_ctx.realm_idx,
      op_id: op_ctx.id,
      promise_id,
      fut: MaybeDone::Done(value),
    }
  }
}

impl Future for OpCall {
  type Output = (RealmIdx, PromiseId, OpId, OpResult);

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let realm_idx = self.realm_idx;
    let promise_id = self.promise_id;
    let op_id = self.op_id;
    let fut = &mut *self.project().fut;
    match fut {
      MaybeDone::Done(_) => {
        // Let's avoid using take_output as it keeps our Pin::box
        let res = std::mem::replace(fut, MaybeDone::Gone);
        let MaybeDone::Done(res) = res
        else {
          unreachable!()
        };
        std::task::Poll::Ready(res)
      }
      MaybeDone::Future(f) => f.poll_unpin(cx),
      MaybeDone::Gone => std::task::Poll::Pending,
    }
    .map(move |res| (realm_idx, promise_id, op_id, res))
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

// TODO(@AaronO): optimize OpCtx(s) mem usage ?
pub struct OpCtx {
  pub id: OpId,
  pub state: Rc<RefCell<OpState>>,
  pub decl: Rc<OpDecl>,
  pub fast_fn_c_info: Option<NonNull<v8::fast_api::CFunctionInfo>>,
  pub runtime_state: Weak<RefCell<JsRuntimeState>>,
  // Index of the current realm into `JsRuntimeState::known_realms`.
  pub realm_idx: RealmIdx,
}

impl OpCtx {
  pub fn new(
    id: OpId,
    realm_idx: RealmIdx,
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
      realm_idx,
      fast_fn_c_info,
    }
  }
}

/// Maintains the resources and ops inside a JS runtime.
pub struct OpState {
  pub resource_table: ResourceTable,
  pub get_error_class_fn: GetErrorClassFn,
  pub tracker: OpsTracker,
  pub last_fast_op_error: Option<AnyError>,
  gotham_state: GothamState,
}

impl OpState {
  pub fn new(ops_count: usize) -> OpState {
    OpState {
      resource_table: Default::default(),
      get_error_class_fn: &|_| "Error",
      gotham_state: Default::default(),
      last_fast_op_error: None,
      tracker: OpsTracker::new(ops_count),
    }
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
