// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#[derive(Default)]
pub struct Metrics {
  pub ops_dispatched: u64,
  pub ops_completed: u64,
  pub bytes_sent_control: u64,
  pub bytes_sent_data: u64,
  pub bytes_received: u64,
  pub resolve_count: u64,
}
