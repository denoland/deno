use deno_core::Snapshot;

pub static COMPILER_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.bin"));

pub fn compiler_snapshot() -> Snapshot {
  Snapshot::Static(COMPILER_SNAPSHOT)
}

pub static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Snapshot {
  //   debug!("Deno isolate init with snapshots.");
  let data = CLI_SNAPSHOT;
  Snapshot::Static(data)
}
