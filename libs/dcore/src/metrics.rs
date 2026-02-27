// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::OpMetricsEvent;
use deno_core::OpMetricsFactoryFn;
use deno_core::OpMetricsSource;
use deno_core::anyhow::Error;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default, Debug, Clone, deno_core::serde::Serialize)]
#[serde(crate = "deno_core::serde")]
struct OpCount {
  slow: u64,
  fast: u64,
  #[serde(rename = "async")]
  async_: u64,
}

#[derive(Default, Debug, Clone, deno_core::serde::Serialize)]
#[serde(crate = "deno_core::serde")]
struct OpMetricsSummaryInner {
  counts: HashMap<String, OpCount>,
  completed_sync: u64,
  completed_async: u64,
  errored_sync: u64,
  errored_async: u64,
}

#[derive(Default, Debug, Clone)]
pub struct OpMetricsSummary(Rc<RefCell<OpMetricsSummaryInner>>);

impl OpMetricsSummary {
  pub fn completed(&self, source: OpMetricsSource) {
    let mut inner = self.0.borrow_mut();
    if matches!(source, OpMetricsSource::Async) {
      inner.completed_async += 1;
    } else {
      inner.completed_sync += 1;
    }
  }

  pub fn dispatched(&self, name: String, source: OpMetricsSource) {
    let mut inner = self.0.borrow_mut();
    let entry = inner.counts.entry(name).or_default();
    match source {
      OpMetricsSource::Slow => entry.slow += 1,
      OpMetricsSource::Fast => entry.fast += 1,
      OpMetricsSource::Async => entry.async_ += 1,
    }
  }

  pub fn errored(&self, source: OpMetricsSource) {
    let mut inner = self.0.borrow_mut();
    if matches!(source, OpMetricsSource::Async) {
      inner.errored_async += 1;
    } else {
      inner.errored_sync += 1;
    }
  }

  pub fn to_json_pretty(&self) -> Result<String, Error> {
    serde_json::to_string_pretty(&*self.0.borrow()).map_err(Into::into)
  }
}

pub fn create_metrics(
  strace: bool,
  summary: bool,
) -> (OpMetricsSummary, OpMetricsFactoryFn) {
  let metrics_summary = OpMetricsSummary::default();
  let now = std::time::Instant::now();
  let max_len: Rc<std::cell::Cell<usize>> = Default::default();
  (
    metrics_summary.clone(),
    Box::new({
      move |_, _, decl| {
        max_len.set(max_len.get().max(decl.name.len()));
        let max_len = max_len.clone();
        let metrics_summary = metrics_summary.clone();
        Some(Rc::new(move |op, event, source| {
          let metrics_summary = metrics_summary.clone();
          let name = op.decl().name.to_owned();
          if strace {
            eprintln!(
              "[{: >10.3}] {name:max_len$}: {event:?} {source:?}",
              now.elapsed().as_secs_f64(),
              max_len = max_len.get()
            );
          }
          if summary {
            match event {
              OpMetricsEvent::Completed | OpMetricsEvent::CompletedAsync => {
                metrics_summary.completed(source);
              }
              OpMetricsEvent::Dispatched => {
                metrics_summary.dispatched(name, source);
              }
              OpMetricsEvent::Error | OpMetricsEvent::ErrorAsync => {
                metrics_summary.errored(source);
              }
            }
          }
        }))
      }
    }),
  )
}
