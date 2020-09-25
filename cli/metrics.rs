// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#[derive(Default, Debug)]
pub struct Metrics {
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

impl Metrics {
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

use deno_core::BufVec;
use deno_core::Op;
use deno_core::OpFn;
use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;

pub fn metrics_op(op_fn: Box<OpFn>) -> Box<OpFn> {
  Box::new(move |op_state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
    // TODOs:
    // * The 'bytes' metrics seem pretty useless, especially now that the
    //   distinction between 'control' and 'data' buffers has become blurry.
    // * Tracking completion of async ops currently makes us put the boxed
    //   future into _another_ box. Keeping some counters may not be expensive
    //   in itself, but adding a heap allocation for every metric seems bad.
    let mut buf_len_iter = bufs.iter().map(|buf| buf.len());
    let bytes_sent_control = buf_len_iter.next().unwrap_or(0);
    let bytes_sent_data = buf_len_iter.sum();

    let op = (op_fn)(op_state.clone(), bufs);

    let op_state_ = op_state.clone();
    let mut s = op_state.borrow_mut();
    let metrics = s.borrow_mut::<Metrics>();

    use deno_core::futures::future::FutureExt;

    match op {
      Op::Sync(buf) => {
        metrics.op_sync(bytes_sent_control, bytes_sent_data, buf.len());
        Op::Sync(buf)
      }
      Op::Async(fut) => {
        metrics.op_dispatched_async(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |buf| {
            let mut s = op_state_.borrow_mut();
            let metrics = s.borrow_mut::<Metrics>();
            metrics.op_completed_async(buf.len());
          })
          .boxed_local();
        Op::Async(fut)
      }
      Op::AsyncUnref(fut) => {
        metrics.op_dispatched_async_unref(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |buf| {
            let mut s = op_state_.borrow_mut();
            let metrics = s.borrow_mut::<Metrics>();
            metrics.op_completed_async_unref(buf.len());
          })
          .boxed_local();
        Op::AsyncUnref(fut)
      }
      other => other,
    }
  })
}
