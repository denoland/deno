#![allow(dead_code)]

pub mod fmt {
  use std::str::FromStr;

  #[derive(Clone, PartialEq)]
  pub enum Color {
    Red,
    Blue,
    Cyan,
    Gray,
    Black,
    White,
    Green,
    Yellow,
    Magenta,
  }

  pub fn bold(buf: &str) -> String {
    return format!("\x1b[1m{}\x1b[0m", buf);
  }

  pub fn duration(time: f64) -> String {
    unsafe {
      if time < 1e0 { return format!("{} ps", f64::from_str(&format!("{:.2}", time * 1e3)).unwrap_unchecked()); }
      
      if time < 1e3 { return format!("{} ns", f64::from_str(&format!("{:.2}", time)).unwrap_unchecked()); }
      if time < 1e6 { return format!("{} µs", f64::from_str(&format!("{:.2}", time / 1e3)).unwrap_unchecked()); }
      if time < 1e9 { return format!("{} ms", f64::from_str(&format!("{:.2}", time / 1e6)).unwrap_unchecked()); }
      if time < 1e12 { return format!("{} s", f64::from_str(&format!("{:.2}", time / 1e9)).unwrap_unchecked()); }
      if time < 36e11 { return format!("{} m", f64::from_str(&format!("{:.2}", time / 60e9)).unwrap_unchecked()); }

      return format!("{} h", f64::from_str(&format!("{:.2}", time / 36e11)).unwrap_unchecked());
    }
  }

  pub fn color(buf: &str, color: Color) -> String {
    return match color {
      Color::Red => format!("\x1b[31m{}\x1b[0m", buf),
      Color::Blue => format!("\x1b[34m{}\x1b[0m", buf),
      Color::Cyan => format!("\x1b[36m{}\x1b[0m", buf),
      Color::Gray => format!("\x1b[90m{}\x1b[0m", buf),
      Color::Black => format!("\x1b[30m{}\x1b[0m", buf),
      Color::White => format!("\x1b[37m{}\x1b[0m", buf),
      Color::Green => format!("\x1b[32m{}\x1b[0m", buf),
      Color::Yellow => format!("\x1b[33m{}\x1b[0m", buf),
      Color::Magenta => format!("\x1b[35m{}\x1b[0m", buf),
    };
  }
}

pub mod cpu {
  #![allow(dead_code)]

  pub fn name() -> String {
    #[cfg(target_os = "linux")] return linux();
    #[cfg(target_os = "macos")] return macos();
    #[cfg(target_os = "windows")] return windows();

    #[allow(unreachable_code)] { return "unknown".to_string(); }
  }

  pub fn macos() -> String {
    let mut sysctl = std::process::Command::new("sysctl");

    sysctl.arg("-n");
    sysctl.arg("machdep.cpu.brand_string");
    return std::str::from_utf8(&sysctl.output().map_or(Vec::from("unknown"), |x| x.stdout)).unwrap().trim().to_string();
  }

  pub fn windows() -> String {
    let mut wmi = std::process::Command::new("wmic");

    wmi.arg("cpu");
    wmi.arg("get");
    wmi.arg("name");

    return match wmi.output() {
      Err(_) => String::from("unknown"),

      Ok(x) => {
        let x = String::from_utf8_lossy(&x.stdout);
        return x.lines().nth(1).unwrap_or("unknown").trim().to_string();
      },
    };
  }

  pub fn linux() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or(String::new());

    for line in info.lines() {
      let mut iter = line.split(':');
      let key = iter.next().unwrap_or("");

      if key.contains("Hardware")
      || key.contains("Processor")
      || key.contains("chip type")
      || key.contains("model name")
      || key.starts_with("cpu type")
      || key.starts_with("cpu model") {
        return iter.next().unwrap_or("unknown").trim().to_string();
      }
    }

    return String::from("unknown");
  }
}

pub mod reporter {
  use super::*;

  #[derive(Clone, PartialEq)]
  pub struct Error {
    pub message: String,
    pub stack: Option<String>,
  }

  #[derive(Clone, PartialEq)]
  pub struct BenchmarkStats {
    pub avg: f64,
    pub min: f64, pub max: f64,
    pub p75: f64, pub p99: f64, pub p995: f64,
  }

  #[derive(Clone, PartialEq)]
  pub struct GroupBenchmark {
    pub name: String,
    pub group: String,
    pub baseline: bool,
    pub stats: BenchmarkStats,
  }

  #[derive(Clone, PartialEq)]
  pub struct Options {
    size: usize,
    pub avg: bool,
    pub colors: bool,
    pub min_max: bool,
    pub percentiles: bool,
  }

  impl Options {
    pub fn new(names: &[&str]) -> Options {
      return Options {
        avg: true,
        colors: true,
        min_max: true,
        size: size(names),
        percentiles: true,
      };
    }
  }

