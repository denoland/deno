// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::ExtModuleLoaderCb;
use crate::Extension;
use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::Snapshot;

pub type CompressionCb = dyn Fn(&mut Vec<u8>, &[u8]);

pub struct CreateSnapshotOptions {
  pub cargo_manifest_dir: &'static str,
  pub snapshot_path: PathBuf,
  pub startup_snapshot: Option<Snapshot>,
  pub extensions: Vec<Extension>,
  pub compression_cb: Option<Box<CompressionCb>>,
  pub snapshot_module_load_cb: Option<ExtModuleLoaderCb>,
}

pub fn create_snapshot(create_snapshot_options: CreateSnapshotOptions) {
  let mut mark = Instant::now();

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    startup_snapshot: create_snapshot_options.startup_snapshot,
    extensions: create_snapshot_options.extensions,
    snapshot_module_load_cb: create_snapshot_options.snapshot_module_load_cb,
    ..Default::default()
  });
  println!(
    "JsRuntime for snapshot prepared, took {:#?} ({})",
    Instant::now().saturating_duration_since(mark),
    create_snapshot_options.snapshot_path.display()
  );
  mark = Instant::now();

  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &snapshot;
  println!(
    "Snapshot size: {}, took {:#?} ({})",
    snapshot_slice.len(),
    Instant::now().saturating_duration_since(mark),
    create_snapshot_options.snapshot_path.display()
  );
  mark = Instant::now();

  let maybe_compressed_snapshot: Box<dyn AsRef<[u8]>> =
    if let Some(compression_cb) = create_snapshot_options.compression_cb {
      let mut vec = vec![];

      vec.extend_from_slice(
        &u32::try_from(snapshot.len())
          .expect("snapshot larger than 4gb")
          .to_le_bytes(),
      );

      (compression_cb)(&mut vec, snapshot_slice);

      println!(
        "Snapshot compressed size: {}, took {:#?} ({})",
        vec.len(),
        Instant::now().saturating_duration_since(mark),
        create_snapshot_options.snapshot_path.display()
      );
      mark = std::time::Instant::now();

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
    "Snapshot written, took: {:#?} ({})",
    Instant::now().saturating_duration_since(mark),
    create_snapshot_options.snapshot_path.display(),
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
