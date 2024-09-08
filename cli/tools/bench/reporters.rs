// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use serde::Serialize;

use crate::tools::test::TestFailureFormatOptions;
use crate::version;

use super::*;

pub trait BenchReporter {
  fn report_group_summary(&mut self);
  fn report_plan(&mut self, plan: &BenchPlan);
  fn report_end(&mut self, report: &BenchReport);
  fn report_register(&mut self, desc: &BenchDescription);
  fn report_wait(&mut self, desc: &BenchDescription);
  fn report_output(&mut self, output: &str);
  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult);
  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>);
}

const JSON_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Serialize)]
struct JsonReporterOutput {
  version: u8,
  runtime: String,
  cpu: String,
  benches: Vec<JsonReporterBench>,
}

impl Default for JsonReporterOutput {
  fn default() -> Self {
    Self {
      version: JSON_SCHEMA_VERSION,
      runtime: format!(
        "{} {}",
        version::DENO_VERSION_INFO.user_agent,
        env!("TARGET")
      ),
      cpu: mitata::cpu::name(),
      benches: vec![],
    }
  }
}

#[derive(Debug, Serialize)]
struct JsonReporterBench {
  origin: String,
  group: Option<String>,
  name: String,
  baseline: bool,
  results: Vec<BenchResult>,
}

#[derive(Debug, Serialize)]
pub struct JsonReporter(JsonReporterOutput);

impl JsonReporter {
  pub fn new() -> Self {
    Self(Default::default())
  }
}

#[allow(clippy::print_stdout)]
impl BenchReporter for JsonReporter {
  fn report_group_summary(&mut self) {}
  #[cold]
  fn report_plan(&mut self, _plan: &BenchPlan) {}

  fn report_end(&mut self, _report: &BenchReport) {
    match write_json_to_stdout(self) {
      Ok(_) => (),
      Err(e) => println!("{}", e),
    }
  }

  fn report_register(&mut self, _desc: &BenchDescription) {}

  fn report_wait(&mut self, _desc: &BenchDescription) {}

  fn report_output(&mut self, _output: &str) {}

  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult) {
    if desc.warmup {
      return;
    }

    let maybe_bench = self.0.benches.iter_mut().find(|bench| {
      bench.origin == desc.origin
        && bench.group == desc.group
        && bench.name == desc.name
        && bench.baseline == desc.baseline
    });

    if let Some(bench) = maybe_bench {
      bench.results.push(result.clone());
    } else {
      self.0.benches.push(JsonReporterBench {
        origin: desc.origin.clone(),
        group: desc.group.clone(),
        name: desc.name.clone(),
        baseline: desc.baseline,
        results: vec![result.clone()],
      });
    }
  }

  fn report_uncaught_error(&mut self, _origin: &str, _error: Box<JsError>) {}
}

pub struct ConsoleReporter {
  name: String,
  show_output: bool,
  group: Option<String>,
  baseline: bool,
  group_measurements: Vec<(BenchDescription, BenchStats)>,
  options: Option<mitata::reporter::Options>,
}

impl ConsoleReporter {
  pub fn new(show_output: bool) -> Self {
    Self {
      show_output,
      group: None,
      options: None,
      baseline: false,
      name: String::new(),
      group_measurements: Vec::new(),
    }
  }
}

#[allow(clippy::print_stdout)]
impl BenchReporter for ConsoleReporter {
  #[cold]
  fn report_plan(&mut self, plan: &BenchPlan) {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    static FIRST_PLAN: AtomicBool = AtomicBool::new(true);

    self.report_group_summary();

    self.group = None;
    self.baseline = false;
    self.name = String::new();
    self.group_measurements.clear();
    self.options = Some(mitata::reporter::Options::new(
      &plan.names.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
    ));

    let options = self.options.as_mut().unwrap();

    options.percentiles = true;

    if FIRST_PLAN
      .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      println!(
        "{}",
        colors::gray(format!("    CPU | {}", mitata::cpu::name()))
      );
      println!(
        "{}\n",
        colors::gray(format!(
          "Runtime | Deno {} ({})",
          crate::version::DENO_VERSION_INFO.deno,
          env!("TARGET")
        ))
      );
    } else {
      println!();
    }

