// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use deno_core::GarbageCollected;

pub struct EldHistogram {
  eld: tokio_eld::EldHistogram<u64>,
}

impl GarbageCollected for EldHistogram {}

#[op2]
impl EldHistogram {
  // Creates an interval EldHistogram object that samples and reports the event
  // loop delay over time.
  //
  // The delays will be reported in nanoseconds.
  #[constructor]
  #[cppgc]
  pub fn new(#[smi] resolution: u32) -> EldHistogram {
    EldHistogram {
      eld: tokio_eld::EldHistogram::new(resolution as usize).unwrap(),
    }
  }

  // Disables the update interval timer.
  //
  // Returns true if the timer was stopped, false if it was already stopped.
  #[fast]
  fn enable(&self) -> bool {
    self.eld.start();

    true
  }

  // Enables the update interval timer.
  //
  // Returns true if the timer was started, false if it was already started.
  #[fast]
  fn disable(&self) -> bool {
    self.eld.stop();

    true
  }

  // Returns the value at the given percentile.
  //
  // `percentile` ∈ (0, 100]
  #[fast]
  #[number]
  fn percentile(&self, percentile: f64) -> u64 {
    self.eld.value_at_percentile(percentile)
  }

  // Returns the value at the given percentile as a bigint.
  #[fast]
  #[bigint]
  fn percentile_bigint(&self, percentile: f64) -> u64 {
    self.eld.value_at_percentile(percentile)
  }

  // Resets the collected histogram data.
  #[fast]
  fn reset(&self) {}

  // getters

  // The number of samples recorded by the histogram.
  #[fast]
  #[number]
  fn count_(&self) -> u64 {
    self.eld.len()
  }

  // The number of samples recorded by the histogram as a bigint.
  #[fast]
  #[bigint]
  fn count_bigint(&self) -> u64 {
    self.eld.len()
  }

  // The maximum recorded event loop delay.
  #[fast]
  #[number]
  fn max(&self) -> u64 {
    self.eld.max()
  }

  // The maximum recorded event loop delay as a bigint.
  #[fast]
  #[bigint]
  fn max_bigint(&self) -> u64 {
    self.eld.max()
  }

  // The mean of the recorded event loop delays.
  #[fast]
  fn mean(&self) -> f64 {
    self.eld.mean()
  }

  // The minimum recorded event loop delay.
  #[fast]
  #[number]
  fn min(&self) -> u64 {
    self.eld.min()
  }

  // The minimum recorded event loop delay as a bigint.
  #[fast]
  #[bigint]
  fn min_bigint(&self) -> u64 {
    self.eld.min()
  }

  // The standard deviation of the recorded event loop delays.
  #[fast]
  fn stddev(&self) -> f64 {
    self.eld.stddev()
  }
}
