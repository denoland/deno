// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::gotham_state::GothamState;
use crate::realm::ContextState;
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
use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::rc::Weak;
use v8::fast_api::CFunctionInfo;
use v8::fast_api::CTypeInfo;

pub type PromiseId = i32;
pub type OpId = u16;

#[pin_project]
pub struct OpCall {
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
      op_id: op_ctx.id,
      promise_id,
      fut: MaybeDone::Future(fut),
    }
  }

  /// Create a future by specifying its output. This is basically the same as
  /// `async { value }` or `futures::future::ready(value)`.
  pub fn ready(op_ctx: &OpCtx, promise_id: PromiseId, value: OpResult) -> Self {
    Self {
      op_id: op_ctx.id,
      promise_id,
      fut: MaybeDone::Done(value),
    }
  }
}

impl Future for OpCall {
  type Output = (PromiseId, OpId, OpResult);

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
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
    .map(move |res| (promise_id, op_id, res))
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

macro_rules! try_number {
  ($n:ident $type:ident $is:ident) => {
    if $n.$is() {
      // SAFETY: v8 handles can be transmuted
      let n: &v8::Uint32 = unsafe { std::mem::transmute($n) };
      return n.value() as _;
    }
  };
}

pub fn to_u32(number: &v8::Value) -> u32 {
  try_number!(number Uint32 is_uint32);
  try_number!(number Int32 is_int32);
  try_number!(number Number is_number);
  if number.is_big_int() {
    // SAFETY: v8 handles can be transmuted
    let n: &v8::BigInt = unsafe { std::mem::transmute(number) };
    return n.u64_value().0 as _;
  }
  0
}

pub fn to_i32(number: &v8::Value) -> i32 {
  try_number!(number Uint32 is_uint32);
  try_number!(number Int32 is_int32);
  try_number!(number Number is_number);
  if number.is_big_int() {
    // SAFETY: v8 handles can be transmuted
    let n: &v8::BigInt = unsafe { std::mem::transmute(number) };
    return n.i64_value().0 as _;
  }
  0
}

#[allow(unused)]
pub fn to_u64(number: &v8::Value) -> u32 {
  try_number!(number Uint32 is_uint32);
  try_number!(number Int32 is_int32);
  try_number!(number Number is_number);
  if number.is_big_int() {
    // SAFETY: v8 handles can be transmuted
    let n: &v8::BigInt = unsafe { std::mem::transmute(number) };
    return n.u64_value().0 as _;
  }
  0
}

#[allow(unused)]
pub fn to_i64(number: &v8::Value) -> i32 {
  try_number!(number Uint32 is_uint32);
  try_number!(number Int32 is_int32);
  try_number!(number Number is_number);
  if number.is_big_int() {
    // SAFETY: v8 handles can be transmuted
    let n: &v8::BigInt = unsafe { std::mem::transmute(number) };
    return n.i64_value().0 as _;
  }
  0
}

#[cfg(test)]
mod tests {
  use crate::error::generic_error;
  use crate::error::AnyError;
  use crate::error::JsError;
  use crate::FastString;
  use crate::JsRuntime;
  use crate::RuntimeOptions;
  use deno_ops::op2;
  use std::cell::Cell;

  crate::extension!(
    testing,
    ops = [
      op_test_fail,
      op_test_add,
      op_test_add_option,
      op_test_result_void_switch,
      op_test_result_void_ok,
      op_test_result_void_err,
      op_test_result_primitive_ok,
      op_test_result_primitive_err
    ]
  );

  thread_local! {
    static FAIL: Cell<bool> = Cell::new(false)
  }

  #[op2(core, fast)]
  pub fn op_test_fail() {
    FAIL.with(|b| {
      println!("fail");
      b.set(true)
    })
  }

