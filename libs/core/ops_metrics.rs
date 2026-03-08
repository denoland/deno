// Copyright 2018-2026 the Deno authors. MIT license.

use std::rc::Rc;

use crate::OpDecl;
use crate::OpId;
use crate::ops::OpCtx;

/// The type of op metrics event.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OpMetricsEvent {
  /// Entered an op dispatch.
  Dispatched,
  /// Left an op synchronously.
  Completed,
  /// Left an op asynchronously.
  CompletedAsync,
  /// Left an op synchronously with an exception.
  Error,
  /// Left an op asynchronously with an exception.
  ErrorAsync,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OpMetricsSource {
  Slow,
  Fast,
  Async,
}

/// A callback to receieve an [`OpMetricsEvent`].
pub type OpMetricsFn = Rc<dyn Fn(&OpCtx, OpMetricsEvent, OpMetricsSource)>;

// TODO(mmastrac): this would be better as a trait
/// A callback to retrieve an optional [`OpMetricsFn`] for this op.
pub type OpMetricsFactoryFn =
  Box<dyn Fn(OpId, usize, &OpDecl) -> Option<OpMetricsFn>>;

/// Given two [`OpMetricsFactoryFn`] implementations, merges them so that op metric events are
/// called on both.
pub fn merge_op_metrics(
  fn1: impl Fn(OpId, usize, &OpDecl) -> Option<OpMetricsFn> + 'static,
  fn2: impl Fn(OpId, usize, &OpDecl) -> Option<OpMetricsFn> + 'static,
) -> OpMetricsFactoryFn {
  Box::new(move |op, count, decl| {
    match (fn1(op, count, decl), fn2(op, count, decl)) {
      (None, None) => None,
      (Some(a), None) => Some(a),
      (None, Some(b)) => Some(b),
      (Some(a), Some(b)) => Some(Rc::new(move |ctx, event, source| {
        a(ctx, event, source);
        b(ctx, event, source);
      })),
    }
  })
}

#[doc(hidden)]
pub fn dispatch_metrics_fast(opctx: &OpCtx, metrics: OpMetricsEvent) {
  // SAFETY: this should only be called from ops where we know the function is Some
  unsafe {
    (opctx.metrics_fn.as_ref().unwrap_unchecked())(
      opctx,
      metrics,
      OpMetricsSource::Fast,
    )
  }
}

#[doc(hidden)]
pub fn dispatch_metrics_slow(opctx: &OpCtx, metrics: OpMetricsEvent) {
  // SAFETY: this should only be called from ops where we know the function is Some
  unsafe {
    (opctx.metrics_fn.as_ref().unwrap_unchecked())(
      opctx,
      metrics,
      OpMetricsSource::Slow,
    )
  }
}

#[doc(hidden)]
pub fn dispatch_metrics_async(opctx: &OpCtx, metrics: OpMetricsEvent) {
  // SAFETY: this should only be called from ops where we know the function is Some
  unsafe {
    (opctx.metrics_fn.as_ref().unwrap_unchecked())(
      opctx,
      metrics,
      OpMetricsSource::Async,
    )
  }
}
