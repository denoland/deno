// Copyright 2018-2026 the Deno authors. MIT license.

//! Event loop and uv_compat instrumentation.
//!
//! Enabled by setting `DENO_UV_INSTR=1`. Writes a periodic summary to
//! stderr every `DENO_UV_INSTR_INTERVAL` ms (default 1000). The summary
//! includes event-loop-tick timings, run_io timings, poll_tcp_handle
//! read/write byte counts, and a histogram of per-connection
//! accept→close durations — enough to locate where tail latency comes
//! from.
//!
//! The instrumentation is designed to have ~zero cost when disabled:
//! the `enabled()` check is inlined and branches on a `LazyLock<bool>`.

use std::cell::RefCell;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
  std::env::var("DENO_UV_INSTR")
    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    .unwrap_or(false)
});

static DUMP_INTERVAL_MS: LazyLock<u64> = LazyLock::new(|| {
  std::env::var("DENO_UV_INSTR_INTERVAL")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(1000)
});

#[inline(always)]
pub fn enabled() -> bool {
  *ENABLED
}

/// Logarithmic histogram over nanoseconds. Buckets are powers of 2
/// from 64ns (bucket 0) up to ~17s (bucket 28). Each bucket holds
/// a `u64` count. Total size is 29 * 8 = 232 bytes.
#[derive(Default, Clone)]
pub struct NsHistogram {
  pub buckets: [u64; 29],
  pub count: u64,
  pub sum_ns: u64,
  pub max_ns: u64,
}

impl NsHistogram {
  pub fn record(&mut self, ns: u64) {
    self.count += 1;
    self.sum_ns = self.sum_ns.saturating_add(ns);
    if ns > self.max_ns {
      self.max_ns = ns;
    }
    // Bucket 0 = <= 64ns (2^6). Each bucket doubles.
    let v = ns.max(1);
    let lz = v.leading_zeros();
    let idx = 63u32.saturating_sub(lz).saturating_sub(6);
    let idx = (idx as usize).min(self.buckets.len() - 1);
    self.buckets[idx] += 1;
  }

  /// Best-effort percentile from the coarse histogram. Returns the
  /// upper bound (ns) of the bucket containing the target count.
  pub fn percentile(&self, p: f64) -> u64 {
    if self.count == 0 {
      return 0;
    }
    let target = ((self.count as f64) * p).ceil() as u64;
    let mut running = 0u64;
    for (idx, &n) in self.buckets.iter().enumerate() {
      running += n;
      if running >= target {
        // Upper bound of bucket idx.
        let upper = 64u64 << idx;
        return upper;
      }
    }
    self.max_ns
  }
}

/// Simple u64 counter histogram, bucketed by power-of-2.
#[derive(Default, Clone)]
pub struct CountHistogram {
  pub buckets: [u64; 32],
  pub count: u64,
  pub sum: u64,
  pub max: u64,
}

impl CountHistogram {
  pub fn record(&mut self, v: u64) {
    self.count += 1;
    self.sum = self.sum.saturating_add(v);
    if v > self.max {
      self.max = v;
    }
    let idx = if v == 0 {
      0
    } else {
      63usize - (v.leading_zeros() as usize)
    };
    let idx = idx.min(self.buckets.len() - 1);
    self.buckets[idx] += 1;
  }
}

#[derive(Default)]
pub struct Stats {
  pub start: Option<Instant>,
  pub last_dump: Option<Instant>,

  // Whole event loop tick
  pub tick_count: u64,
  pub tick_time: NsHistogram,
  // Time between consecutive poll_event_loop_inner entries (tokio wake latency).
  pub gap_between_ticks: NsHistogram,
  pub last_tick_end: Option<Instant>,

  // Per phase (inside one tick)
  pub phase_timers_ns: NsHistogram,
  pub phase_pending_ns: NsHistogram,
  pub phase_idle_prepare_ns: NsHistogram,
  pub phase_io_ns: NsHistogram,
  pub phase_check_ns: NsHistogram,
  pub phase_close_ns: NsHistogram,

