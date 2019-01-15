#![allow(unused_imports)]
#![allow(dead_code)]
#![cfg_attr(
  feature = "cargo-clippy",
  allow(clippy::all, clippy::pedantic)
)]
use flatbuffers;
use std::sync::atomic::Ordering;

// GN_OUT_DIR is set either by build.rs (for the Cargo build), or by
// build_extra/rust/run.py (for the GN+Ninja build).
include!(concat!(env!("GN_OUT_DIR"), "/gen/msg_generated.rs"));

impl<'a> From<&'a super::isolate::Metrics> for MetricsResArgs {
  fn from(m: &'a super::isolate::Metrics) -> Self {
    MetricsResArgs {
      ops_dispatched: m.ops_dispatched.load(Ordering::SeqCst) as u64,
      ops_completed: m.ops_completed.load(Ordering::SeqCst) as u64,
      bytes_sent_control: m.bytes_sent_control.load(Ordering::SeqCst) as u64,
      bytes_sent_data: m.bytes_sent_data.load(Ordering::SeqCst) as u64,
      bytes_received: m.bytes_received.load(Ordering::SeqCst) as u64,
    }
  }
}
