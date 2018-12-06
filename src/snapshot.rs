// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno::DenoBuf;

pub fn deno_snapshot() -> DenoBuf {
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));

  unsafe { DenoBuf::from_raw_parts(data.as_ptr(), data.len()) }
}
