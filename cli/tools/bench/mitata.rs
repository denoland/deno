// Copyright 2022 evanwashere
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

/// Taken from https://stackoverflow.com/a/76572321
pub fn precision_f64(x: f64, decimals: u32) -> f64 {
  if x == 0. || decimals == 0 {
    0.
  } else {
    let shift = decimals as i32 - x.abs().log10().ceil() as i32;
    let shift_factor = 10_f64.powi(shift);

    (x * shift_factor).round() / shift_factor
  }
}

pub fn avg_to_iter_per_s(time: f64) -> String {
  let iter_per_s = precision_f64(1e9 / time, 4);
  let (decimals, fractional) = into_decimal_and_fractional_parts(iter_per_s);
  human_readable_decimal_with_fractional(decimals, fractional)
}

/// Return a tuple representing decimal part of provided float, as well as its
/// first fractional digit.
fn into_decimal_and_fractional_parts(num: f64) -> (i64, i64) {
  let mut decimal_part = num.floor() as i64;
  let fractional_part = {
    let decs = ((num - num.floor()) * 10.0).round();
    if decs == 10.0 {
      decimal_part += 1;
      0
    } else {
      decs as i64
    }
  };
  (decimal_part, fractional_part)
}

fn human_readable_decimal_with_fractional(
  decimal: i64,
  fractional: i64,
) -> String {
  // Sweet one-liner to separate integer by commas from:
  // https://stackoverflow.com/a/67834588/21759102
  let fmt_decimal = decimal
    .to_string()
    .as_bytes()
    .rchunks(3)
    .rev()
    .map(std::str::from_utf8)
    .collect::<Result<Vec<&str>, _>>()
    .unwrap()
    .join(",");

  if fmt_decimal.len() >= 4 {
    fmt_decimal
  } else {
    format!("{}.{}", fmt_decimal, fractional)
  }
}

pub fn fmt_duration(time: f64) -> String {
  if time < 1e0 {
    return format!("{:.1} ps", time * 1e3);
  }
  if time < 1e3 {
    return format!("{:.1} ns", time);
  }
  if time < 1e6 {
    return format!("{:.1} µs", time / 1e3);
  }
  if time < 1e9 {
    return format!("{:.1} ms", time / 1e6);
  }
  if time < 1e12 {
    return format!("{:.1} s", time / 1e9);
  }
  if time < 36e11 {
    return format!("{:.1} m", time / 60e9);
  }

  format!("{:.1} h", time / 36e11)
}

pub mod cpu {
  #![allow(dead_code, reason = "for certain platforms")]

  pub fn name() -> String {
    #[cfg(target_os = "linux")]
    return linux();
    #[cfg(target_os = "macos")]
    return macos();
    #[cfg(target_os = "windows")]
    return windows();

    #[allow(unreachable_code, reason = "for certain platforms")]
    {
      "unknown".to_string()
    }
  }

  pub fn macos() -> String {
    let mut sysctl = std::process::Command::new("sysctl");

    sysctl.arg("-n");
    sysctl.arg("machdep.cpu.brand_string");
    std::str::from_utf8(
      &sysctl
        .output()
        .map(|x| x.stdout)
        .unwrap_or(Vec::from("unknown")),
    )
    .unwrap()
    .trim()
    .to_string()
  }

  pub fn windows() -> String {
    let mut wmi = std::process::Command::new("wmic");

    wmi.arg("cpu");
    wmi.arg("get");
    wmi.arg("name");

    match wmi.output() {
      Err(_) => String::from("unknown"),

      Ok(x) => {
        let x = String::from_utf8_lossy(&x.stdout);
        x.lines().nth(1).unwrap_or("unknown").trim().to_string()
      }
    }
  }

  pub fn linux() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();

    for line in info.lines() {
      let mut iter = line.split(':');
      let key = iter.next().unwrap_or("");

      if key.contains("Hardware")
        || key.contains("Processor")
        || key.contains("chip type")
        || key.contains("model name")
        || key.starts_with("cpu type")
        || key.starts_with("cpu model")
      {
        return iter.next().unwrap_or("unknown").trim().to_string();
      }
    }

    String::from("unknown")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_into_decimal_and_fractional_parts() {
    assert_eq!(into_decimal_and_fractional_parts(10.0), (10, 0));
    assert_eq!(into_decimal_and_fractional_parts(10.1), (10, 1));
    assert_eq!(into_decimal_and_fractional_parts(10.2), (10, 2));
    assert_eq!(into_decimal_and_fractional_parts(10.3), (10, 3));
    assert_eq!(into_decimal_and_fractional_parts(10.4), (10, 4));
    assert_eq!(into_decimal_and_fractional_parts(10.5), (10, 5));
    assert_eq!(into_decimal_and_fractional_parts(10.6), (10, 6));
    assert_eq!(into_decimal_and_fractional_parts(10.7), (10, 7));
    assert_eq!(into_decimal_and_fractional_parts(10.8), (10, 8));
    assert_eq!(into_decimal_and_fractional_parts(10.9), (10, 9));
    assert_eq!(into_decimal_and_fractional_parts(10.99), (11, 0));
  }

  #[test]
  fn test_avg_to_iter_per_s() {
    assert_eq!(avg_to_iter_per_s(55.85), "17,910,000");
    assert_eq!(avg_to_iter_per_s(64_870_000.0), "15.4");
    assert_eq!(avg_to_iter_per_s(104_370_000.0), "9.6");
    assert_eq!(avg_to_iter_per_s(640_000.0), "1,563");
    assert_eq!(avg_to_iter_per_s(6_400_000.0), "156.3");
    assert_eq!(avg_to_iter_per_s(46_890_000.0), "21.3");
    assert_eq!(avg_to_iter_per_s(100_000_000.0), "10.0");
    assert_eq!(avg_to_iter_per_s(1_000_000_000.0), "1.0");
    assert_eq!(avg_to_iter_per_s(5_920_000_000.0), "0.2");
  }
}
