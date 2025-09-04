// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(not(feature = "disable"))]
pub static CLI_SNAPSHOT: Option<&[u8]> = Some(include_bytes!(concat!(
  env!("OUT_DIR"),
  "/CLI_SNAPSHOT.bin"
)));
#[cfg(feature = "disable")]
pub static CLI_SNAPSHOT: Option<&[u8]> = None;

mod shared;

pub use shared::TS_VERSION;
