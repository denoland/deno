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

/// A callback to receive an [`OpMetricsEvent`].
pub type OpMetricsFn = Rc<dyn Fn(&OpCtx, OpMetricsEvent, OpMetricsSource)>;

// TODO(bartlomieju): this would be better as a trait
/// A callback to retrieve an optional [`OpMetricsFn`] for this op.
pub type OpMetricsFactoryFn =
  Box<dyn Fn(OpId, usize, &OpDecl) -> Option<OpMetricsFn>>;

#[doc(hidden)]
pub fn dispatch_metrics_fast(opctx: &OpCtx, metrics: OpMetricsEvent) {
  if let Some(f) = &opctx.metrics_fn {
    f(opctx, metrics, OpMetricsSource::Fast);
  }
}

#[doc(hidden)]
pub fn dispatch_metrics_slow(opctx: &OpCtx, metrics: OpMetricsEvent) {
  if let Some(f) = &opctx.metrics_fn {
    f(opctx, metrics, OpMetricsSource::Slow);
  }
}

#[doc(hidden)]
pub fn dispatch_metrics_async(opctx: &OpCtx, metrics: OpMetricsEvent) {
  if let Some(f) = &opctx.metrics_fn {
    f(opctx, metrics, OpMetricsSource::Async);
  }
}
