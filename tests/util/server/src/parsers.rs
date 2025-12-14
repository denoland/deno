// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;

use lazy_regex::Lazy;
use regex::Regex;
use serde::Serialize;

pub struct WrkOutput {
  pub latency: f64,
  pub requests: u64,
}

pub fn parse_wrk_output(output: &str) -> WrkOutput {
  static REQUESTS_RX: Lazy<Regex> =
    lazy_regex::lazy_regex!(r"Requests/sec:\s+(\d+)");
  static LATENCY_RX: Lazy<Regex> =
    lazy_regex::lazy_regex!(r"\s+99%(?:\s+(\d+.\d+)([a-z]+))");

  let mut requests = None;
  let mut latency = None;

  for line in output.lines() {
    if requests.is_none()
      && let Some(cap) = REQUESTS_RX.captures(line)
    {
      requests = Some(str::parse::<u64>(cap.get(1).unwrap().as_str()).unwrap());
    }
    if latency.is_none()
      && let Some(cap) = LATENCY_RX.captures(line)
    {
      let time = cap.get(1).unwrap();
      let unit = cap.get(2).unwrap();

      latency = Some(
        str::parse::<f64>(time.as_str()).unwrap()
          * match unit.as_str() {
            "ms" => 1.0,
            "us" => 0.001,
            "s" => 1000.0,
            _ => unreachable!(),
          },
      );
    }
  }

  WrkOutput {
    requests: requests.unwrap(),
    latency: latency.unwrap(),
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct StraceOutput {
  pub percent_time: f64,
  pub seconds: f64,
  pub usecs_per_call: Option<u64>,
  pub calls: u64,
  pub errors: u64,
}

pub fn parse_strace_output(output: &str) -> HashMap<String, StraceOutput> {
  let mut summary = HashMap::new();

  // Filter out non-relevant lines. See the error log at
  // https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
  // This is checked in testdata/strace_summary2.out
  let mut lines = output.lines().filter(|line| {
    !line.is_empty()
      && !line.contains("detached ...")
      && !line.contains("unfinished ...")
      && !line.contains("????")
  });
  let count = lines.clone().count();

  if count < 4 {
    return summary;
  }

  let total_line = lines.next_back().unwrap();
  lines.next_back(); // Drop separator
  let data_lines = lines.skip(2);

  for line in data_lines {
    let syscall_fields = line.split_whitespace().collect::<Vec<_>>();
    let len = syscall_fields.len();
    let syscall_name = syscall_fields.last().unwrap();
    if (5..=6).contains(&len) {
      summary.insert(
        syscall_name.to_string(),
        StraceOutput {
          percent_time: str::parse::<f64>(syscall_fields[0]).unwrap(),
          seconds: str::parse::<f64>(syscall_fields[1]).unwrap(),
          usecs_per_call: Some(str::parse::<u64>(syscall_fields[2]).unwrap()),
          calls: str::parse::<u64>(syscall_fields[3]).unwrap(),
          errors: if syscall_fields.len() < 6 {
            0
          } else {
            str::parse::<u64>(syscall_fields[4]).unwrap()
          },
        },
      );
    }
  }

  let total_fields = total_line.split_whitespace().collect::<Vec<_>>();

  let mut usecs_call_offset = 0;
  summary.insert(
    "total".to_string(),
    StraceOutput {
      percent_time: str::parse::<f64>(total_fields[0]).unwrap(),
      seconds: str::parse::<f64>(total_fields[1]).unwrap(),
      usecs_per_call: if total_fields.len() > 5 {
        usecs_call_offset = 1;
        Some(str::parse::<u64>(total_fields[2]).unwrap())
      } else {
        None
      },
      calls: str::parse::<u64>(total_fields[2 + usecs_call_offset]).unwrap(),
      errors: str::parse::<u64>(total_fields[3 + usecs_call_offset]).unwrap(),
    },
  );

  summary
}

pub fn parse_max_mem(output: &str) -> Option<u64> {
  // Takes the output from "time -v" as input and extracts the 'maximum
  // resident set size' and returns it in bytes.
  for line in output.lines() {
    if line
      .to_lowercase()
      .contains("maximum resident set size (kbytes)")
    {
      let value = line.split(": ").nth(1).unwrap();
      return Some(str::parse::<u64>(value).unwrap() * 1024);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn parse_wrk_output_1() {
    const TEXT: &str = include_str!("./testdata/wrk1.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 1837);
    assert!((wrk.latency - 6.25).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_2() {
    const TEXT: &str = include_str!("./testdata/wrk2.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 53435);
    assert!((wrk.latency - 6.22).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_3() {
    const TEXT: &str = include_str!("./testdata/wrk3.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 96037);
    assert!((wrk.latency - 6.36).abs() < f64::EPSILON);
  }

  #[test]
  fn max_mem_parse() {
    const TEXT: &str = include_str!("./testdata/time.out");
    let size = parse_max_mem(TEXT);

    assert_eq!(size, Some(120380 * 1024));
  }

  #[test]
  fn strace_parse_1() {
    const TEXT: &str = include_str!("./testdata/strace_summary.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let munmap = strace.get("munmap").unwrap();
    assert_eq!(munmap.calls, 60);
    assert_eq!(munmap.errors, 0);

    // line with errors
    assert_eq!(strace.get("mkdir").unwrap().errors, 2);

    // last syscall line
    let prlimit = strace.get("prlimit64").unwrap();
    assert_eq!(prlimit.calls, 2);
    assert!((prlimit.percent_time - 0.0).abs() < f64::EPSILON);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 704);
    assert_eq!(strace.get("total").unwrap().errors, 5);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, None);
  }

  #[test]
  fn strace_parse_2() {
    const TEXT: &str = include_str!("./testdata/strace_summary2.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("futex").unwrap();
    assert_eq!(futex.calls, 449);
    assert_eq!(futex.errors, 94);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 821);
    assert_eq!(strace.get("total").unwrap().errors, 107);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, None);
  }

  #[test]
  fn strace_parse_3() {
    const TEXT: &str = include_str!("./testdata/strace_summary3.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("mprotect").unwrap();
    assert_eq!(futex.calls, 90);
    assert_eq!(futex.errors, 0);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 543);
    assert_eq!(strace.get("total").unwrap().errors, 36);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, Some(6));
  }
}
