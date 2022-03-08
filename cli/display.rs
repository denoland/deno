// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// A function that converts a float to a string the represents a human
/// readable version of that number.
pub fn human_size(size: f64) -> String {
  let negative = if size.is_sign_positive() { "" } else { "-" };
  let size = size.abs();
  let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  if size < 1_f64 {
    return format!("{}{}{}", negative, size, "B");
  }
  let delimiter = 1024_f64;
  let exponent = std::cmp::min(
    (size.ln() / delimiter.ln()).floor() as i32,
    (units.len() - 1) as i32,
  );
  let pretty_bytes = format!("{:.2}", size / delimiter.powi(exponent))
    .parse::<f64>()
    .unwrap()
    * 1_f64;
  let unit = units[exponent as usize];
  format!("{}{}{}", negative, pretty_bytes, unit)
}

/// A function that converts a milisecond elapsed time to a string that
/// represents a human readable version of that time.
pub fn human_elapsed(elapsed: u128) -> String {
  if elapsed < 1_000 {
    return format!("{}ms", elapsed);
  }
  if elapsed < 1_000 * 60 {
    return format!("{}s", elapsed / 1000);
  }

  let seconds = elapsed / 1_000;
  let minutes = seconds / 60;
  let seconds_remainder = seconds % 60;
  format!("{}m{}s", minutes, seconds_remainder)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_human_size() {
    assert_eq!(human_size(1_f64), "1B");
    assert_eq!(human_size((12 * 1024) as f64), "12KB");
    assert_eq!(human_size((24_i64 * 1024 * 1024) as f64), "24MB");
    assert_eq!(human_size((24_i64 * 1024 * 1024 * 1024) as f64), "24GB");
    assert_eq!(
      human_size((24_i64 * 1024 * 1024 * 1024 * 1024) as f64),
      "24TB"
    );
  }

  #[test]
  fn test_human_elapsed() {
    assert_eq!(human_elapsed(1), "1ms");
    assert_eq!(human_elapsed(256), "256ms");
    assert_eq!(human_elapsed(1000), "1s");
    assert_eq!(human_elapsed(1001), "1s");
    assert_eq!(human_elapsed(1020), "1s");
    assert_eq!(human_elapsed(70 * 1000), "1m10s");
    assert_eq!(human_elapsed(86 * 1000 + 100), "1m26s");
  }
}
