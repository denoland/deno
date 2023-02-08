// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use anyhow::Context;
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
        &("internal:".to_string() + &display_path_str.replace('\\', "/")),
        &std::fs::read_to_string(&file).unwrap(),
      )
      .unwrap();
  }
  for file in create_snapshot_options.additional_esm_files {
    let display_path = file.strip_prefix(display_root).unwrap_or(&file);
    let display_path_str = display_path.display().to_string();

    let filename =
      &("internal:".to_string() + &display_path_str.replace('\\', "/"));

    futures::executor::block_on(async {
      let id = js_runtime
        .load_side_module(
          &ModuleSpecifier::parse(filename)?,
          Some(std::fs::read_to_string(&file)?),
        )
        .await?;
      let receiver = js_runtime.mod_evaluate(id);
      js_runtime.run_event_loop(false).await?;
      receiver.await?
    })
    .with_context(|| format!("Couldn't execute '{}'", file.display()))
    .unwrap();
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

pub type FilterFn = Box<dyn Fn(&PathBuf) -> bool>;

pub fn get_js_files(
  cargo_manifest_dir: &'static str,
  directory: &str,
  filter: Option<FilterFn>,
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
        && filter.as_ref().map(|filter| filter(path)).unwrap_or(true)
    })
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
