// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno::deno_buf;
use std;

pub fn deno_snapshot() -> deno_buf {
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));
  let ptr = data.as_ptr();
  // TODO The transmute is not necessary here. deno_buf specifies mutable
  // pointers when it doesn't necessarally need mutable. So maybe the deno_buf
  // type should be broken into a mutable and non-mutable version?
  let ptr_mut = unsafe { std::mem::transmute::<*const u8, *mut u8>(ptr) };
  deno_buf {
    alloc_ptr: std::ptr::null_mut(),
    alloc_len: 0,
    data_ptr: ptr_mut,
    data_len: data.len(),
  }
}
