// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::Extension;
use crate::InternalModuleLoaderCb;
use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::Snapshot;

pub type CompressionCb = dyn Fn(&mut Vec<u8>, &[u8]);

pub struct CreateSnapshotOptions {
  pub cargo_manifest_dir: &'static str,
  pub snapshot_path: PathBuf,
  pub startup_snapshot: Option<Snapshot>,
  pub extensions: Vec<Extension>,
  pub extensions_with_js: Vec<Extension>,
  pub compression_cb: Option<Box<CompressionCb>>,
  pub snapshot_module_load_cb: Option<InternalModuleLoaderCb>,
}

pub fn create_snapshot(create_snapshot_options: CreateSnapshotOptions) {
  let mut mark = Instant::now();

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    startup_snapshot: create_snapshot_options.startup_snapshot,
    extensions: create_snapshot_options.extensions,
    extensions_with_js: create_snapshot_options.extensions_with_js,
    snapshot_module_load_cb: create_snapshot_options.snapshot_module_load_cb,
    ..Default::default()
  });

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

pub fn get_context_data(
  scope: &mut v8::HandleScope<()>,
  context: v8::Local<v8::Context>,
) -> (
  Vec<String>,
  Vec<v8::Global<v8::Module>>,
  v8::Global<v8::Object>,
) {
  fn data_error_to_panic(err: v8::DataError) -> ! {
    match err {
      v8::DataError::BadType { actual, expected } => {
        panic!(
          "Invalid type for snapshot data: expected {expected}, got {actual}"
        );
      }
      v8::DataError::NoData { expected } => {
        panic!("No data for snapshot data: expected {expected}");
      }
    }
  }

  let mut module_handles = vec![];
  let mut scope = v8::ContextScope::new(scope, context);

  // The 0th element is the list of extensions that were snapshotted.
  let extensions: Vec<String> =
    match scope.get_context_data_from_snapshot_once::<v8::Value>(0) {
      Ok(val) => serde_v8::from_v8(&mut scope, val).unwrap(),
      Err(err) => data_error_to_panic(err),
    };

  // The 1st element is the module map itself, followed by X number of module
  // handles. We need to deserialize the "next_module_id" field from the
  // map to see how many module handles we expect.
  match scope.get_context_data_from_snapshot_once::<v8::Object>(1) {
    Ok(val) => {
      let next_module_id = {
        let info_str = v8::String::new(&mut scope, "info").unwrap();
        let info_data: v8::Local<v8::Array> = val
          .get(&mut scope, info_str.into())
          .unwrap()
          .try_into()
          .unwrap();
        info_data.length()
      };

      for i in 2..=next_module_id {
        match scope
          .get_context_data_from_snapshot_once::<v8::Module>(i as usize)
        {
          Ok(val) => {
            let module_global = v8::Global::new(&mut scope, val);
            module_handles.push(module_global);
          }
          Err(err) => data_error_to_panic(err),
        }
      }

      (extensions, module_handles, v8::Global::new(&mut scope, val))
    }
    Err(err) => data_error_to_panic(err),
  }
}
