// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::parking_lot::Mutex;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PerfHooksError {
  #[class(generic)]
  #[error(transparent)]
  TokioEld(#[from] tokio_eld::Error),
}

pub struct EldHistogram {
  eld: Mutex<tokio_eld::EldHistogram<u64>>,
  started: Mutex<bool>,
}

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
      eld: Mutex::new(tokio_eld::EldHistogram::new(resolution as usize)?),
      started: Mutex::new(false),
    })
  }

  // Disables the update interval timer.
  //
  // Returns true if the timer was stopped, false if it was already stopped.
  #[fast]
  fn enable(&self) -> bool {
    let mut started = self.started.lock();
    if *started {
      return false;
    }

    self.eld.lock().start();
    *started = true;

    true
  }

  // Enables the update interval timer.
  //
  // Returns true if the timer was started, false if it was already started.
  #[fast]
  fn disable(&self) -> bool {
    let mut started = self.started.lock();
    if !*started {
      return false;
    }

    self.eld.lock().stop();
    *started = false;

    true
  }

  #[fast]
  fn reset(&self) {
    self.eld.lock().reset();
  }

  // Returns the value at the given percentile.
  //
  // `percentile` ∈ (0, 100]
  #[fast]
  #[number]
  fn percentile(&self, percentile: f64) -> u64 {
    self.eld.lock().value_at_percentile(percentile)
  }

  // Returns the value at the given percentile as a bigint.
  #[fast]
  #[bigint]
  fn percentile_big_int(&self, percentile: f64) -> u64 {
    self.eld.lock().value_at_percentile(percentile)
  }

  // The number of samples recorded by the histogram.
  #[getter]
  #[number]
  fn count(&self) -> u64 {
    self.eld.lock().len()
  }

  // The number of samples recorded by the histogram as a bigint.
  #[getter]
  #[bigint]
  fn count_big_int(&self) -> u64 {
    self.eld.lock().len()
  }

  // The maximum recorded event loop delay.
  #[getter]
  #[number]
  fn max(&self) -> u64 {
    self.eld.lock().max()
  }

  // The maximum recorded event loop delay as a bigint.
  #[getter]
  #[bigint]
  fn max_big_int(&self) -> u64 {
    self.eld.lock().max()
  }

  // The mean of the recorded event loop delays.
  #[getter]
  fn mean(&self) -> f64 {
    self.eld.lock().mean()
  }

  // The minimum recorded event loop delay.
  #[getter]
  #[number]
  fn min(&self) -> u64 {
    self.eld.lock().min()
  }

  // The minimum recorded event loop delay as a bigint.
  #[getter]
  #[bigint]
  fn min_big_int(&self) -> u64 {
    self.eld.lock().min()
  }

  // The standard deviation of the recorded event loop delays.
  #[getter]
  fn stddev(&self) -> f64 {
    self.eld.lock().stdev()
  }
}
