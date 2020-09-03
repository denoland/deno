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
  pub resolve_count: u64,
}

impl Metrics {
  fn op_dispatched(&mut self, bytes_sent_control: u64, bytes_sent_data: u64) {
    self.ops_dispatched += 1;
    self.bytes_sent_control += bytes_sent_control;
    self.bytes_sent_data += bytes_sent_data;
  }

  fn op_completed(&mut self, bytes_received: u64) {
    self.ops_completed += 1;
    self.bytes_received += bytes_received;
  }

  pub fn op_sync(
    &mut self,
    bytes_sent_control: u64,
    bytes_sent_data: u64,
    bytes_received: u64,
  ) {
    self.ops_dispatched_sync += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data);
    self.ops_completed_sync += 1;
    self.op_completed(bytes_received);
  }

  pub fn op_dispatched_async(
    &mut self,
    bytes_sent_control: u64,
    bytes_sent_data: u64,
  ) {
    self.ops_dispatched_async += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data)
  }

  pub fn op_dispatched_async_unref(
    &mut self,
    bytes_sent_control: u64,
    bytes_sent_data: u64,
  ) {
    self.ops_dispatched_async_unref += 1;
    self.op_dispatched(bytes_sent_control, bytes_sent_data)
  }

  pub fn op_completed_async(&mut self, bytes_received: u64) {
    self.ops_completed_async += 1;
    self.op_completed(bytes_received);
  }

  pub fn op_completed_async_unref(&mut self, bytes_received: u64) {
    self.ops_completed_async_unref += 1;
    self.op_completed(bytes_received);
  }
}
