// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::op2;
use hdrhistogram::Histogram;

const EMPTY_HISTOGRAM_MIN: u64 = i64::MAX as u64;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PerfHooksError {
  #[class(generic)]
  #[error(transparent)]
  TokioEld(#[from] tokio_eld::Error),
  #[class(generic)]
  #[error(transparent)]
  HistogramCreation(#[from] hdrhistogram::errors::CreationError),
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

// Backs the user-facing `RecordableHistogram` returned by
// `perf_hooks.createHistogram()`. Wraps an `hdrhistogram::Histogram<u64>`
// configured with caller-supplied bounds, plus the bookkeeping needed for
// `recordDelta()` and the `exceeds` counter (incremented when a recorded
// value overflows the histogram's `highest` bound).
pub struct BaseHistogram {
  inner: RefCell<Histogram<u64>>,
  highest: u64,
  exceeds: Cell<u64>,
  added_out_of_range: Cell<u64>,
  prev_delta_ns: Cell<Option<u64>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for BaseHistogram {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"BaseHistogram"
  }
}

fn now_ns() -> u64 {
  // Match Node's `process.hrtime` clock domain — monotonic nanoseconds.
  use std::time::Instant;
  thread_local! {
    static ORIGIN: Instant = Instant::now();
  }
  ORIGIN.with(|origin| origin.elapsed().as_nanos() as u64)
}

#[op2]
impl BaseHistogram {
  // Creates a `RecordableHistogram` with the given bounds and significant
  // figures. Mirrors the behavior of Node's `createHistogram(options)`.
  //
  // Caller is responsible for validating bounds; this just forwards them to
  // `hdrhistogram::Histogram::new_with_bounds`.
  #[constructor]
  #[cppgc]
  pub fn new(
    #[bigint] lowest: u64,
    #[bigint] highest: u64,
    #[smi] figures: u32,
  ) -> Result<BaseHistogram, PerfHooksError> {
    let inner =
      Histogram::<u64>::new_with_bounds(lowest, highest, figures as u8)?;
    Ok(BaseHistogram {
      inner: RefCell::new(inner),
      highest,
      exceeds: Cell::new(0),
      added_out_of_range: Cell::new(0),
      prev_delta_ns: Cell::new(None),
    })
  }

  // Records a value into the histogram. If the value exceeds the configured
  // `highest`, increments the `exceeds` counter instead of erroring.
  #[fast]
  fn record(&self, #[bigint] value: u64) {
    if value > self.highest {
      self.exceeds.set(self.exceeds.get().saturating_add(1));
      return;
    }
    let mut h = self.inner.borrow_mut();
    if h.record(value).is_err() {
      self.exceeds.set(self.exceeds.get().saturating_add(1));
    }
  }

  // Records the nanoseconds elapsed since the previous call to recordDelta.
  // The first call seeds the timestamp without recording (matches Node).
  #[fast]
  fn record_delta(&self) {
    let now = now_ns();
    if let Some(prev) = self.prev_delta_ns.get() {
      let delta = now.saturating_sub(prev);
      if delta > self.highest {
        self.exceeds.set(self.exceeds.get().saturating_add(1));
        self.prev_delta_ns.set(Some(now));
        return;
      }
      let mut h = self.inner.borrow_mut();
      if h.record(delta).is_err() {
        self.exceeds.set(self.exceeds.get().saturating_add(1));
      }
    }
    self.prev_delta_ns.set(Some(now));
  }

  // Adds counts from another histogram into this one.
  #[fast]
  fn add(&self, #[cppgc] other: &BaseHistogram) {
    let other_h = other.inner.borrow();
    let mut h = self.inner.borrow_mut();
    let mut added_out_of_range = self.added_out_of_range.get();
    for v in other_h.iter_recorded() {
      if v.value_iterated_to() > self.highest
        || h
          .record_n(v.value_iterated_to(), v.count_at_value())
          .is_err()
      {
        added_out_of_range =
          added_out_of_range.saturating_add(v.count_at_value());
      }
    }
    self.added_out_of_range.set(added_out_of_range);
    self
      .exceeds
      .set(self.exceeds.get().saturating_add(other.exceeds.get()));
  }

  #[fast]
  fn reset(&self) {
    self.inner.borrow_mut().reset();
    self.exceeds.set(0);
    self.added_out_of_range.set(0);
    self.prev_delta_ns.set(None);
  }

  #[fast]
  #[number]
  fn percentile(&self, percentile: f64) -> u64 {
    self.inner.borrow().value_at_percentile(percentile)
  }

  #[fast]
  #[bigint]
  fn percentile_big_int(&self, percentile: f64) -> u64 {
    self.inner.borrow().value_at_percentile(percentile)
  }

  // Returns the percentile distribution as a flat `[percentile, value, ...]`
  // array. The JS layer turns it into a `Map`. We iterate the recorded values
  // and emit one entry per distinct value.
  //
  // Values are bounded by `highest` (validated to fit in a JS safe integer by
  // the createHistogram caller), so emitting them as f64 is lossless.
  #[serde]
  fn percentiles(&self) -> Vec<f64> {
    let h = self.inner.borrow();
    let mut out = Vec::new();
    if h.is_empty() {
      out.push(100.0);
      out.push(0.0);
      return out;
    }
    out.push(0.0);
    out.push(h.min() as f64);
    if h.len() > 1 {
      let max = h.max();
      let mut percentile = 50.0;
      while percentile < 100.0 {
        let value = h.value_at_percentile(percentile);
        out.push(percentile);
        out.push(value as f64);
        if value >= max {
          break;
        }
        percentile += (100.0 - percentile) / 2.0;
      }
    }
    out.push(100.0);
    out.push(h.max() as f64);
    out
  }

  // Same shape as `percentiles`; the JS layer re-wraps the value entries as
  // BigInt when exposing them through `percentilesBigInt`.
  #[serde]
  fn percentiles_big_int(&self) -> Vec<f64> {
    let h = self.inner.borrow();
    let mut out = Vec::new();
    if h.is_empty() {
      out.push(100.0);
      out.push(0.0);
      return out;
    }
    out.push(0.0);
    out.push(h.min() as f64);
    if h.len() > 1 {
      let max = h.max();
      let mut percentile = 50.0;
      while percentile < 100.0 {
        let value = h.value_at_percentile(percentile);
        out.push(percentile);
        out.push(value as f64);
        if value >= max {
          break;
        }
        percentile += (100.0 - percentile) / 2.0;
      }
    }
    out.push(100.0);
    out.push(h.max() as f64);
    out
  }

  #[getter]
  #[number]
  fn count(&self) -> u64 {
    self
      .inner
      .borrow()
      .len()
      .saturating_add(self.added_out_of_range.get())
  }

  #[getter]
  #[bigint]
  fn count_big_int(&self) -> u64 {
    self
      .inner
      .borrow()
      .len()
      .saturating_add(self.added_out_of_range.get())
  }

  #[getter]
  #[number]
  fn min(&self) -> u64 {
    let h = self.inner.borrow();
    if h.is_empty() {
      EMPTY_HISTOGRAM_MIN
    } else {
      h.min()
    }
  }

  #[getter]
  #[bigint]
  fn min_big_int(&self) -> u64 {
    let h = self.inner.borrow();
    if h.is_empty() {
      EMPTY_HISTOGRAM_MIN
    } else {
      h.min()
    }
  }

  #[getter]
  #[number]
  fn max(&self) -> u64 {
    self.inner.borrow().max()
  }

  #[getter]
  #[bigint]
  fn max_big_int(&self) -> u64 {
    self.inner.borrow().max()
  }

  #[getter]
  fn mean(&self) -> f64 {
    let h = self.inner.borrow();
    if h.is_empty() { f64::NAN } else { h.mean() }
  }

  #[getter]
  fn stddev(&self) -> f64 {
    let h = self.inner.borrow();
    if h.is_empty() { f64::NAN } else { h.stdev() }
  }

  #[getter]
  #[number]
  fn exceeds(&self) -> u64 {
    self.exceeds.get()
  }

  #[getter]
  #[bigint]
  fn exceeds_big_int(&self) -> u64 {
    self.exceeds.get()
  }
}