  pub fn size(names: &[&str]) -> usize {
    let mut max = 9;

    for name in names {
      if max < name.len() { max = name.len(); }
    }

    return 2 + max;
  }

  pub fn br(options: &Options) -> String {
    let mut s = String::new();

    s.push_str(&"-".repeat(options.size + 14 * options.avg as usize + 24 * options.min_max as usize));

    if options.percentiles {
      s.push_str(" ");
      s.push_str(&"-".repeat(9 + 10 + 10));
    }

    return s;
  }

  pub fn benchmark_error(n: &str, e: &Error, options: &Options) -> String {
    let size = options.size;
    let mut s = String::new();

    s.push_str(&format!("{:<size$}", n));
    s.push_str(&format!("{}: {}", &(if !options.colors { "error".to_string() } else { fmt::color("error", fmt::Color::Red) }), e.message));

    if let Some(ref stack) = e.stack {
      s.push_str("\n");

      match options.colors {
        false => s.push_str(stack),
        true => s.push_str(&fmt::color(stack, fmt::Color::Gray)),
      }
    }

    return s;
  }

  pub fn header(options: &Options) -> String {
    let size = options.size;
    let mut s = String::new();

    s.push_str(&format!("{:<size$}", "benchmark"));
    if options.avg { s.push_str(&format!("{:>14}", "time (avg)")); }
    if options.min_max { s.push_str(&format!("{:>24}", "(min … max)")); }
    if options.percentiles { s.push_str(&format!(" {:>9} {:>9} {:>9}", "p75", "p99", "p995")); }

    return s;
  }

  pub fn benchmark(name: &str, stats: &BenchmarkStats, options: &Options) -> String {
    let size = options.size;
    let mut s = String::new();

    s.push_str(&format!("{:<size$}", name));

    if !options.colors {
      if options.avg { s.push_str(&format!("{:>14}", format!("{}/iter", fmt::duration(stats.avg)))); }
      if options.min_max { s.push_str(&format!("{:>24}", format!("({} … {})", fmt::duration(stats.min), fmt::duration(stats.max)))); }
      if options.percentiles { s.push_str(&format!(" {:>9} {:>9} {:>9}", fmt::duration(stats.p75), fmt::duration(stats.p99), fmt::duration(stats.p995))); }
    }

    else {
      if options.avg { s.push_str(&format!("{:>23}", format!("{}/iter", fmt::color(&fmt::duration(stats.avg), fmt::Color::Yellow)))); }
      if options.min_max { s.push_str(&format!("{:>42}", format!("({} … {})", fmt::color(&format!("{}", fmt::duration(stats.min)), fmt::Color::Cyan), fmt::color(&format!("{}", fmt::duration(stats.max)), fmt::Color::Magenta)))); }
      if options.percentiles { s.push_str(&format!(" {:>18} {:>18} {:>18}", fmt::color(&format!("{}", fmt::duration(stats.p75)), fmt::Color::Gray), fmt::color(&format!("{}", fmt::duration(stats.p99)), fmt::Color::Gray), fmt::color(&format!("{}", fmt::duration(stats.p995)), fmt::Color::Gray))); }
    }

    return s;
  }

  pub fn summary(benchmarks: &[GroupBenchmark], options: &Options) -> String {
    use std::str::FromStr;

    let mut s = String::new();
    let mut benchmarks = benchmarks.to_owned();
    benchmarks.sort_by(|a, b| a.stats.avg.partial_cmp(&b.stats.avg).unwrap());
    let baseline = benchmarks.iter().find(|b| b.baseline).unwrap_or(&benchmarks[0]);

    if !options.colors {
      s.push_str(&format!("summary\n  {}", baseline.name));

      for b in benchmarks.iter().filter(|b| *b != baseline) {
        let faster = b.stats.avg >= baseline.stats.avg;
        let diff = f64::from_str(&format!("{:.2}", 1.0 / baseline.stats.avg * b.stats.avg)).unwrap();
        let inv_diff = f64::from_str(&format!("{:.2}", 1.0 / b.stats.avg * baseline.stats.avg)).unwrap();
        s.push_str(&format!("\n   {}x times {} than {}", if faster { diff } else { inv_diff }, if faster { "faster" } else { "slower" }, b.name));
      }
    }

    else {
      s.push_str(&format!("{}\n  {}", fmt::bold("summary"), fmt::bold(&fmt::color(&baseline.name, fmt::Color::Cyan))));

      for b in benchmarks.iter().filter(|b| *b != baseline) {
        let faster = b.stats.avg >= baseline.stats.avg;
        let diff = f64::from_str(&format!("{:.2}", 1.0 / baseline.stats.avg * b.stats.avg)).unwrap();
        let inv_diff = f64::from_str(&format!("{:.2}", 1.0 / b.stats.avg * baseline.stats.avg)).unwrap();
        s.push_str(&format!("\n   {}x {} than {}", if faster { fmt::color(&format!("{}", diff), fmt::Color::Green) } else { fmt::color(&format!("{}", inv_diff), fmt::Color::Red) }, if faster { "faster" } else { "slower" }, fmt::bold(&fmt::color(&b.name, fmt::Color::Cyan))));
      }
    }

    return s;
  }
}

