// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::ops::*;
use crate::OpResult;
use crate::PromiseId;
use anyhow::Error;
use futures::future::Either;
use futures::future::Future;
use futures::future::FutureExt;
use futures::task::noop_waker_ref;
use std::cell::RefCell;
use std::future::ready;
use std::option::Option;
use std::task::Context;
use std::task::Poll;

#[inline]
pub fn queue_fast_async_op<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  promise_id: PromiseId,
  op: impl Future<Output = Result<R, Error>> + 'static,
) {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };
  let fut = op.map(|result| crate::_ops::to_op_result(get_class, result));
  // SAFETY: this is guaranteed to be running on a current-thread executor
  ctx.context_state.borrow_mut().pending_ops.spawn(unsafe {
    crate::task::MaskFutureAsSend::new(OpCall::new(ctx, promise_id, fut))
  });
}

#[inline]
pub fn map_async_op1<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = Result<R, Error>> + 'static,
) -> impl Future<Output = OpResult> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  op.map(|res| crate::_ops::to_op_result(get_class, res))
}

#[inline]
pub fn map_async_op2<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = R> + 'static,
) -> impl Future<Output = OpResult> {
  let state = RefCell::borrow(&ctx.state);
  state.tracker.track_async(ctx.id);

  op.map(|res| OpResult::Ok(res.into()))
}

#[inline]
pub fn map_async_op3<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = Result<R, Error>> + 'static, Error>,
) -> impl Future<Output = OpResult> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => {
      Either::Left(ready(OpResult::Err(OpError::new(get_class, err))))
    }
    Ok(fut) => {
      Either::Right(fut.map(|res| crate::_ops::to_op_result(get_class, res)))
    }
  }
}

#[inline]
pub fn map_async_op4<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = R> + 'static, Error>,
) -> impl Future<Output = OpResult> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => {
      Either::Left(ready(OpResult::Err(OpError::new(get_class, err))))
    }
    Ok(fut) => Either::Right(fut.map(|r| OpResult::Ok(r.into()))),
  }
}

pub fn queue_async_op<'s>(
  ctx: &OpCtx,
  scope: &'s mut v8::HandleScope,
  deferred: bool,
  promise_id: PromiseId,
  op: impl Future<Output = OpResult> + 'static,
) -> Option<v8::Local<'s, v8::Value>> {
  // An op's realm (as given by `OpCtx::realm_idx`) must match the realm in
  // which it is invoked. Otherwise, we might have cross-realm object exposure.
  // deno_core doesn't currently support such exposure, even though embedders
  // can cause them, so we panic in debug mode (since the check is expensive).
  // TODO(mmastrac): Restore this
  // debug_assert_eq!(
  //   runtime_state.borrow().context(ctx.realm_idx as usize, scope),
  //   Some(scope.get_current_context())
  // );

  let id = ctx.id;

  // TODO(mmastrac): We have to poll every future here because that assumption is baked into a large number
  // of ops. If we can figure out a way around this, we can remove this call to boxed_local and save a malloc per future.
  let mut pinned = op.map(move |res| (promise_id, id, res)).boxed_local();

  match pinned.poll_unpin(&mut Context::from_waker(noop_waker_ref())) {
    Poll::Pending => {}
    Poll::Ready(mut res) => {
      if deferred {
        ctx
          .context_state
          .borrow_mut()
          .pending_ops
          // SAFETY: this is guaranteed to be running on a current-thread executor
          .spawn(unsafe { crate::task::MaskFutureAsSend::new(ready(res)) });
        return None;
      } else {
        ctx.state.borrow_mut().tracker.track_async_completed(ctx.id);
        return Some(res.2.to_v8(scope).unwrap());
      }
    }
  }

  ctx
    .context_state
    .borrow_mut()
    .pending_ops
    // SAFETY: this is guaranteed to be running on a current-thread executor
    .spawn(unsafe { crate::task::MaskFutureAsSend::new(pinned) });
  None
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
