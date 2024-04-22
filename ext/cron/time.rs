// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// Identical to chrono::Utc::now() but without the system "clock"
/// feature flag.
///
/// The "clock" feature flag pulls in the "iana-time-zone" crate
/// which links to macOS's "CoreFoundation" framework which increases
/// startup time for the CLI.
pub fn utc_now() -> chrono::DateTime<chrono::Utc> {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system time before Unix epoch");
  chrono::DateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos())
    .unwrap()
}
