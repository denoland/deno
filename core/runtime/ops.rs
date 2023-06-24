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

  crate::extension!(
    testing,
    ops = [
      op_test_add,
      op_test_add_option,
      op_test_result_void_ok,
      op_test_result_void_err
    ]
  );

  /// Run a test for a single op.
  fn run_test(
    op: &'static str,
    test: &'static str,
    f: impl FnOnce(Result<&v8::Value, anyhow::Error>, &mut v8::HandleScope),
  ) {
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![testing::init_ops_and_esm()],
      ..Default::default()
    });
    let value: Result<v8::Global<v8::Value>, anyhow::Error> = runtime
      .execute_script(
        "",
        FastString::Owned(
          format!("const {{ {op} }} = Deno.core.ensureFastOps(); {test}")
            .into(),
        ),
      );
    let mut scope: v8::HandleScope =
      // SAFETY: transmute for test (this lifetime should be safe for this purpose)
      unsafe { std::mem::transmute(runtime.handle_scope()) };
    match value {
      Ok(value) => {
        let value = value.open(&mut scope);
        f(Ok(value), &mut scope)
      }
      Err(err) => f(Err(err), &mut scope),
    }
  }

  #[op2(core, fast)]
  pub fn op_test_add(a: u32, b: u32) -> u32 {
    a + b
  }

  #[tokio::test]
  pub async fn test_op_add() -> Result<(), Box<dyn std::error::Error>> {
    run_test("op_test_add", "op_test_add(1, 11)", |value, scope| {
      assert_eq!(value.unwrap().int32_value(scope), Some(12));
    });
    Ok(())
  }

  #[op2(core)]
  pub fn op_test_add_option(a: u32, b: Option<u32>) -> u32 {
    a + b.unwrap_or(100)
  }

  #[tokio::test]
  pub async fn test_op_add_option() -> Result<(), Box<dyn std::error::Error>> {
    run_test(
      "op_test_add_option",
      "op_test_add_option(1, 11)",
      |value, scope| {
        assert_eq!(value.unwrap().int32_value(scope), Some(12));
      },
    );
    run_test(
      "op_test_add_option",
      "op_test_add_option(1, null)",
      |value, scope| {
        assert_eq!(value.unwrap().int32_value(scope), Some(101));
      },
    );
    Ok(())
  }

  #[op2(core)]
  pub fn op_test_result_void_err() -> Result<(), AnyError> {
    Err(generic_error("failed!!!"))
  }

  #[op2(core)]
  pub fn op_test_result_void_ok() -> Result<(), AnyError> {
    Ok(())
  }

  #[tokio::test]
  pub async fn test_op_result() -> Result<(), Box<dyn std::error::Error>> {
    run_test(
      "op_test_result_void_err",
      "op_test_result_void_err()",
      |value, _scope| {
        let js_error = value.err().unwrap().downcast::<JsError>().unwrap();
        assert_eq!(js_error.message, Some("failed!!!".to_owned()));
      },
    );
    run_test(
      "op_test_result_void_ok",
      "op_test_result_void_ok()",
      |value, _scope| assert!(value.unwrap().is_null_or_undefined()),
    );
    Ok(())
  }
}