    println!(
      "{}\n\n{}\n{}",
      colors::gray(&plan.origin),
      mitata::reporter::header(options),
      mitata::reporter::br(options)
    );
  }

  fn report_register(&mut self, _desc: &BenchDescription) {}

  fn report_wait(&mut self, desc: &BenchDescription) {
    self.name.clone_from(&desc.name);

    match &desc.group {
      None => {}

      Some(group) => {
        if self.group.is_none() || group != self.group.as_ref().unwrap() {
          self.report_group_summary();
          println!("{} {}", colors::gray("group"), colors::green(group));
        }

        self.group = Some(group.clone());
      }
    }
  }

  fn report_output(&mut self, output: &str) {
    if self.show_output {
      print!("{} {}", colors::gray(format!("{}:", self.name)), output)
    }
  }

  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult) {
    if desc.warmup {
      return;
    }

    let options = self.options.as_ref().unwrap();
    match result {
      BenchResult::Ok(stats) => {
        let mut desc = desc.clone();

        if desc.baseline && !self.baseline {
          self.baseline = true;
        } else {
          desc.baseline = false;
        }

        println!(
          "{}",
          mitata::reporter::benchmark(
            &desc.name,
            &mitata::reporter::BenchmarkStats {
              avg: stats.avg,
              min: stats.min,
              max: stats.max,
              p75: stats.p75,
              p99: stats.p99,
              p995: stats.p995,
            },
            options
          )
        );

        if !stats.high_precision && stats.used_explicit_timers {
          println!("{}", colors::yellow(format!("Warning: start() and end() calls in \"{}\" are ignored because it averages less\nthan 10µs per iteration. Remove them for better results.", &desc.name)));
        }

        self.group_measurements.push((desc, stats.clone()));
      }

      BenchResult::Failed(js_error) => {
        println!(
          "{}",
          mitata::reporter::benchmark_error(
            &desc.name,
            &mitata::reporter::Error {
              stack: None,
              message: format_test_error(
                js_error,
                &TestFailureFormatOptions::default()
              ),
            },
            options
          )
        )
      }
    };
  }

  fn report_group_summary(&mut self) {
    if self.options.is_none() {
      return;
    }

    if 2 <= self.group_measurements.len()
      && (self.group.is_some() || (self.group.is_none() && self.baseline))
    {
      println!(
        "\n{}",
        mitata::reporter::summary(
          &self
            .group_measurements
            .iter()
            .map(|(d, s)| mitata::reporter::GroupBenchmark {
              name: d.name.clone(),
              baseline: d.baseline,
              group: d.group.as_deref().unwrap_or("").to_owned(),

              stats: mitata::reporter::BenchmarkStats {
                avg: s.avg,
                min: s.min,
                max: s.max,
                p75: s.p75,
                p99: s.p99,
                p995: s.p995,
              },
            })
            .collect::<Vec<mitata::reporter::GroupBenchmark>>(),
        )
      );
    }
    println!();

    self.baseline = false;
    self.group_measurements.clear();
  }

  fn report_end(&mut self, _: &BenchReport) {
    self.report_group_summary();
  }

  fn report_uncaught_error(&mut self, _origin: &str, error: Box<JsError>) {
    println!(
      "{}: {}",
      colors::red_bold("error"),
      format_test_error(&error, &TestFailureFormatOptions::default())
    );
    println!("This error was not caught from a benchmark and caused the bench runner to fail on the referenced module.");
    println!("It most likely originated from a dangling promise, event/timeout handler or top-level code.");
    println!();
  }
}
