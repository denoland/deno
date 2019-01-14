// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::libdeno::deno_buf;

pub fn deno_snapshot() -> deno_buf {
  #[cfg(not(feature = "check-only"))]
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let data = vec![];

  unsafe { deno_buf::from_raw_parts(data.as_ptr(), data.len()) }
}
