// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::libdeno::deno_buf;

#[cfg(not(feature = "use-snapshots"))]
pub fn deno_snapshot() -> Option<deno_buf> {
  None
}

#[cfg(feature = "use-snapshots")]
pub fn deno_snapshot() -> Option<deno_buf> {
  #[cfg(not(feature = "check-only"))]
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let data = vec![];

  unsafe { Some(deno_buf::from_raw_parts(data.as_ptr(), data.len())) }
}

#[cfg(not(feature = "use-snapshots"))]
pub fn compiler_snapshot() -> Option<deno_buf> {
  None
}

#[cfg(feature = "use-snapshots")]
pub fn compiler_snapshot() -> Option<deno_buf> {
  #[cfg(not(feature = "check-only"))]
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_compiler.bin"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let data = vec![];

  unsafe { Some(deno_buf::from_raw_parts(data.as_ptr(), data.len())) }
}
