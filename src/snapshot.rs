// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno::deno_buf;

pub fn deno_snapshot() -> deno_buf {
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));

  unsafe { deno_buf::from_raw_parts(data.as_ptr(), data.len()) }
}
