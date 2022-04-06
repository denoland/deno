// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use crate::serde::Serialize;
use crate::OpName;
use std::cell::UnsafeCell;
use std::collections::HashMap;

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
#[derive(Debug)]
pub struct OpsTracker {
  pub ops: UnsafeCell<HashMap<OpName, OpMetrics>>,
}

impl OpsTracker {
  pub fn new(op_names: impl IntoIterator<Item = OpName>) -> Self {
    let map = op_names
      .into_iter()
      .map(|name| (name, OpMetrics::default()))
      .collect();
    Self {
      ops: UnsafeCell::new(map),
    }
  }

  pub fn per_op(&self) -> HashMap<OpName, OpMetrics> {
    self.ops_mut().clone()
  }

  pub fn aggregate(&self) -> OpMetrics {
    let mut sum = OpMetrics::default();

    for metrics in self.ops_mut().values() {
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

  #[allow(clippy::mut_from_ref)]
  #[inline]
  fn ops_mut(&self) -> &mut HashMap<&'static str, OpMetrics> {
    unsafe { &mut *self.ops.get() }
  }

  #[allow(clippy::mut_from_ref)]
  #[inline]
  fn metrics_mut(&self, op_name: OpName) -> &mut OpMetrics {
    self.ops_mut().get_mut(op_name).unwrap()
  }

  #[inline]
  pub fn track_sync(&self, op_name: OpName) {
    let metrics = self.metrics_mut(op_name);
    metrics.ops_dispatched += 1;
    metrics.ops_completed += 1;
    metrics.ops_dispatched_sync += 1;
    metrics.ops_completed_sync += 1;
  }

  #[inline]
  pub fn track_async(&self, op_name: OpName) {
    let metrics = self.metrics_mut(op_name);
    metrics.ops_dispatched += 1;
    metrics.ops_dispatched_async += 1;
  }

  #[inline]
  pub fn track_async_completed(&self, op_name: OpName) {
    let metrics = self.metrics_mut(op_name);
    metrics.ops_completed += 1;
    metrics.ops_completed_async += 1;
  }
}
