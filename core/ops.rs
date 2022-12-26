// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::gotham_state::GothamState;
use crate::resources::ResourceTable;
use crate::runtime::GetErrorClassFn;
use crate::runtime::JsRuntimeState;
use crate::OpDecl;
use crate::OpsTracker;
use anyhow::Error;
use futures::future::maybe_done;
use futures::future::FusedFuture;
use futures::future::MaybeDone;
use futures::ready;
use futures::task::noop_waker;
use futures::Future;
use serde::Serialize;
use std::cell::RefCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;
use std::rc::Weak;
use std::task::Context;
use std::task::Poll;

/// Wrapper around a Future, which causes that Future to be polled immediately.
///
/// Background: ops are stored in a `FuturesUnordered` structure which polls
/// them, but without the `OpCall` wrapper this doesn't happen until the next
/// turn of the event loop, which is too late for certain ops.
pub struct OpCall<T>(MaybeDone<Pin<Box<dyn Future<Output = T>>>>);

pub enum EagerPollResult<T> {
  Ready(T),
  Pending(OpCall<T>),
}

impl<T> OpCall<T> {
  /// Wraps a future, and polls the inner future immediately.
  /// This should be the default choice for ops.
  pub fn eager(fut: impl Future<Output = T> + 'static) -> EagerPollResult<T> {
    let boxed = Box::pin(fut) as Pin<Box<dyn Future<Output = T>>>;
    let mut inner = maybe_done(boxed);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut inner);
    let poll = pinned.as_mut().poll(&mut cx);
    match poll {
      Poll::Ready(_) => EagerPollResult::Ready(pinned.take_output().unwrap()),
      _ => EagerPollResult::Pending(Self(inner)),
    }
  }

  /// Wraps a future; the inner future is polled the usual way (lazily).
  pub fn lazy(fut: impl Future<Output = T> + 'static) -> Self {
    let boxed = Box::pin(fut) as Pin<Box<dyn Future<Output = T>>>;
    let inner = maybe_done(boxed);
    Self(inner)
  }

  /// Create a future by specifying its output. This is basically the same as
  /// `async { value }` or `futures::future::ready(value)`.
  pub fn ready(value: T) -> Self {
    Self(MaybeDone::Done(value))
  }
}

impl<T> Future for OpCall<T> {
  type Output = T;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    // TODO(piscisaureus): safety comment
    #[allow(clippy::undocumented_unsafe_blocks)]
    let inner = unsafe { &mut self.get_unchecked_mut().0 };
    let mut pinned = Pin::new(inner);
    ready!(pinned.as_mut().poll(cx));
    Poll::Ready(pinned.as_mut().take_output().unwrap())
  }
}

impl<F> FusedFuture for OpCall<F>
where
  F: Future,
{
  fn is_terminated(&self) -> bool {
    self.0.is_terminated()
  }
}

pub type PromiseId = i32;
pub type OpAsyncFuture = OpCall<(PromiseId, OpId, OpResult)>;
pub type OpFn =
  fn(&mut v8::HandleScope, v8::FunctionCallbackArguments, v8::ReturnValue);
pub type OpId = usize;

pub enum Op {
  Sync(OpResult),
  Async(OpAsyncFuture),
  NotFound,
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
      message: format!("{:#}", err),
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
  pub runtime_state: Weak<RefCell<JsRuntimeState>>,
  // Index of the current realm into `JsRuntimeState::known_realms`.
  pub realm_idx: usize,
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
