// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PerfHooksError {
  #[class(generic)]
  #[error(transparent)]
  TokioEld(#[from] tokio_eld::Error),
}

/// Returns uv metrics info as (loop_count, events, events_waiting).
/// Returns (0, 0, 0) if no uv loop is registered.
#[op2]
#[serde]
pub fn op_node_uv_metrics_info(state: &mut OpState) -> (u64, u64, u64) {
  let Some(uv_loop) = state.try_borrow::<Box<deno_core::uv_compat::UvLoop>>()
  else {
    return (0, 0, 0);
  };
  let loop_ptr: *const deno_core::uv_compat::UvLoop = &**uv_loop as *const _;
  // SAFETY: loop_ptr is valid; it points to the UvLoop stored in OpState.
  unsafe { deno_core::uv_compat::uv_loop_metrics_info(loop_ptr) }
}

pub struct EldHistogram {
  eld: RefCell<tokio_eld::EldHistogram<u64>>,
  started: Cell<bool>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for EldHistogram {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"EldHistogram"
  }
}

#[op2]
impl EldHistogram {
  // Creates an interval EldHistogram object that samples and reports the event
  // loop delay over time.
  //
  // The delays will be reported in nanoseconds.
  #[constructor]
  #[cppgc]
  pub fn new(#[smi] resolution: u32) -> Result<EldHistogram, PerfHooksError> {
    Ok(EldHistogram {
      eld: RefCell::new(tokio_eld::EldHistogram::new(resolution as usize)?),
      started: Cell::new(false),
    })
  }

  // Disables the update interval timer.
  //
  // Returns true if the timer was stopped, false if it was already stopped.
  #[fast]
  fn enable(&self) -> bool {
    if self.started.get() {
      return false;
    }

    self.eld.borrow().start();
    self.started.set(true);

    true
  }

  // Enables the update interval timer.
  //
  // Returns true if the timer was started, false if it was already started.
  #[fast]
  fn disable(&self) -> bool {
    if !self.started.get() {
      return false;
    }

    self.eld.borrow().stop();
    self.started.set(false);

    true
  }

  #[fast]
  fn reset(&self) {
    self.eld.borrow_mut().reset();
  }

  // Returns the value at the given percentile.
  //
  // `percentile` ∈ (0, 100]
  #[fast]
  #[number]
  fn percentile(&self, percentile: f64) -> u64 {
    self.eld.borrow().value_at_percentile(percentile)
  }

  // Returns the value at the given percentile as a bigint.
  #[fast]
  #[bigint]
  fn percentile_big_int(&self, percentile: f64) -> u64 {
    self.eld.borrow().value_at_percentile(percentile)
  }

  // The number of samples recorded by the histogram.
  #[getter]
  #[number]
  fn count(&self) -> u64 {
    self.eld.borrow().len()
  }

  // The number of samples recorded by the histogram as a bigint.
  #[getter]
  #[bigint]
  fn count_big_int(&self) -> u64 {
    self.eld.borrow().len()
  }

  // The maximum recorded event loop delay.
  #[getter]
  #[number]
  fn max(&self) -> u64 {
    self.eld.borrow().max()
  }

  // The maximum recorded event loop delay as a bigint.
  #[getter]
  #[bigint]
  fn max_big_int(&self) -> u64 {
    self.eld.borrow().max()
  }

  // The mean of the recorded event loop delays.
  #[getter]
  fn mean(&self) -> f64 {
    self.eld.borrow().mean()
  }

  // The minimum recorded event loop delay.
  #[getter]
  #[number]
  fn min(&self) -> u64 {
    self.eld.borrow().min()
  }

  // The minimum recorded event loop delay as a bigint.
  #[getter]
  #[bigint]
  fn min_big_int(&self) -> u64 {
    self.eld.borrow().min()
  }

  // The standard deviation of the recorded event loop delays.
  #[getter]
  fn stddev(&self) -> f64 {
    self.eld.borrow().stdev()
  }
}
