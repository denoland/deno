// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::runtime::jsruntime::BUILTIN_SOURCES;
use crate::runtime::RuntimeSnapshotOptions;
use crate::ExtModuleLoaderCb;
use crate::Extension;
use crate::ExtensionFileSourceCode;
use crate::JsRuntimeForSnapshot;
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

pub struct CreateSnapshotOutput {
  /// Any files marked as LoadedFromFsDuringSnapshot are collected here and should be
  /// printed as 'cargo:rerun-if-changed' lines from your build script.
  pub files_loaded_during_snapshot: Vec<PathBuf>,
}

#[must_use = "The files listed by create_snapshot should be printed as 'cargo:rerun-if-changed' lines"]
pub fn create_snapshot(
  create_snapshot_options: CreateSnapshotOptions,
) -> CreateSnapshotOutput {
  let mut mark = Instant::now();

  let js_runtime = JsRuntimeForSnapshot::new(
    RuntimeOptions {
      startup_snapshot: create_snapshot_options.startup_snapshot,
      extensions: create_snapshot_options.extensions,
      ..Default::default()
    },
    RuntimeSnapshotOptions {
      snapshot_module_load_cb: create_snapshot_options.snapshot_module_load_cb,
    },
  );
  println!(
    "JsRuntime for snapshot prepared, took {:#?} ({})",
    Instant::now().saturating_duration_since(mark),
    create_snapshot_options.snapshot_path.display()
  );
  mark = Instant::now();

  let mut files_loaded_during_snapshot = vec![];
  for source in &*BUILTIN_SOURCES {
    if let ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path) =
      &source.code
    {
      files_loaded_during_snapshot.push(path.clone());
    }
  }
  for source in js_runtime
    .extensions()
    .iter()
    .flat_map(|e| vec![e.get_esm_sources(), e.get_js_sources()])
    .flatten()
  {
    if let ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path) =
      &source.code
    {
      files_loaded_during_snapshot.push(path.clone());
    }
  }

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
  CreateSnapshotOutput {
    files_loaded_during_snapshot,
  }
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

pub(crate) struct SnapshottedData {
  pub module_map_data: v8::Global<v8::Array>,
  pub module_handles: Vec<v8::Global<v8::Module>>,
}

static MODULE_MAP_CONTEXT_DATA_INDEX: usize = 0;

pub(crate) fn get_snapshotted_data(
  scope: &mut v8::HandleScope<()>,
  context: v8::Local<v8::Context>,
) -> SnapshottedData {
  let mut scope = v8::ContextScope::new(scope, context);

  // The 0th element is the module map itself, followed by X number of module
  // handles. We need to deserialize the "next_module_id" field from the
  // map to see how many module handles we expect.
  let result = scope.get_context_data_from_snapshot_once::<v8::Array>(
    MODULE_MAP_CONTEXT_DATA_INDEX,
  );

  let val = match result {
    Ok(v) => v,
    Err(err) => data_error_to_panic(err),
  };

  let next_module_id = {
    let info_data: v8::Local<v8::Array> =
      val.get_index(&mut scope, 1).unwrap().try_into().unwrap();
    info_data.length()
  };

  // Over allocate so executing a few scripts doesn't have to resize this vec.
  let mut module_handles = Vec::with_capacity(next_module_id as usize + 16);
  for i in 1..=next_module_id {
    match scope.get_context_data_from_snapshot_once::<v8::Module>(i as usize) {
      Ok(val) => {
        let module_global = v8::Global::new(&mut scope, val);
        module_handles.push(module_global);
      }
      Err(err) => data_error_to_panic(err),
    }
  }

  SnapshottedData {
    module_map_data: v8::Global::new(&mut scope, val),
    module_handles,
  }
}

pub(crate) fn set_snapshotted_data(
  scope: &mut v8::HandleScope<()>,
  context: v8::Global<v8::Context>,
  snapshotted_data: SnapshottedData,
) {
  let local_context = v8::Local::new(scope, context);
  let local_data = v8::Local::new(scope, snapshotted_data.module_map_data);
  let offset = scope.add_context_data(local_context, local_data);
  assert_eq!(offset, MODULE_MAP_CONTEXT_DATA_INDEX);

  for (index, handle) in snapshotted_data.module_handles.into_iter().enumerate()
  {
    let module_handle = v8::Local::new(scope, handle);
    let offset = scope.add_context_data(local_context, module_handle);
    assert_eq!(offset, index + 1);
  }
}

/// Returns an isolate set up for snapshotting.
pub(crate) fn create_snapshot_creator(
  external_refs: &'static v8::ExternalReferences,
  maybe_startup_snapshot: Option<Snapshot>,
) -> v8::OwnedIsolate {
  if let Some(snapshot) = maybe_startup_snapshot {
    match snapshot {
      Snapshot::Static(data) => {
        v8::Isolate::snapshot_creator_from_existing_snapshot(
          data,
          Some(external_refs),
        )
      }
      Snapshot::JustCreated(data) => {
        v8::Isolate::snapshot_creator_from_existing_snapshot(
          data,
          Some(external_refs),
        )
      }
      Snapshot::Boxed(data) => {
        v8::Isolate::snapshot_creator_from_existing_snapshot(
          data,
          Some(external_refs),
        )
      }
    }
  } else {
    v8::Isolate::snapshot_creator(Some(external_refs))
  }
}
