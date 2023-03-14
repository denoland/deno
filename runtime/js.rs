// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
#[cfg(not(feature = "dont_create_runtime_snapshot"))]
use deno_core::Snapshot;
#[cfg(not(feature = "dont_create_runtime_snapshot"))]
use log::debug;
#[cfg(not(feature = "dont_create_runtime_snapshot"))]
use once_cell::sync::Lazy;

#[cfg(not(feature = "dont_create_runtime_snapshot"))]
static COMPRESSED_RUNTIME_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"));

#[cfg(not(feature = "dont_create_runtime_snapshot"))]
pub static RUNTIME_SNAPSHOT: Lazy<Box<[u8]>> = Lazy::new(
  #[allow(clippy::uninit_vec)]
  #[cold]
  #[inline(never)]
  || {
    let size =
      u32::from_le_bytes(COMPRESSED_RUNTIME_SNAPSHOT[0..4].try_into().unwrap())
        as usize;
    let mut vec = Vec::with_capacity(size);

    // SAFETY: vec is allocated with exact snapshot size (+ alignment)
    // SAFETY: non zeroed bytes are overwritten with decompressed snapshot
    unsafe {
      vec.set_len(size);
    }

    lzzzz::lz4::decompress(&COMPRESSED_RUNTIME_SNAPSHOT[4..], &mut vec)
      .unwrap();

    vec.into_boxed_slice()
  },
);

#[cfg(not(feature = "dont_create_runtime_snapshot"))]
pub fn deno_isolate_init() -> Snapshot {
  debug!("Deno isolate init with snapshots.");
  Snapshot::Static(&RUNTIME_SNAPSHOT)
}

#[cfg(not(feature = "include_js_files_for_snapshotting"))]
pub static SOURCE_CODE_FOR_99_MAIN_JS: &str = include_str!("js/99_main.js");

#[cfg(feature = "include_js_files_for_snapshotting")]
pub static PATH_FOR_99_MAIN_JS: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/js/99_main.js");
