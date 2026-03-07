// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::OpDecl;
use crate::OpId;
use crate::ops::OpCtx;
use crate::serde::Serialize;

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

// TODO(mmastrac): this would be better as a trait
/// A callback to retrieve an optional [`OpMetricsFn`] for this op.
pub type OpMetricsFactoryFn =
  Box<dyn Fn(OpId, usize, &OpDecl) -> Option<OpMetricsFn>>;

/// Per-op metrics counters using `Cell` for interior mutability without
/// `RefCell` overhead. These are directly incremented from generated op
/// dispatch code, avoiding `Rc<dyn Fn>` vtable dispatch.
#[derive(Debug, Default)]
pub struct OpMetricsCells {
  pub ops_dispatched_sync: Cell<u64>,
  pub ops_dispatched_async: Cell<u64>,
  pub ops_dispatched_fast: Cell<u64>,
  pub ops_completed_async: Cell<u64>,
}

impl OpMetricsCells {
  #[inline(always)]
  fn increment(&self, cell: &Cell<u64>) {
    cell.set(cell.get() + 1);
  }

  pub fn to_summary(&self) -> OpMetricsSummary {
    OpMetricsSummary {
      ops_dispatched_sync: self.ops_dispatched_sync.get(),
      ops_dispatched_async: self.ops_dispatched_async.get(),
      ops_dispatched_fast: self.ops_dispatched_fast.get(),
      ops_completed_async: self.ops_completed_async.get(),
    }
  }
}

#[doc(hidden)]
#[inline(always)]
fn dispatch_metrics_common(
  opctx: &OpCtx,
  event: OpMetricsEvent,
  source: OpMetricsSource,
) {
  // Direct counter update (no vtable, no RefCell)
  if let Some(cells) = &opctx.metrics_cells {
    match event {
      OpMetricsEvent::Dispatched => {
        if source == OpMetricsSource::Fast {
          cells.increment(&cells.ops_dispatched_fast);
        }
        if opctx.decl.is_async {
          cells.increment(&cells.ops_dispatched_async);
        } else {
          cells.increment(&cells.ops_dispatched_sync);
        }
      }
      OpMetricsEvent::Completed
      | OpMetricsEvent::Error
      | OpMetricsEvent::CompletedAsync
      | OpMetricsEvent::ErrorAsync => {
        if opctx.decl.is_async {
          cells.increment(&cells.ops_completed_async);
        }
      }
    }
  }
  // Optional trace callback (rare, for --trace-ops)
  if let Some(f) = &opctx.trace_ops_fn {
    f(opctx, event, source);
  }
}

#[doc(hidden)]
pub fn dispatch_metrics_fast(opctx: &OpCtx, metrics: OpMetricsEvent) {
  dispatch_metrics_common(opctx, metrics, OpMetricsSource::Fast);
}

#[doc(hidden)]
pub fn dispatch_metrics_slow(opctx: &OpCtx, metrics: OpMetricsEvent) {
  dispatch_metrics_common(opctx, metrics, OpMetricsSource::Slow);
}

#[doc(hidden)]
pub fn dispatch_metrics_async(opctx: &OpCtx, metrics: OpMetricsEvent) {
  dispatch_metrics_common(opctx, metrics, OpMetricsSource::Async);
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
  ops: RefCell<Vec<Rc<OpMetricsCells>>>,
}

impl OpMetricsSummaryTracker {
  pub fn per_op(&self) -> Vec<OpMetricsSummary> {
    self.ops.borrow().iter().map(|c| c.to_summary()).collect()
  }

  pub fn aggregate(&self) -> OpMetricsSummary {
    let mut sum = OpMetricsSummary::default();
    for cells in self.ops.borrow().iter() {
      let s = cells.to_summary();
      sum.ops_dispatched_sync += s.ops_dispatched_sync;
      sum.ops_dispatched_fast += s.ops_dispatched_fast;
      sum.ops_dispatched_async += s.ops_dispatched_async;
      sum.ops_completed_async += s.ops_completed_async;
    }
    sum
  }

  /// Creates an [`Rc<OpMetricsCells>`] for the given op and registers
  /// it with this tracker. Returns `None` if the op is not enabled.
  pub fn op_metrics_cells_fn(
    self: &Rc<Self>,
    op_enabled: impl Fn(&OpDecl) -> bool + 'static,
  ) -> OpMetricsCellsFactoryFn {
    let this = self.clone();
    Box::new(move |_, _, op| {
      let cells = Rc::new(OpMetricsCells::default());
      this.ops.borrow_mut().push(cells.clone());
      if op_enabled(op) { Some(cells) } else { None }
    })
  }
}

/// Factory function that creates per-op metrics cells.
pub type OpMetricsCellsFactoryFn =
  Box<dyn Fn(OpId, usize, &OpDecl) -> Option<Rc<OpMetricsCells>>>;

