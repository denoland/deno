// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAverage {
  pub name: String,
  pub count: u32,
  pub average_duration: u32,
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

impl From<PerformanceMark> for PerformanceMeasure {
  fn from(value: PerformanceMark) -> Self {
    Self {
      name: value.name,
      count: value.count,
      duration: value.start.elapsed(),
    }
  }
}

/// A simple structure for marking a start of something to measure the duration
/// of and measuring that duration.  Each measurement is identified by a string
/// name and a counter is incremented each time a new measurement is marked.
///
/// The structure will limit the size of measurements to the most recent 1000,
/// and will roll off when that limit is reached.
#[derive(Debug)]
pub struct Performance {
  counts: Arc<Mutex<HashMap<String, u32>>>,
  max_size: usize,
  measures: Arc<Mutex<VecDeque<PerformanceMeasure>>>,
}

impl Default for Performance {
  fn default() -> Self {
    Self {
      counts: Default::default(),
      max_size: 1_000,
      measures: Default::default(),
    }
  }
}

impl Performance {
  /// Return the count and average duration of a measurement identified by name.
  #[cfg(test)]
  pub fn average(&self, name: &str) -> (usize, Duration) {
    let mut items = Vec::new();
    for measure in self.measures.lock().unwrap().iter() {
      if measure.name == name {
        items.push(measure.duration);
      }
    }
    let len = items.len();
    let average = if len > 0 {
      items.into_iter().sum::<Duration>() / len as u32
    } else {
      unreachable!("unexpected empty items key");
    };
    (len, average)
  }

  /// Return an iterator which provides the names, count, and average duration
  /// of each measurement.
  pub fn averages(&self) -> Vec<PerformanceAverage> {
    let mut averages: HashMap<String, Vec<Duration>> = HashMap::new();
    for measure in self.measures.lock().unwrap().iter() {
      averages
        .entry(measure.name.clone())
        .or_default()
        .push(measure.duration);
    }
    averages
      .into_iter()
      .map(|(k, d)| {
        let a = d.clone().into_iter().sum::<Duration>() / d.len() as u32;
        PerformanceAverage {
          name: k,
          count: d.len() as u32,
          average_duration: a.as_millis() as u32,
        }
      })
      .collect()
  }

  /// Marks the start of a measurement which returns a performance mark
  /// structure, which is then passed to `.measure()` to finalize the duration
  /// and add it to the internal buffer.
  pub fn mark<S: AsRef<str>>(&self, name: S) -> PerformanceMark {
    let name = name.as_ref();
    let mut counts = self.counts.lock().unwrap();
    let count = counts.entry(name.to_string()).or_insert(0);
    *count += 1;
    PerformanceMark {
      name: name.to_string(),
      count: *count,
      start: Instant::now(),
    }
  }

  /// A function which accepts a previously created performance mark which will
  /// be used to finalize the duration of the span being measured, and add the
  /// measurement to the internal buffer.
  pub fn measure(&self, mark: PerformanceMark) {
    let measure = PerformanceMeasure::from(mark);
    let mut measures = self.measures.lock().unwrap();
    measures.push_back(measure);
    while measures.len() > self.max_size {
      measures.pop_front();
    }
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
    let (count, _) = performance.average("a");
    assert_eq!(count, 2);
    let (count, _) = performance.average("b");
    assert_eq!(count, 1);
    let (count, _) = performance.average("c");
    assert_eq!(count, 0);
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