  /// Run a test for a single op.
  fn run_test2(
    repeat: usize,
    op: &'static str,
    test: &'static str,
  ) -> Result<(), AnyError> {
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![testing::init_ops_and_esm()],
      ..Default::default()
    });
    runtime
      .execute_script(
        "",
        FastString::Owned(
          format!(
            r"
            const {{ op_test_fail, {op} }} = Deno.core.ensureFastOps();
            function assert(b) {{
              if (!b) {{
                op_test_fail();
              }}
            }}
          "
          )
          .into(),
        ),
      )
      .unwrap();
    FAIL.with(|b| b.set(false));
    runtime.execute_script(
      "",
      FastString::Owned(
        format!(
          r"
      for (let __index__ = 0; __index__ < {repeat}; __index__++) {{
        {test}
      }}
    "
        )
        .into(),
      ),
    )?;
    if FAIL.with(|b| b.get()) {
      Err(generic_error("test failed"))
    } else {
      Ok(())
    }
  }

  #[tokio::test(flavor = "current_thread")]
  pub async fn test_op_fail() {
    assert!(run_test2(1, "", "assert(false)").is_err());
  }

  #[op2(core, fast)]
  pub fn op_test_add(a: u32, b: u32) -> u32 {
    a + b
  }

  #[tokio::test(flavor = "current_thread")]
  pub async fn test_op_add() -> Result<(), Box<dyn std::error::Error>> {
    Ok(run_test2(
      10000,
      "op_test_add",
      "assert(op_test_add(1, 11) == 12)",
    )?)
  }

  #[op2(core)]
  pub fn op_test_add_option(a: u32, b: Option<u32>) -> u32 {
    a + b.unwrap_or(100)
  }

  #[tokio::test(flavor = "current_thread")]
  pub async fn test_op_add_option() -> Result<(), Box<dyn std::error::Error>> {
    // This isn't fast, so we don't repeat it
    run_test2(
      1,
      "op_test_add_option",
      "assert(op_test_add_option(1, 11) == 12)",
    )?;
    run_test2(
      1,
      "op_test_add_option",
      "assert(op_test_add_option(1, null) == 101)",
    )?;
    Ok(())
  }

  thread_local! {
    static RETURN_COUNT: Cell<usize> = Cell::new(0);
  }

  #[op2(core, fast)]
  pub fn op_test_result_void_switch() -> Result<(), AnyError> {
    let count = RETURN_COUNT.with(|count| {
      let new = count.get() + 1;
      count.set(new);
      new
    });
    if count > 5000 {
      Err(generic_error("failed!!!"))
    } else {
      Ok(())
    }
  }

  #[op2(core, fast)]
  pub fn op_test_result_void_err() -> Result<(), AnyError> {
    Err(generic_error("failed!!!"))
  }

  #[op2(core, fast)]
  pub fn op_test_result_void_ok() -> Result<(), AnyError> {
    Ok(())
  }

  #[tokio::test(flavor = "current_thread")]
  pub async fn test_op_result_void() -> Result<(), Box<dyn std::error::Error>> {
    // Test the non-switching kinds
    run_test2(
      10000,
      "op_test_result_void_err",
      "try { op_test_result_void_err(); assert(false) } catch (e) {}",
    )?;
    run_test2(10000, "op_test_result_void_ok", "op_test_result_void_ok()")?;
    Ok(())
  }

  #[tokio::test(flavor = "current_thread")]
  pub async fn test_op_result_void_switch(
  ) -> Result<(), Box<dyn std::error::Error>> {
    RETURN_COUNT.with(|count| count.set(0));
    let err = run_test2(
      10000,
      "op_test_result_void_switch",
      "op_test_result_void_switch();",
    )
    .expect_err("Expected this to fail");
    let js_err = err.downcast::<JsError>().unwrap();
    assert_eq!(js_err.message, Some("failed!!!".into()));
    assert_eq!(RETURN_COUNT.with(|count| count.get()), 5001);
    Ok(())
  }

  #[op2(core, fast)]
  pub fn op_test_result_primitive_err() -> Result<u32, AnyError> {
    Err(generic_error("failed!!!"))
  }

  #[op2(core, fast)]
  pub fn op_test_result_primitive_ok() -> Result<u32, AnyError> {
    Ok(123)
  }

  #[tokio::test]
  pub async fn test_op_result_primitive(
  ) -> Result<(), Box<dyn std::error::Error>> {
    run_test2(
      10000,
      "op_test_result_primitive_err",
      "try { op_test_result_primitive_err(); assert(false) } catch (e) {}",
    )?;
    run_test2(
      10000,
      "op_test_result_primitive_ok",
      "op_test_result_primitive_ok()",
    )?;
    Ok(())
  }
}
