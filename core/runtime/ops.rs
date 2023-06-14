// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::ops::*;
use crate::OpResult;
use crate::PromiseId;
use anyhow::Error;
use futures::future::Future;
use futures::future::FutureExt;
use futures::future::MaybeDone;
use futures::task::noop_waker;
use std::cell::RefCell;
use std::option::Option;
use std::pin::Pin;
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
  let fut = op
    .map(|result| crate::_ops::to_op_result(get_class, result))
    .boxed_local();
  // SAFETY: this this is guaranteed to be running on a current-thread executor
  ctx.context_state.borrow_mut().pending_ops.spawn(unsafe {
    crate::task::MaskFutureAsSend::new(OpCall::pending(ctx, promise_id, fut))
  });
}

#[inline]
pub fn map_async_op1<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = Result<R, Error>> + 'static,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  let fut = op
    .map(|result| crate::_ops::to_op_result(get_class, result))
    .boxed_local();
  MaybeDone::Future(fut)
}

#[inline]
pub fn map_async_op2<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = R> + 'static,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let state = RefCell::borrow(&ctx.state);
  state.tracker.track_async(ctx.id);

  let fut = op.map(|result| OpResult::Ok(result.into())).boxed_local();
  MaybeDone::Future(fut)
}

#[inline]
pub fn map_async_op3<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = Result<R, Error>> + 'static, Error>,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => MaybeDone::Done(OpResult::Err(OpError::new(get_class, err))),
    Ok(fut) => MaybeDone::Future(
      fut
        .map(|result| crate::_ops::to_op_result(get_class, result))
        .boxed_local(),
    ),
  }
}

#[inline]
pub fn map_async_op4<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = R> + 'static, Error>,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => MaybeDone::Done(OpResult::Err(OpError::new(get_class, err))),
    Ok(fut) => MaybeDone::Future(
      fut.map(|result| OpResult::Ok(result.into())).boxed_local(),
    ),
  }
}

pub fn queue_async_op<'s>(
  ctx: &OpCtx,
  scope: &'s mut v8::HandleScope,
  deferred: bool,
  promise_id: PromiseId,
  mut op: MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>>,
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

  // All ops are polled immediately
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);

  // Note that MaybeDone returns () from the future
  let op_call = match op.poll_unpin(&mut cx) {
    Poll::Pending => {
      let MaybeDone::Future(fut) = op else {
        unreachable!()
      };
      OpCall::pending(ctx, promise_id, fut)
    }
    Poll::Ready(_) => {
      let mut op_result = Pin::new(&mut op).take_output().unwrap();
      // If the op is ready and is not marked as deferred we can immediately return
      // the result.
      if !deferred {
        ctx.state.borrow_mut().tracker.track_async_completed(ctx.id);
        return Some(op_result.to_v8(scope).unwrap());
      }

      OpCall::ready(ctx, promise_id, op_result)
    }
  };

  // Otherwise we will push it to the `pending_ops` and let it be polled again
  // or resolved on the next tick of the event loop.
  ctx
    .context_state
    .borrow_mut()
    .pending_ops
    // SAFETY: this this is guaranteed to be running on a current-thread executor
    .spawn(unsafe { crate::task::MaskFutureAsSend::new(op_call) });
  None
}
