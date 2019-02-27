// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::libdeno::deno_buf;

#[cfg(target_arch = "aarch64")]
use crate::libdeno::{deno_generate_snapshot};
#[cfg(target_arch = "aarch64")]
use crate::flags::v8_set_flags;
#[cfg(target_arch = "aarch64")]
use std::ffi::CString;

#[cfg(target_arch = "aarch64")]
pub fn deno_snapshot() -> deno_buf {
  let js_file = concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js");
  #[cfg(not(feature = "check-only"))]
  let js_source = include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let js_source = vec![];

  let cjs_file = CString::new(js_file).unwrap();

  v8_set_flags(vec![js_file.to_string(), std::str::from_utf8(js_source).unwrap().to_string()]);

  unsafe { deno_generate_snapshot(cjs_file.as_ptr(), js_source.as_ptr()) }
}

#[cfg(not(target_arch = "aarch64"))]
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

#[cfg(target_arch = "aarch64")]
pub fn compiler_snapshot() -> deno_buf {
  let js_file = concat!(env!("GN_OUT_DIR"), "/gen/bundle/compiler.js");
  #[cfg(not(feature = "check-only"))]
  let js_source = include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/compiler.js"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let js_source = vec![];

  let cjs_file = CString::new(js_file).unwrap();

    v8_set_flags(vec![js_file.to_string(), std::str::from_utf8(js_source).unwrap().to_string()]);

  unsafe { deno_generate_snapshot(cjs_file.as_ptr(), js_source.as_ptr()) }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn compiler_snapshot() -> deno_buf {
  #[cfg(not(feature = "check-only"))]
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_compiler.bin"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let data = vec![];

  unsafe { deno_buf::from_raw_parts(data.as_ptr(), data.len()) }
}
