// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::Snapshot;
use log::debug;
use once_cell::sync::Lazy;
use std::path::PathBuf;

pub static RUNTIME_SNAPSHOT: Lazy<Box<[u8]>> = Lazy::new(
  #[allow(clippy::uninit_vec)]
  #[cold]
  #[inline(never)]
  || {
    static COMPRESSED_RUNTIME_SNAPSHOT: &[u8] =
      include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"));

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

pub fn deno_isolate_init() -> Snapshot {
  debug!("Deno isolate init with snapshots.");
  Snapshot::Static(&RUNTIME_SNAPSHOT)
}

pub fn get_99_main() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("99_main.js")
}
