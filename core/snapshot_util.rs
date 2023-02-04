// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use crate::Extension;
use crate::JsRuntime;
use crate::ModuleSpecifier;
use crate::RuntimeOptions;
use crate::Snapshot;

pub type CompressionCb = dyn Fn(&mut Vec<u8>, &[u8]);

pub struct CreateSnapshotOptions {
  pub cargo_manifest_dir: &'static str,
  pub snapshot_path: PathBuf,
  pub startup_snapshot: Option<Snapshot>,
  pub extensions: Vec<Extension>,
  pub extensions_with_js: Vec<Extension>,
  pub additional_files: Vec<PathBuf>,
  pub additional_esm_files: Vec<PathBuf>,
  pub compression_cb: Option<Box<CompressionCb>>,
}

pub fn create_snapshot(create_snapshot_options: CreateSnapshotOptions) {
  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    startup_snapshot: create_snapshot_options.startup_snapshot,
    extensions: create_snapshot_options.extensions,
    extensions_with_js: create_snapshot_options.extensions_with_js,
    ..Default::default()
  });

  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  let display_root = Path::new(create_snapshot_options.cargo_manifest_dir)
    .parent()
    .unwrap();
  for file in create_snapshot_options.additional_files {
    let display_path = file.strip_prefix(display_root).unwrap_or(&file);
    let display_path_str = display_path.display().to_string();
    js_runtime
      .execute_script(
        &("deno:".to_string() + &display_path_str.replace('\\', "/")),
        &std::fs::read_to_string(&file).unwrap(),
      )
      .unwrap();
  }
  for file in create_snapshot_options.additional_esm_files {
    let display_path = file.strip_prefix(display_root).unwrap_or(&file);
    let display_path_str = display_path.display().to_string();

    let filename =
      &("deno:".to_string() + &display_path_str.replace('\\', "/"));

    let id = futures::executor::block_on(js_runtime.load_side_module(
      &ModuleSpecifier::parse(filename).unwrap(),
      Some(std::fs::read_to_string(&file).unwrap()),
    ))
    .unwrap();
    let receiver = js_runtime.mod_evaluate(id);
    futures::executor::block_on(js_runtime.run_event_loop(false)).unwrap();
    let r = futures::executor::block_on(receiver).unwrap();
    eprintln!("result {:#?}", r);
  }

  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());

  let maybe_compressed_snapshot: Box<dyn AsRef<[u8]>> =
    if let Some(compression_cb) = create_snapshot_options.compression_cb {
      let mut vec = vec![];

      vec.extend_from_slice(
        &u32::try_from(snapshot.len())
          .expect("snapshot larger than 4gb")
          .to_le_bytes(),
      );

      (compression_cb)(&mut vec, snapshot_slice);

      println!("Snapshot compressed size: {}", vec.len());

      Box::new(vec)
    } else {
      Box::new(snapshot_slice)
    };

  std::fs::write(
    &create_snapshot_options.snapshot_path,
    &*maybe_compressed_snapshot,
  )
  .unwrap();
  println!(
    "Snapshot written to: {} ",
    create_snapshot_options.snapshot_path.display()
  );
}

pub fn get_js_files(
  cargo_manifest_dir: &'static str,
  directory: &str,
) -> Vec<PathBuf> {
  let manifest_dir = Path::new(cargo_manifest_dir);
  let mut js_files = std::fs::read_dir(directory)
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      manifest_dir.join(file.path())
    })
    .filter(|path| {
      path.extension().unwrap_or_default() == "js"
        && !path.ends_with("01_build.js")
        && !path.ends_with("01_errors.js")
        && !path.ends_with("01_version.js")
        && !path.ends_with("06_util.js")
        && !path.ends_with("10_permissions.js")
        && !path.ends_with("11_workers.js")
        && !path.ends_with("12_io.js")
        && !path.ends_with("13_buffer.js")
        && !path.ends_with("30_fs.js")
        && !path.ends_with("30_os.js")
        && !path.ends_with("40_diagnostics.js")
        && !path.ends_with("40_files.js")
        && !path.ends_with("40_fs_events.js")
        && !path.ends_with("40_process.js")
        && !path.ends_with("40_read_file.js")
        && !path.ends_with("40_spawn.js")
        && !path.ends_with("40_signals.js")
        && !path.ends_with("40_tty.js")
        && !path.ends_with("40_write_file.js")
        && !path.ends_with("40_http.js")
        && !path.ends_with("41_prompt.js")
        && !path.ends_with("90_deno_ns.js")
        && !path.ends_with("98_global_scope.js")
        && !path.ends_with("99_main.js")
    })
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
