// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use file_test_runner::TestResult;
use file_test_runner::parallelism::ParallelismProvider;

use crate::IS_CI;
use crate::colors;
use crate::eprintln;
use crate::print::spawn_thread;

pub fn flaky_test_ci(
  test_name: &str,
  run_test: impl Fn() -> TestResult,
) -> TestResult {
  if *IS_CI {
    run_flaky_test(test_name, run_test)
  } else {
    run_test()
  }
}

pub struct CpuMonitorParallelism {
  parallelism: Arc<file_test_runner::parallelism::Parallelism>,
  _tx: std::sync::mpsc::Sender<()>,
}

impl Default for CpuMonitorParallelism {
  fn default() -> Self {
    let (tx, rx) = std::sync::mpsc::channel();
    let parallelism =
      Arc::new(file_test_runner::parallelism::Parallelism::from_env());
    spawn_thread({
      let parallelism = parallelism.clone();
      move || {
        let mut system = sysinfo::System::default();
        let max_parallelism = parallelism.max_parallelism().get();
        let mut current_cpus = max_parallelism;
        if max_parallelism < 3 {
          return; // never decrease parallelism
        }
        // CPU thresholds for throttling test parallelism
        // Higher parallelism uses tighter bounds (95-97%) to be more responsive
        // Lower parallelism uses wider bounds to avoid thrashing
        let (upper_bound, lower_bound) = if max_parallelism >= 50 {
          // High parallelism: tight bounds for quick response
          (97, 95)
        } else {
          // Low parallelism: calculate adaptive bounds
          // Upper bound: leave headroom inversely proportional to parallelism
          // e.g., parallelism=10 -> upper=90%, parallelism=30 -> upper=~97%
          let upper = ((100.0 - 100.0 / max_parallelism as f64) as u8).max(50);

          // Lower bound: scale down from upper bound
          // More parallelism -> tighter bounds (smaller gap)
          // Less parallelism -> wider bounds (larger gap)
          let gap = (100.0 / max_parallelism as f64).min(20.0) as u8;
          let lower = upper.saturating_sub(gap).max(30);

          (upper, lower)
        };
        loop {
          match rx.recv_timeout(Duration::from_millis(250)) {
            Err(RecvTimeoutError::Timeout) => {
              // the documentation recommends calling this twice in order
              // to get a more accurate cpu reading
              system.refresh_cpu_usage();
              std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
              system.refresh_cpu_usage();
              let utilization =
                system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
                  / system.cpus().len() as f32;
              if utilization > 101f32 {
                // something wrong, ignore
                continue;
              }
              let utilization = utilization as u8;
              if utilization > upper_bound {
                if current_cpus > 2 {
                  current_cpus -= 1;
                  parallelism
                    .set_parallelism(NonZeroUsize::new(current_cpus).unwrap());
                }
              } else if utilization < lower_bound
                && current_cpus < max_parallelism
              {
                current_cpus += 1;
                parallelism
                  .set_parallelism(NonZeroUsize::new(current_cpus).unwrap());
              }
            }
            _ => {
              return;
            }
          }
        }
      }
    });
    Self {
      parallelism,
      _tx: tx,
    }
  }
}

impl CpuMonitorParallelism {
  pub fn for_run_options(
    &self,
  ) -> Arc<file_test_runner::parallelism::Parallelism> {
    self.parallelism.clone()
  }
}

pub fn run_flaky_test(
  test_name: &str,
  action: impl Fn() -> TestResult,
) -> TestResult {
  for i in 0..2 {
    let result = action();
    if !result.is_failed() {
      return result;
    }
    if *IS_CI {
      eprintln!(
        "{} {} was flaky on run {}",
        colors::bold_red("Warning"),
        colors::gray(test_name),
        i,
      );
    }
    std::thread::sleep(Duration::from_millis(100));
  }

  // surface on third try
  action()
}

pub struct TestTimeoutHolder {
  _tx: std::sync::mpsc::Sender<()>,
}

pub fn with_timeout(
  test_name: String,
  duration: Duration,
) -> TestTimeoutHolder {
  let (tx, rx) = ::std::sync::mpsc::channel::<()>();
  // ok to allow because we don't need to maintain logging context here
  #[allow(clippy::disallowed_methods)]
  std::thread::spawn(move || {
    if rx.recv_timeout(duration)
      == Err(::std::sync::mpsc::RecvTimeoutError::Timeout)
    {
      use std::io::Write;
      #[allow(clippy::print_stderr)]
      {
        ::std::eprintln!(
          "Test {test_name} timed out after {} seconds, aborting",
          duration.as_secs()
        );
      }
      _ = std::io::stderr().flush();
      #[allow(clippy::disallowed_methods)]
      ::std::process::exit(1);
    }
  });
  TestTimeoutHolder { _tx: tx }
}
