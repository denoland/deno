// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// Identical to chrono::Utc::now() but without the system "clock"
/// feature flag.
///
/// The "clock" feature flag pulls in the "iana-time-zone" crate
/// which links to macOS's "CoreFoundation" framework which increases
/// startup time for the CLI.
///
/// You can simply include this file in your project using
/// `include!("path/to/cli/util/time.rs"))` and use it
/// as a drop-in replacement for chrono::Utc::now().
pub fn utc_now() -> chrono::DateTime<chrono::Utc> {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system time before Unix epoch");
  let naive = chrono::NaiveDateTime::from_timestamp_opt(
    now.as_secs() as i64,
    now.subsec_nanos(),
  )
  .unwrap();
  chrono::DateTime::from_naive_utc_and_offset(naive, chrono::Utc)
}