  // I/O spin loop
  pub io_spin_iterations: CountHistogram,

  // run_io
  pub run_io_calls: u64,
  pub run_io_time: NsHistogram,
  pub run_io_passes: CountHistogram,
  pub run_io_tcp_handles: CountHistogram,

  // poll_tcp_handle
  pub poll_tcp_calls: u64,
  pub poll_tcp_time: NsHistogram,
  pub poll_tcp_bytes_read: u64,
  pub poll_tcp_bytes_written: u64,
  pub poll_tcp_reads: u64,
  pub poll_tcp_writes: u64,
  pub poll_tcp_accepts: u64,

  // Per-connection lifecycle
  pub conn_accepts: u64,
  pub conn_closes: u64,
  pub conn_accept_to_first_read: NsHistogram,
  pub conn_accept_to_first_write: NsHistogram,
  pub conn_accept_to_close: NsHistogram,
  // Gap between when a TCP handle's data became ready (reactor polled it
  // and got WouldBlock on prior pass) and next successful read.
  pub conn_read_wait: NsHistogram,
}

thread_local! {
  static STATS: RefCell<Stats> = const { RefCell::new(Stats {
    start: None,
    last_dump: None,
    tick_count: 0,
    tick_time: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    gap_between_ticks: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    last_tick_end: None,
    phase_timers_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    phase_pending_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    phase_idle_prepare_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    phase_io_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    phase_check_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    phase_close_ns: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    io_spin_iterations: CountHistogram { buckets: [0; 32], count: 0, sum: 0, max: 0 },
    run_io_calls: 0,
    run_io_time: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    run_io_passes: CountHistogram { buckets: [0; 32], count: 0, sum: 0, max: 0 },
    run_io_tcp_handles: CountHistogram { buckets: [0; 32], count: 0, sum: 0, max: 0 },
    poll_tcp_calls: 0,
    poll_tcp_time: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    poll_tcp_bytes_read: 0,
    poll_tcp_bytes_written: 0,
    poll_tcp_reads: 0,
    poll_tcp_writes: 0,
    poll_tcp_accepts: 0,
    conn_accepts: 0,
    conn_closes: 0,
    conn_accept_to_first_read: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    conn_accept_to_first_write: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    conn_accept_to_close: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
    conn_read_wait: NsHistogram { buckets: [0; 29], count: 0, sum_ns: 0, max_ns: 0 },
  }) };
}

pub fn with_stats<F, R>(f: F) -> R
where
  F: FnOnce(&mut Stats) -> R,
{
  STATS.with(|s| f(&mut s.borrow_mut()))
}

/// Time a scope and record into a histogram slot selected by `field`.
pub struct ScopeTimer {
  start: Instant,
}

impl ScopeTimer {
  #[inline]
  pub fn start() -> Self {
    Self {
      start: Instant::now(),
    }
  }
  #[inline]
  pub fn elapsed_ns(&self) -> u64 {
    self.start.elapsed().as_nanos() as u64
  }
}

/// Maybe dump stats to stderr. Called from the event loop; no-op if
/// not enough time has elapsed since the last dump.
pub fn maybe_dump() {
  if !enabled() {
    return;
  }
  let interval = Duration::from_millis(*DUMP_INTERVAL_MS);
  let now = Instant::now();
  let should_dump = with_stats(|s| {
    if s.start.is_none() {
      s.start = Some(now);
    }
    let last = s.last_dump.unwrap_or(s.start.unwrap());
    if now.duration_since(last) >= interval {
      s.last_dump = Some(now);
      true
    } else {
      false
    }
  });
  if should_dump {
    dump_stats();
  }
}

/// Unconditional dump to stderr. Safe to call from exit handlers.
pub fn dump_stats() {
  with_stats(|s| {
    let mut out = String::new();
    fmt_stats(s, &mut out);
    eprintln!("{out}");
  });
}