pub mod bench {
  pub struct Stats {
    pub n: usize,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub p75: f64,
    pub p99: f64,
    pub p995: f64,
    pub p999: f64,
    pub jit: [f64; 10],
  }

  pub fn stats(n: usize, t: bool, avg: f64, min: f64, max: f64, jit: &[f64; 10], all: &[f64]) -> Stats {
    return Stats {
      n,
      min,
      max,
      jit: jit.to_owned(),
      p75: all[(n as f64 * (75.0 / 100.0)) as usize - 1],
      p99: all[(n as f64 * (99.0 / 100.0)) as usize - 1],
      p995: all[(n as f64 * (99.5 / 100.0)) as usize - 1],
      p999: all[(n as f64 * (99.9 / 100.0)) as usize - 1],
      avg: if !t { avg / n as f64 } else { (avg / n as f64).ceil() },
    };
  }

  pub fn sync<T, F>(t: std::time::Duration, f: F, collect: bool) -> Stats where F: Fn() -> T {
    let mut n = 0;
    let mut avg = 0.0;
    let mut wavg = 0.0;
    let mut jit = [0.0; 10];
    let mut all = Vec::new();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;

    {
      for offset in 0..10 {
        let t1 = std::time::Instant::now();

        let _ = f();
        jit[offset] = t1.elapsed().as_nanos() as f64;
      }

      let mut c = 0;
      let mut iterations: isize = 4;
      let mut budget = std::time::Duration::from_millis(10);

      loop {
        if 0 > iterations && budget == std::time::Duration::from_millis(0) { break; }

        let t1 = std::time::Instant::now();

        let _ = f();
        let t2 = t1.elapsed();
        let t2_ns = t2.as_nanos() as f64;
        if 0.0 > t2_ns { iterations += 1; continue; }

        c += 1;
        wavg += t2_ns;
        iterations -= 1;
        budget = budget.saturating_sub(t2);
      }

      wavg /= c as f64;
    }

    {
      if wavg > 10_000.0 {
        let mut budget = t.clone();
        let mut iterations: isize = 10;

        loop {
          if 0 > iterations && budget == std::time::Duration::from_millis(0) { break; }

          let t1 = std::time::Instant::now();

          let _ = f();
          let t2 = t1.elapsed();
          let t2_ns = t2.as_nanos() as f64;
          if 0.0 > t2_ns { iterations += 1; continue; }

          n += 1;
          avg += t2_ns;
          iterations -= 1;
          all.push(t2_ns);
          if min > t2_ns { min = t2_ns; }
          if max < t2_ns { max = t2_ns; }
          budget = budget.saturating_sub(t2);
        }
      }

      else {
        let mut budget = t.clone();
        let mut iterations: isize = 10;

        if !collect {
          loop {
            if 0 > iterations && budget == std::time::Duration::from_millis(0) { break; }

            let t1 = std::time::Instant::now();
            for _ in 0..(1e4 as usize) { let _ = f(); }

            let t2 = t1.elapsed();
            let t2_ns = t2.as_nanos() as f64 / 1e4;
            if 0.0 > t2_ns { iterations += 1; continue; }

            n += 1;
            avg += t2_ns;
            iterations -= 1;
            all.push(t2_ns);
            if min > t2_ns { min = t2_ns; }
            if max < t2_ns { max = t2_ns; }
            budget = budget.saturating_sub(t2);
          }
        }

        else {
          let mut garbage = Vec::with_capacity(1e4 as usize);

          loop {
            if 0 > iterations && budget == std::time::Duration::from_millis(0) { break; }

            let t1 = std::time::Instant::now();
            for o in 0..(1e4 as usize) { unsafe { *garbage.get_unchecked_mut(o) = f(); } }

            let t2 = t1.elapsed();
            let t2_ns = t2.as_nanos() as f64 / 1e4;
            if 0.0 > t2_ns { iterations += 1; continue; }

            n += 1;
            avg += t2_ns;
            iterations -= 1;
            all.push(t2_ns);
            if min > t2_ns { min = t2_ns; }
            if max < t2_ns { max = t2_ns; }
            budget = budget.saturating_sub(t2);
          }
        }
      }
    }

    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    return stats(n, wavg > 10_000.0, avg, min, max, &jit, &all);
  }
}