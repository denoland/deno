// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json::json;
use std::cmp;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use super::logging::lsp_debug;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAverage {
  pub name: String,
  pub count: u32,
  pub average_duration: u32,
}

impl PartialOrd for PerformanceAverage {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PerformanceAverage {
  fn cmp(&self, other: &Self) -> cmp::Ordering {
    self.name.cmp(&other.name)
  }
}

/// A structure which serves as a start of a measurement span.
#[derive(Debug)]
pub struct PerformanceMark {
  name: String,
  count: u32,
  start: Instant,
}

/// A structure which holds the information about the measured span.
#[derive(Debug, Clone)]
pub struct PerformanceMeasure {
  pub name: String,
  pub count: u32,
  pub duration: Duration,
}

impl fmt::Display for PerformanceMeasure {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{} ({}ms)",
      self.name,
      self.duration.as_micros() as f64 / 1000.0
    )
  }
}

impl From<PerformanceMark> for PerformanceMeasure {
  fn from(value: PerformanceMark) -> Self {
    Self {
      name: value.name,
      count: value.count,
      duration: value.start.elapsed(),
    }
  }
}

#[derive(Debug)]
pub struct PerformanceScopeMark {
  performance_inner: Arc<Mutex<PerformanceInner>>,
  inner: Option<PerformanceMark>,
}

impl Drop for PerformanceScopeMark {
  fn drop(&mut self) {
    self
      .performance_inner
      .lock()
      .measure(self.inner.take().unwrap());
  }
}

#[derive(Debug)]
struct PerformanceInner {
  counts: HashMap<String, u32>,
  measurements_by_type: HashMap<String, (/* count */ u32, /* duration */ f64)>,
  max_size: usize,
  measures: VecDeque<PerformanceMeasure>,
}

impl PerformanceInner {
  fn measure(&mut self, mark: PerformanceMark) -> Duration {
    let measure = PerformanceMeasure::from(mark);
    lsp_debug!(
      "{},",
      json!({
        "type": "measure",
        "name": measure.name,
        "count": measure.count,
        "duration": measure.duration.as_micros() as f64 / 1000.0,
      })
    );
    let duration = measure.duration;
    let measurement = self
      .measurements_by_type
      .entry(measure.name.to_string())
      .or_insert((0, 0.0));
    measurement.1 += duration.as_micros() as f64 / 1000.0;
    self.measures.push_front(measure);
    while self.measures.len() > self.max_size {
      self.measures.pop_back();
    }
    duration
  }
}

impl Default for PerformanceInner {
  fn default() -> Self {
    Self {
      counts: Default::default(),
      measurements_by_type: Default::default(),
      max_size: 3_000,
      measures: Default::default(),
    }
  }
}

/// A simple structure for marking a start of something to measure the duration
/// of and measuring that duration.  Each measurement is identified by a string
/// name and a counter is incremented each time a new measurement is marked.
///
/// The structure will limit the size of measurements to the most recent 1000,
/// and will roll off when that limit is reached.
#[derive(Debug, Default)]
pub struct Performance(Arc<Mutex<PerformanceInner>>);

impl Performance {
  /// Return the count and average duration of a measurement identified by name.
  #[cfg(test)]
  pub fn average(&self, name: &str) -> Option<(usize, Duration)> {
    let mut items = Vec::new();
    for measure in self.0.lock().measures.iter() {
      if measure.name == name {
        items.push(measure.duration);
      }
    }
    let len = items.len();

    if len > 0 {
      let average = items.into_iter().sum::<Duration>() / len as u32;
      Some((len, average))
    } else {
      None
    }
  }

  /// Return an iterator which provides the names, count, and average duration
  /// of each measurement.
  pub fn averages(&self) -> Vec<PerformanceAverage> {
    let mut averages: HashMap<String, Vec<Duration>> = HashMap::new();
    for measure in self.0.lock().measures.iter() {
      averages
        .entry(measure.name.clone())
        .or_default()
        .push(measure.duration);
    }
    averages
      .into_iter()
      .map(|(k, d)| {
        let count = d.len() as u32;
        let a = d.into_iter().sum::<Duration>() / count;
        PerformanceAverage {
          name: k,
          count,
          average_duration: a.as_millis() as u32,
        }
      })
      .collect()
  }

