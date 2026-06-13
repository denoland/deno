// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::time::Duration;

use deno_core::error::AnyError;
use deno_core::serde_json;

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
  format!("{negative}{pretty_bytes}{unit}")
}

const BYTES_TO_KIB: u64 = 2u64.pow(10);
const BYTES_TO_MIB: u64 = 2u64.pow(20);

/// Gets the size used for downloading data. The total bytes is used to
/// determine the units to use.
pub fn human_download_size(byte_count: u64, total_bytes: u64) -> String {
  return if total_bytes < BYTES_TO_MIB {
    get_in_format(byte_count, BYTES_TO_KIB, "KiB")
  } else {
    get_in_format(byte_count, BYTES_TO_MIB, "MiB")
  };

  fn get_in_format(byte_count: u64, conversion: u64, suffix: &str) -> String {
    let converted_value = byte_count / conversion;
    let decimal = (byte_count % conversion) * 100 / conversion;
    format!("{converted_value}.{decimal:0>2}{suffix}")
  }
}

/// A function that converts an elapsed [`Duration`] to a string that
/// represents a human readable version of that time.
///
/// Durations under a millisecond are rendered with fractional millisecond
/// precision (e.g. `0.5ms`, `0.052ms`) so that very fast operations aren't
/// reported as `0ms`.
pub fn human_elapsed(elapsed: Duration) -> String {
  let millis = elapsed.as_millis();
  if millis == 0 {
    let micros = elapsed.as_micros();
    if micros == 0 {
      return "0ms".to_string();
    }
    // `micros` is in `1..1000` here. Render it as fractional milliseconds,
    // trimming trailing zeros so e.g. `523µs` -> `0.523ms`, `50µs` -> `0.05ms`
    // and `500µs` -> `0.5ms`.
    let fraction = format!("{micros:03}");
    return format!("0.{}ms", fraction.trim_end_matches('0'));
  }
  human_elapsed_with_ms_limit(millis, 1_000)
}

pub fn human_elapsed_with_ms_limit(elapsed: u128, ms_limit: u128) -> String {
  if elapsed < ms_limit {
    return format!("{elapsed}ms");
  }
  if elapsed < 1_000 * 60 {
    return format!("{}s", elapsed / 1000);
  }

  let seconds = elapsed / 1_000;
  let minutes = seconds / 60;
  let seconds_remainder = seconds % 60;
  format!("{minutes}m{seconds_remainder}s")
}

pub fn write_to_stdout_ignore_sigpipe(
  bytes: &[u8],
) -> Result<(), std::io::Error> {
  use std::io::ErrorKind;

  match std::io::stdout().write_all(bytes) {
    Ok(()) => Ok(()),
    Err(e) => match e.kind() {
      ErrorKind::BrokenPipe => Ok(()),
      _ => Err(e),
    },
  }
}

pub fn write_json_to_stdout<T>(value: &T) -> Result<(), AnyError>
where
  T: ?Sized + serde::ser::Serialize,
{
  let mut writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(&mut writer, value)?;
  writeln!(&mut writer)?;
  Ok(())
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
    assert_eq!(human_size(0_f64), "0B");
    assert_eq!(human_size(-10_f64), "-10B");
  }

  #[test]
  fn test_human_download_size() {
    assert_eq!(
      human_download_size(BYTES_TO_KIB / 100 - 1, BYTES_TO_KIB),
      "0.00KiB"
    );
    assert_eq!(
      human_download_size(BYTES_TO_KIB / 100 + 1, BYTES_TO_KIB),
      "0.01KiB"
    );
    assert_eq!(
      human_download_size(BYTES_TO_KIB / 5, BYTES_TO_KIB),
      "0.19KiB"
    );
    assert_eq!(
      human_download_size(BYTES_TO_MIB - 1, BYTES_TO_MIB - 1),
      "1023.99KiB"
    );
    assert_eq!(human_download_size(BYTES_TO_MIB, BYTES_TO_MIB), "1.00MiB");
    assert_eq!(
      human_download_size(BYTES_TO_MIB * 9 - 1523, BYTES_TO_MIB),
      "8.99MiB"
    );
  }

  #[test]
  fn test_human_elapsed() {
    assert_eq!(human_elapsed(Duration::from_millis(1)), "1ms");
    assert_eq!(human_elapsed(Duration::from_millis(256)), "256ms");
    assert_eq!(human_elapsed(Duration::from_millis(1000)), "1s");
    assert_eq!(human_elapsed(Duration::from_millis(1001)), "1s");
    assert_eq!(human_elapsed(Duration::from_millis(1020)), "1s");
    assert_eq!(human_elapsed(Duration::from_millis(70 * 1000)), "1m10s");
    assert_eq!(
      human_elapsed(Duration::from_millis(86 * 1000 + 100)),
      "1m26s"
    );
  }

  #[test]
  fn test_human_elapsed_sub_millisecond() {
    assert_eq!(human_elapsed(Duration::ZERO), "0ms");
    assert_eq!(human_elapsed(Duration::from_nanos(500)), "0ms");
    assert_eq!(human_elapsed(Duration::from_micros(1)), "0.001ms");
    assert_eq!(human_elapsed(Duration::from_micros(5)), "0.005ms");
    assert_eq!(human_elapsed(Duration::from_micros(50)), "0.05ms");
    assert_eq!(human_elapsed(Duration::from_micros(100)), "0.1ms");
    assert_eq!(human_elapsed(Duration::from_micros(523)), "0.523ms");
    assert_eq!(human_elapsed(Duration::from_micros(500)), "0.5ms");
    assert_eq!(human_elapsed(Duration::from_micros(999)), "0.999ms");
  }
}
