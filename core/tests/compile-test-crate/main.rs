use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
#[cfg(feature = "snapshot")]
use deno_core::Snapshot;

fn main() {
  #[cfg(feature = "snapshot")]
  let startup_snapshot = {
    let snapshot_data =
      include_bytes!(concat!(env!("OUT_DIR"), "/SNAPSHOT.bin"));
    Some(Snapshot::Static(snapshot_data))
  };

  #[cfg(not(feature = "snapshot"))]
  let startup_snapshot = None;

  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot,
    ..Default::default()
  });
  runtime
    .execute_script("", r#"Deno.core.print("Hi!");"#)
    .unwrap();
}
