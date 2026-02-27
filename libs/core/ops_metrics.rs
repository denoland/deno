// Copyright 2018-2025 the Deno authors. MIT license.

use crate::OpDecl;
use crate::OpId;
use crate::ops::OpCtx;
use crate::serde::Serialize;
use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;

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

/// Used for both aggregate and per-op metrics.
#[derive(Clone, Default, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpMetricsSummary {
  // The number of ops dispatched synchronously
  pub ops_dispatched_sync: u64,
  // The number of ops dispatched asynchronously
  pub ops_dispatched_async: u64,
  // The number of sync ops dispatched fast
  pub ops_dispatched_fast: u64,
  // The number of asynchronously-dispatch ops completed
  pub ops_completed_async: u64,
}

impl OpMetricsSummary {
  /// Does this op have outstanding async op dispatches?
  pub fn has_outstanding_ops(&self) -> bool {
    self.ops_dispatched_async > self.ops_completed_async
  }
}

#[derive(Default, Debug)]
pub struct OpMetricsSummaryTracker {
  ops: RefCell<Vec<OpMetricsSummary>>,
}

impl OpMetricsSummaryTracker {
  pub fn per_op(&self) -> Ref<'_, Vec<OpMetricsSummary>> {
    self.ops.borrow()
  }

  pub fn aggregate(&self) -> OpMetricsSummary {
    let mut sum = OpMetricsSummary::default();

    for metrics in self.ops.borrow().iter() {
      sum.ops_dispatched_sync += metrics.ops_dispatched_sync;
      sum.ops_dispatched_fast += metrics.ops_dispatched_fast;
      sum.ops_dispatched_async += metrics.ops_dispatched_async;
      sum.ops_completed_async += metrics.ops_completed_async;
    }

    sum
  }

  #[inline]
  fn metrics_mut(&self, id: OpId) -> RefMut<'_, OpMetricsSummary> {
    RefMut::map(self.ops.borrow_mut(), |ops| &mut ops[id as usize])
  }

  /// Returns a [`OpMetricsFn`] for this tracker.
  fn op_metrics_fn(self: Rc<Self>) -> OpMetricsFn {
    Rc::new(move |ctx, event, source| match event {
      OpMetricsEvent::Dispatched => {
        let mut m = self.metrics_mut(ctx.id);
        if source == OpMetricsSource::Fast {
          m.ops_dispatched_fast += 1;
        }
        if ctx.decl.is_async {
          m.ops_dispatched_async += 1;
        } else {
          m.ops_dispatched_sync += 1;
        }
      }
      OpMetricsEvent::Completed
      | OpMetricsEvent::Error
      | OpMetricsEvent::CompletedAsync
      | OpMetricsEvent::ErrorAsync => {
        if ctx.decl.is_async {
          self.metrics_mut(ctx.id).ops_completed_async += 1;
        }
      }
    })
  }

  /// Retrieves the metrics factory function for this tracker.
  pub fn op_metrics_factory_fn(
    self: Rc<Self>,
    op_enabled: impl Fn(&OpDecl) -> bool + 'static,
  ) -> OpMetricsFactoryFn {
    Box::new(move |_, total, op| {
      let mut ops = self.ops.borrow_mut();
      if ops.capacity() == 0 {
        ops.reserve_exact(total);
      }
      ops.push(OpMetricsSummary::default());
      if op_enabled(op) {
        Some(self.clone().op_metrics_fn())
      } else {
        None
      }
    })
  }
}
