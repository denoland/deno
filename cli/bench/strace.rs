// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::Result;
use std::collections::HashMap;

pub(crate) struct StraceResult {
  pub percent_time: f64,
  pub seconds: f64,
  pub usecs_per_call: u64,
  pub calls: u64,
  pub errors: u64,
}

pub(crate) fn parse(output: &str) -> Result<HashMap<String, StraceResult>> {
  let mut summary = HashMap::new();

  // Filter out non-relevant lines. See the error log at
  // https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
  // This is checked in testdata/strace_summary2.out
  let mut lines = output
    .lines()
    .filter(|line| !line.is_empty() && !line.contains("detached ..."));
  let count = lines.clone().count();

  if count < 4 {
    return Ok(summary);
  }

  let total_line = lines.next_back().unwrap();
  lines.next_back(); // Drop separator
  let data_lines = lines.skip(2);

  for line in data_lines {
    let syscall_fields = line.split_whitespace().collect::<Vec<_>>();
    let len = syscall_fields.len();
    let syscall_name = syscall_fields.last().unwrap();

    if 5 <= len && len <= 6 {
      summary.insert(syscall_name.to_string(), extract_values(&syscall_fields));
    }
  }

  summary.insert(
    "total".to_string(),
    extract_values(&total_line.split_whitespace().collect::<Vec<_>>()),
  );

  Ok(summary)
}

fn extract_values(syscall_fields: &[&str]) -> StraceResult {
  StraceResult {
    percent_time: str::parse::<f64>(syscall_fields[0]).unwrap(),
    seconds: str::parse::<f64>(syscall_fields[1]).unwrap(),
    usecs_per_call: str::parse::<u64>(syscall_fields[2]).unwrap(),
    calls: str::parse::<u64>(syscall_fields[3]).unwrap(),
    errors: if syscall_fields.len() < 6 {
      0
    } else {
      str::parse::<u64>(syscall_fields[4]).unwrap()
    },
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn strace_parse_1() {
    const text: &str = include_str!("./testdata/strace_summary.out");
    let strace = parse(text)?;

    // first syscall line
    let munmap = strace.get("munmap").unwrap();
    assert_eq!(munmap.calls, 60);
    assert_eq!(munmap.errors, 0);

    // line with errors
    assert_eq!(strace.get("mkdir").unwrap().errors 2);

    // last syscall line
    let prlimit = strace.get("prlimit64").unwrap();
    assert_eq!(prlimit.calls, 2);
    assert_eq!(prlimit.percent_time, 0);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 704);
  }

  #[test]
  fn strace_parse_2() {
    const text: &str = include_str!("./testdata/strace_summary2.out");
    let strace = parse(text)?;

    // first syscall line
    let futex = strace.get("futex").unwrap();
    assert_eq!(munmap.calls, 449);
    assert_eq!(munmap.errors, 94);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 821);
  }
}
