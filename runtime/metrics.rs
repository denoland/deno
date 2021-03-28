// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::serde::Serialize;

#[derive(Default, Debug)]
pub struct RuntimeMetrics {
  pub ops: HashMap<&'static str, OpMetrics>,
}

impl RuntimeMetrics {
  pub fn combined_metrics(&self) -> OpMetrics {
    let mut total = OpMetrics::default();

    for metrics in self.ops.values() {
      total.ops_dispatched += metrics.ops_dispatched;
      total.ops_dispatched_sync += metrics.ops_dispatched_sync;
      total.ops_dispatched_async += metrics.ops_dispatched_async;
      total.ops_dispatched_async_unref += metrics.ops_dispatched_async_unref;
      total.ops_completed += metrics.ops_completed;
      total.ops_completed_sync += metrics.ops_completed_sync;
      total.ops_completed_async += metrics.ops_completed_async;
      total.ops_completed_async_unref += metrics.ops_completed_async_unref;
      total.bytes_sent_control += metrics.bytes_sent_control;
      total.bytes_sent_data += metrics.bytes_sent_data;
      total.bytes_received += metrics.bytes_received;
    }

    total
  }
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpMetrics {
  pub ops_dispatched: u64,
  pub ops_dispatched_sync: u64,
  pub ops_dispatched_async: u64,
  pub ops_dispatched_async_unref: u64,
  pub ops_completed: u64,
  pub ops_completed_sync: u64,
  pub ops_completed_async: u64,
  pub ops_completed_async_unref: u64,
  pub bytes_sent_control: u64,
  pub bytes_sent_data: u64,
  pub bytes_received: u64,
}

impl OpMetrics {
  fn op_dispatched(
    &mut self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.ops_dispatched += 1;
    self.bytes_sent_control += bytes_sent_control as u64;
    self.bytes_sent_data += bytes_sent_data as u64;
  }

  fn op_completed(&mut self, bytes_received: usize) {
    self.ops_completed += 1;
    self.bytes_received += bytes_received as u64;
  }

  pub fn op_sync(
    &mut self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
    bytes_received: usize,
  ) {
    self.ops_dispatched_sync += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data);
    self.ops_completed_sync += 1;
    self.op_completed(bytes_received);
  }

  pub fn op_dispatched_async(
    &mut self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.ops_dispatched_async += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data)
  }

  pub fn op_dispatched_async_unref(
    &mut self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.ops_dispatched_async_unref += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data)
  }

  pub fn op_completed_async(&mut self, bytes_received: usize) {
    self.ops_completed_async += 1;
    self.op_completed(bytes_received);
  }

  pub fn op_completed_async_unref(&mut self, bytes_received: usize) {
    self.ops_completed_async_unref += 1;
    self.op_completed(bytes_received);
  }
}

use deno_core::Op;
use deno_core::OpFn;
use std::collections::HashMap;

pub fn metrics_op(name: &'static str, op_fn: Box<OpFn>) -> Box<OpFn> {
  Box::new(move |op_state, payload, buf| -> Op {
    // TODOs:
    // * The 'bytes' metrics seem pretty useless, especially now that the
    //   distinction between 'control' and 'data' buffers has become blurry.
    // * Tracking completion of async ops currently makes us put the boxed
    //   future into _another_ box. Keeping some counters may not be expensive
    //   in itself, but adding a heap allocation for every metric seems bad.

    // TODO: remove this, doesn't make a ton of sense
    let bytes_sent_control = 0;
    let bytes_sent_data = match buf {
      Some(ref b) => b.len(),
      None => 0,
    };

    let op = (op_fn)(op_state.clone(), payload, buf);

    let op_state_ = op_state.clone();
    let mut s = op_state.borrow_mut();
    let runtime_metrics = s.borrow_mut::<RuntimeMetrics>();

    let metrics = if let Some(metrics) = runtime_metrics.ops.get_mut(name) {
      metrics
    } else {
      runtime_metrics.ops.insert(name, OpMetrics::default());
      runtime_metrics.ops.get_mut(name).unwrap()
    };

    use deno_core::futures::future::FutureExt;

    match op {
      Op::Sync(buf) => {
        metrics.op_sync(bytes_sent_control, bytes_sent_data, 0);
        Op::Sync(buf)
      }
      Op::Async(fut) => {
        metrics.op_dispatched_async(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |_resp| {
            let mut s = op_state_.borrow_mut();
            let runtime_metrics = s.borrow_mut::<RuntimeMetrics>();
            let metrics = runtime_metrics.ops.get_mut(name).unwrap();
            metrics.op_completed_async(0);
          })
          .boxed_local();
        Op::Async(fut)
      }
      Op::AsyncUnref(fut) => {
        metrics.op_dispatched_async_unref(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |_resp| {
            let mut s = op_state_.borrow_mut();
            let runtime_metrics = s.borrow_mut::<RuntimeMetrics>();
            let metrics = runtime_metrics.ops.get_mut(name).unwrap();
            metrics.op_completed_async_unref(0);
          })
          .boxed_local();
        Op::AsyncUnref(fut)
      }
      other => other,
    }
  })
}
