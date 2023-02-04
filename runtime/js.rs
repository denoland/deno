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

pub fn get_01_build() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("01_build.js")
}
pub fn get_01_errors() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("01_errors.js")
}
pub fn get_01_version() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("01_version.js")
}
pub fn get_06_util() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("06_util.js")
}
pub fn get_10_permissions() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("10_permissions.js")
}
pub fn get_11_workers() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("11_workers.js")
}
pub fn get_12_io() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("12_io.js")
}
pub fn get_13_buffer() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("13_buffer.js")
}
pub fn get_30_fs() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("30_fs.js")
}
pub fn get_30_os() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("30_os.js")
}
pub fn get_40_diagnostics() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_diagnostics.js")
}
pub fn get_40_files() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_files.js")
}
pub fn get_40_spawn() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_spawn.js")
}
pub fn get_40_fs_events() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_fs_events.js")
}
pub fn get_40_process() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_process.js")
}
pub fn get_40_read_file() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_read_file.js")
}
pub fn get_40_signals() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_signals.js")
}

pub fn get_40_tty() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_tty.js")
}

pub fn get_40_write_file() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_write_file.js")
}

pub fn get_41_prompt() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("41_prompt.js")
}

pub fn get_40_http() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("40_http.js")
}

pub fn get_90_deno_ns() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("90_deno_ns.js")
}

pub fn get_98_global_scope() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("98_global_scope.js")
}

pub fn get_99_main() -> PathBuf {
  let manifest = env!("CARGO_MANIFEST_DIR");
  let path = PathBuf::from(manifest);
  path.join("js").join("99_main.js")
}
