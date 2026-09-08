// Copyright 2018-2026 the Deno authors. MIT license.
//! Measures per-iteration event loop cost.
//!
//! Two modes, both printing `key=value` lines on stdout:
//!
//! - `ticks [N]`   -- poll the event loop `N` (default 200000) times with
//!   nothing to do but a far-future timer. Each poll walks every event loop
//!   phase, so this is the cost of an otherwise idle tick.
//! - `new [N]`    -- create and drop `N` (default 50) empty runtimes.
//! - `ops [B] [S]` -- await `B` (default 2000) batches of `S` (default 100)
//!   `op_void_async_deferred` calls via `Promise.all`, i.e. the async op
//!   completion path.

use std::time::Instant;

use deno_core::JsRuntime;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;

/// This process's consumed CPU time (user + system) in nanoseconds. Wall
/// time is useless for the timer-driven mode, which spends most of its life
/// asleep waiting for the next 1 ms tick.
#[cfg(unix)]
fn cpu_time_ns() -> u64 {
  // SAFETY: `getrusage` fills in the (zeroed) struct we hand it.
  unsafe {
    let mut usage: libc::rusage = std::mem::zeroed();
    libc::getrusage(libc::RUSAGE_SELF, &mut usage);
    let to_ns = |t: libc::timeval| {
      t.tv_sec as u64 * 1_000_000_000 + t.tv_usec as u64 * 1_000
    };
    to_ns(usage.ru_utime) + to_ns(usage.ru_stime)
  }
}

/// No `getrusage` off unix; the `*_cpu_*` numbers degrade to wall clock,
/// which is fine for the modes that never sleep.
#[cfg(not(unix))]
fn cpu_time_ns() -> u64 {
  use std::sync::OnceLock;
  static START: OnceLock<Instant> = OnceLock::new();
  START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

fn arg(n: usize, default: u64) -> u64 {
  std::env::args()
    .nth(n)
    .map(|a| a.parse().expect("numeric argument expected"))
    .unwrap_or(default)
}

async fn run(source: String, label: &str, iterations: u64) {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  runtime.execute_script("<probe>", source).unwrap();
  let start = Instant::now();
  let cpu_start = cpu_time_ns();
  runtime
    .run_event_loop(PollEventLoopOptions::default())
    .await
    .unwrap();
  let cpu = cpu_time_ns() - cpu_start;
  let elapsed = start.elapsed();
  println!("{label}_iterations={iterations}");
  println!("{label}_total_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
  println!("{label}_cpu_ms={:.3}", cpu as f64 / 1e6);
  println!(
    "{label}_cpu_per_iteration_ns={:.1}",
    cpu as f64 / iterations as f64
  );
  println!(
    "{label}_per_iteration_ns={:.1}",
    elapsed.as_nanos() as f64 / iterations as f64
  );
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let mode = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "ticks".to_string());
  match mode.as_str() {
    "ticks" => {
      let n = arg(2, 200_000);
      // Poll the event loop `n` times by hand. A far-future refed timer
      // keeps it pending so every poll walks all six phases; polling
      // directly (rather than letting a repeating timer drive the loop)
      // keeps ~2 ms of sleeping per iteration out of the measurement.
      let mut runtime = JsRuntime::new(RuntimeOptions::default());
      runtime
        .execute_script(
          "<probe>",
          "Deno.core.createSystemTimer(() => {}, 600000, true);",
        )
        .unwrap();
      // `ticks N uv` forces the uv loop to exist, which measures the
      // "uv actually in use" tick instead of the inert one.
      if std::env::args().nth(3).as_deref() == Some("uv") {
        runtime.uv_loop_ptr().expect("uv loop");
      }
      let waker = std::task::Waker::noop();
      let mut cx = std::task::Context::from_waker(waker);
      let start = Instant::now();
      let cpu_start = cpu_time_ns();
      for _ in 0..n {
        let poll =
          runtime.poll_event_loop(&mut cx, PollEventLoopOptions::default());
        assert!(poll.is_pending(), "event loop should stay pending");
      }
      let cpu = cpu_time_ns() - cpu_start;
      let elapsed = start.elapsed();
      println!("ticks_iterations={n}");
      println!("ticks_total_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
      println!("ticks_cpu_ms={:.3}", cpu as f64 / 1e6);
      println!("ticks_cpu_per_iteration_ns={:.1}", cpu as f64 / n as f64);
      println!(
        "ticks_per_iteration_ns={:.1}",
        elapsed.as_nanos() as f64 / n as f64
      );
    }
    "new" => {
      // Cost of creating (and dropping) an empty runtime.
      let n = arg(2, 50);
      let start = Instant::now();
      let cpu_start = cpu_time_ns();
      for _ in 0..n {
        drop(JsRuntime::new(RuntimeOptions::default()));
      }
      let cpu = cpu_time_ns() - cpu_start;
      let elapsed = start.elapsed();
      println!("new_iterations={n}");
      println!("new_total_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
      println!("new_cpu_per_runtime_us={:.1}", cpu as f64 / n as f64 / 1e3);
      println!(
        "new_per_runtime_us={:.1}",
        elapsed.as_nanos() as f64 / n as f64 / 1e3
      );
    }
    "ops" => {
      let batches = arg(2, 2_000);
      let size = arg(3, 100);
      let src = format!(
        r#"
        const op = Deno.core.ops.op_void_async_deferred;
        (async () => {{
          for (let b = 0; b < {batches}; b++) {{
            const promises = [];
            for (let i = 0; i < {size}; i++) promises.push(op());
            await Promise.all(promises);
          }}
        }})();
        "#
      );
      run(src, "ops", batches * size).await;
    }
    other => panic!("unknown mode: {other}"),
  }
}
