// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::Snapshot;
use log::debug;
use once_cell::sync::Lazy;

pub static CLI_SNAPSHOT: Lazy<Box<[u8]>> = Lazy::new(
  #[allow(clippy::uninit_vec)]
  #[cold]
  #[inline(never)]
  || {
    static COMPRESSED_CLI_SNAPSHOT: &[u8] =
      include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

    let size =
      u32::from_le_bytes(COMPRESSED_CLI_SNAPSHOT[0..4].try_into().unwrap())
        as usize;
    let mut vec = Vec::with_capacity(size);

    // SAFETY: vec is allocated with exact snapshot size (+ alignment)
    // SAFETY: non zeroed bytes are overwritten with decompressed snapshot
    unsafe {
      vec.set_len(size);
    }

    lzzzz::lz4::decompress(&COMPRESSED_CLI_SNAPSHOT[4..], &mut vec).unwrap();

    vec.into_boxed_slice()
  },
);

pub fn deno_isolate_init() -> Snapshot {
  debug!("Deno isolate init with snapshots.");
  Snapshot::Static(&*CLI_SNAPSHOT)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cli_snapshot() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
      startup_snapshot: Some(deno_isolate_init()),
      ..Default::default()
    });
    js_runtime
      .execute_script(
        "<anon>",
        r#"
      if (!(bootstrap.mainRuntime && bootstrap.workerRuntime)) {
        throw Error("bad");
      }
      console.log("we have console.log!!!");
    "#,
      )
      .unwrap();
  }
}
