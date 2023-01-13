// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::serde::Serialize;
use crate::OpId;
use std::cell::{RefCell, RefMut};

// TODO(@AaronO): split into AggregateMetrics & PerOpMetrics
#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpMetrics {
  pub ops_dispatched: u64,
  pub ops_dispatched_sync: u64,
  pub ops_dispatched_async: u64,
  // TODO(bartlomieju): this field is never updated
  pub ops_dispatched_async_unref: u64,
  pub ops_completed: u64,
  pub ops_completed_sync: u64,
  pub ops_completed_async: u64,
  // TODO(bartlomieju): this field is never updated
  pub ops_completed_async_unref: u64,
  pub bytes_sent_control: u64,
  pub bytes_sent_data: u64,
  pub bytes_received: u64,
}

// TODO(@AaronO): track errors
#[derive(Default, Debug)]
pub struct OpsTracker {
  ops: RefCell<Vec<OpMetrics>>,
}

impl OpsTracker {
  pub fn new(ops_count: usize) -> Self {
    Self {
      ops: RefCell::new(vec![Default::default(); ops_count]),
    }
  }

  pub fn per_op(&self) -> Vec<OpMetrics> {
    self.ops.borrow().clone()
  }

  pub fn aggregate(&self) -> OpMetrics {
    let mut sum = OpMetrics::default();

    for metrics in self.ops.borrow().iter() {
      sum.ops_dispatched += metrics.ops_dispatched;
      sum.ops_dispatched_sync += metrics.ops_dispatched_sync;
      sum.ops_dispatched_async += metrics.ops_dispatched_async;
      sum.ops_dispatched_async_unref += metrics.ops_dispatched_async_unref;
      sum.ops_completed += metrics.ops_completed;
      sum.ops_completed_sync += metrics.ops_completed_sync;
      sum.ops_completed_async += metrics.ops_completed_async;
      sum.ops_completed_async_unref += metrics.ops_completed_async_unref;
      sum.bytes_sent_control += metrics.bytes_sent_control;
      sum.bytes_sent_data += metrics.bytes_sent_data;
      sum.bytes_received += metrics.bytes_received;
    }

    sum
  }

  #[inline]
  fn metrics_mut(&self, id: OpId) -> RefMut<OpMetrics> {
    RefMut::map(self.ops.borrow_mut(), |ops| &mut ops[id])
  }

  #[inline]
  pub fn track_sync(&self, id: OpId) {
    let mut metrics = self.metrics_mut(id);
    metrics.ops_dispatched += 1;
    metrics.ops_completed += 1;
    metrics.ops_dispatched_sync += 1;
    metrics.ops_completed_sync += 1;
  }

  #[inline]
  pub fn track_async(&self, id: OpId) {
    let mut metrics = self.metrics_mut(id);
    metrics.ops_dispatched += 1;
    metrics.ops_dispatched_async += 1;
  }

  #[inline]
  pub fn track_async_completed(&self, id: OpId) {
    let mut metrics = self.metrics_mut(id);
    metrics.ops_completed += 1;
    metrics.ops_completed_async += 1;
  }
}