  pub fn measurements_by_type(&self) -> Vec<(String, u32, f64)> {
    self
      .0
      .lock()
      .measurements_by_type
      .iter()
      .map(|(name, (count, duration))| (name.to_string(), *count, *duration))
      .collect::<Vec<_>>()
  }

  pub fn averages_as_f64(&self) -> Vec<(String, u32, f64)> {
    let mut averages: HashMap<String, Vec<Duration>> = HashMap::new();
    for measure in self.0.lock().measures.iter() {
      averages
        .entry(measure.name.clone())
        .or_default()
        .push(measure.duration);
    }
    averages
      .into_iter()
      .map(|(k, d)| {
        let count = d.len() as u32;
        let a = d.into_iter().sum::<Duration>() / count;
        (k, count, a.as_micros() as f64 / 1000.0)
      })
      .collect()
  }

  fn mark_inner<S: AsRef<str>, V: Serialize>(
    &self,
    name: S,
    maybe_args: Option<V>,
  ) -> PerformanceMark {
    let mut inner = self.0.lock();
    let name = name.as_ref();
    let count = *inner
      .counts
      .entry(name.to_string())
      .and_modify(|c| *c += 1)
      .or_insert(1);
    inner
      .measurements_by_type
      .entry(name.to_string())
      .and_modify(|(c, _)| *c += 1)
      .or_insert((1, 0.0));
    let msg = if let Some(args) = maybe_args {
      json!({
        "type": "mark",
        "name": name,
        "count": count,
        "args": args,
      })
    } else {
      json!({
        "type": "mark",
        "name": name,
      })
    };
    lsp_debug!("{},", msg);
    PerformanceMark {
      name: name.to_string(),
      count,
      start: Instant::now(),
    }
  }

  /// Marks the start of a measurement which returns a performance mark
  /// structure, which is then passed to `.measure()` to finalize the duration
  /// and add it to the internal buffer.
  pub fn mark<S: AsRef<str>>(&self, name: S) -> PerformanceMark {
    self.mark_inner(name, None::<()>)
  }

  /// Marks the start of a measurement which returns a performance mark
  /// structure, which is then passed to `.measure()` to finalize the duration
  /// and add it to the internal buffer.
  pub fn mark_with_args<S: AsRef<str>, V: Serialize>(
    &self,
    name: S,
    args: V,
  ) -> PerformanceMark {
    self.mark_inner(name, Some(args))
  }

  /// Creates a performance mark which will be measured against on drop. Use
  /// like this:
  /// ```rust
  /// let _mark = self.performance.measure_scope("foo");
  /// ```
  /// Don't use like this:
  /// ```rust
  /// // ❌
  /// let _ = self.performance.measure_scope("foo");
  /// ```
  pub fn measure_scope<S: AsRef<str>>(&self, name: S) -> PerformanceScopeMark {
    PerformanceScopeMark {
      performance_inner: self.0.clone(),
      inner: Some(self.mark(name)),
    }
  }

  /// A function which accepts a previously created performance mark which will
  /// be used to finalize the duration of the span being measured, and add the
  /// measurement to the internal buffer.
  pub fn measure(&self, mark: PerformanceMark) -> Duration {
    self.0.lock().measure(mark)
  }

  pub fn to_vec(&self) -> Vec<PerformanceMeasure> {
    self.0.lock().measures.iter().cloned().collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_average() {
    let performance = Performance::default();
    let mark1 = performance.mark("a");
    let mark2 = performance.mark("a");
    let mark3 = performance.mark("b");
    performance.measure(mark2);
    performance.measure(mark1);
    performance.measure(mark3);
    let (count, _) = performance.average("a").expect("should have had value");
    assert_eq!(count, 2);
    let (count, _) = performance.average("b").expect("should have had value");
    assert_eq!(count, 1);
    assert!(performance.average("c").is_none());
  }

  #[test]
  fn test_averages() {
    let performance = Performance::default();
    let mark1 = performance.mark("a");
    let mark2 = performance.mark("a");
    performance.measure(mark2);
    performance.measure(mark1);
    let averages = performance.averages();
    assert_eq!(averages.len(), 1);
    assert_eq!(averages[0].count, 2);
  }
}
