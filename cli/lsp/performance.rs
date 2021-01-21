// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

/// A structure which serves as a start of a measurement span.
#[derive(Debug)]
pub struct PerformanceMark {
  name: String,
  count: u32,
  start: Instant,
}

/// A structure which holds the information about the measured span.
#[derive(Debug)]
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
  counts: HashMap<String, u32>,
  measures: VecDeque<PerformanceMeasure>,
  size: usize,
}

impl Default for Performance {
  fn default() -> Self {
    Self {
      counts: Default::default(),
      measures: Default::default(),
      size: 1_000,
    }
  }
}

impl Performance {
  /// Return the count and average duration of a measurement identified by name.
  #[allow(unused)]
  pub fn average(&self, name: &str) -> (usize, Duration) {
    let mut items = Vec::new();
    for measure in &self.measures {
      if measure.name == name {
        items.push(measure.duration);
      }
    }
    let len = items.len();
    let average = if len > 0 {
      items.into_iter().sum::<Duration>() / len as u32
    } else {
      Duration::default()
    };
    (len, average)
  }

  /// Return an iterator which provides the names, count, and average duration
  /// of each measurement.
  pub fn averages(&self) -> impl Iterator<Item = (String, usize, Duration)> {
    let mut averages: HashMap<String, Vec<Duration>> = HashMap::new();
    for measure in &self.measures {
      averages
        .entry(measure.name.clone())
        .or_default()
        .push(measure.duration);
    }
    averages.into_iter().map(|(k, d)| {
      let a = d.clone().into_iter().sum::<Duration>() / d.len() as u32;
      (k, d.len(), a)
    })
  }

  /// Returns an iterator which provides each performance measure currently in
  /// memory.
  #[allow(unused)]
  pub fn get_measures(&self) -> impl Iterator<Item = &PerformanceMeasure> {
    self.measures.iter()
  }

  /// Returns an iterator which provides all the instances of a particular
  /// measure identified by name that are currently in memory.
  #[allow(unused)]
  pub fn get_measures_by_name<'a>(
    &'a self,
    name: &'a str,
  ) -> impl Iterator<Item = &'a PerformanceMeasure> {
    self.measures.iter().filter(move |m| m.name == name)
  }

  /// Marks the start of a measurement which returns a performance mark
  /// structure, which is then passed to `.measure()` to finalize the duration
  /// and add it to the internal buffer.
  pub fn mark<S: AsRef<str>>(&mut self, name: S) -> PerformanceMark {
    let name = name.as_ref();
    let count = self.counts.entry(name.to_string()).or_insert(0);
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
  pub fn measure(&mut self, mark: PerformanceMark) {
    let measure = PerformanceMeasure::from(mark);
    self.measures.push_back(measure);
    while self.measures.len() > self.size {
      self.measures.pop_front();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_measure() {
    let mut performance = Performance::default();
    let mark = performance.mark("a");
    performance.measure(mark);
    assert_eq!(performance.get_measures_by_name("a").count(), 1);
    let measure = performance.get_measures().take(1).next().unwrap();
    assert_eq!(measure.name, "a");
    assert_eq!(measure.count, 1);
  }

  #[test]
  fn test_average() {
    let mut performance = Performance::default();
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
    let mut performance = Performance::default();
    let mark1 = performance.mark("a");
    let mark2 = performance.mark("a");
    performance.measure(mark2);
    performance.measure(mark1);
    let averages: Vec<(String, usize, Duration)> =
      performance.averages().collect();
    assert_eq!(averages.len(), 1);
    assert_eq!(averages[0].1, 2);
  }
}