fn fmt_ns(ns: u64) -> String {
  if ns < 1_000 {
    format!("{ns}ns")
  } else if ns < 1_000_000 {
    format!("{:.2}µs", ns as f64 / 1_000.0)
  } else if ns < 1_000_000_000 {
    format!("{:.2}ms", ns as f64 / 1_000_000.0)
  } else {
    format!("{:.2}s", ns as f64 / 1_000_000_000.0)
  }
}

fn fmt_hist(name: &str, h: &NsHistogram, out: &mut String) {
  if h.count == 0 {
    return;
  }
  let avg = h.sum_ns / h.count.max(1);
  out.push_str(&format!(
    "  {name:<28} n={:>8}  avg={:>8}  p50={:>8}  p90={:>8}  p99={:>8}  p999={:>8}  max={:>8}\n",
    h.count,
    fmt_ns(avg),
    fmt_ns(h.percentile(0.50)),
    fmt_ns(h.percentile(0.90)),
    fmt_ns(h.percentile(0.99)),
    fmt_ns(h.percentile(0.999)),
    fmt_ns(h.max_ns),
  ));
}

fn fmt_count(name: &str, h: &CountHistogram, out: &mut String) {
  if h.count == 0 {
    return;
  }
  let avg = h.sum as f64 / h.count as f64;
  out.push_str(&format!(
    "  {name:<28} n={:>8}  avg={:>8.2}  max={:>8}\n",
    h.count, avg, h.max,
  ));
}

fn fmt_stats(s: &Stats, out: &mut String) {
  let elapsed = s
    .start
    .map(|t| t.elapsed())
    .unwrap_or_else(|| Duration::from_secs(0));
  out.push_str(&format!(
    "\n===== uv_compat instrumentation (elapsed {:?}) =====\n",
    elapsed
  ));
  out.push_str(&format!(
    "  ticks={}  run_io_calls={}  poll_tcp_calls={}\n",
    s.tick_count, s.run_io_calls, s.poll_tcp_calls,
  ));
  out.push_str(&format!(
    "  conn: accepts={} closes={}  bytes_read={} bytes_written={}  reads={} writes={} accepts_ev={}\n",
    s.conn_accepts,
    s.conn_closes,
    s.poll_tcp_bytes_read,
    s.poll_tcp_bytes_written,
    s.poll_tcp_reads,
    s.poll_tcp_writes,
    s.poll_tcp_accepts,
  ));
  out.push_str("--- event loop phases ---\n");
  fmt_hist("tick_total", &s.tick_time, out);
  fmt_hist("gap_between_ticks", &s.gap_between_ticks, out);
  fmt_hist("phase_timers", &s.phase_timers_ns, out);
  fmt_hist("phase_pending", &s.phase_pending_ns, out);
  fmt_hist("phase_idle_prepare", &s.phase_idle_prepare_ns, out);
  fmt_hist("phase_io", &s.phase_io_ns, out);
  fmt_hist("phase_check", &s.phase_check_ns, out);
  fmt_hist("phase_close", &s.phase_close_ns, out);
  fmt_count("io_spin_iterations", &s.io_spin_iterations, out);
  out.push_str("--- run_io ---\n");
  fmt_hist("run_io_time", &s.run_io_time, out);
  fmt_count("run_io_passes", &s.run_io_passes, out);
  fmt_count("run_io_tcp_handles", &s.run_io_tcp_handles, out);
  out.push_str("--- poll_tcp_handle ---\n");
  fmt_hist("poll_tcp_time", &s.poll_tcp_time, out);
  out.push_str("--- per-connection ---\n");
  fmt_hist("accept_to_first_read", &s.conn_accept_to_first_read, out);
  fmt_hist("accept_to_first_write", &s.conn_accept_to_first_write, out);
  fmt_hist("accept_to_close", &s.conn_accept_to_close, out);
  fmt_hist("read_wait (data ready→read)", &s.conn_read_wait, out);
  out.push_str("=====================================================\n");
}
