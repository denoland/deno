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

#[cfg(test)]
mod tests {
  #[test]
  fn cli_snapshot() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
      startup_snapshot: Some(crate::deno_isolate_init()),
      ..Default::default()
    });
    js_runtime
      .execute(
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
