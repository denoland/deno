// Copyright 2018-2025 the Deno authors. MIT license.

use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use crate::Extension;
use crate::JsRuntimeForSnapshot;
use crate::RuntimeOptions;
use crate::cppgc::FunctionTemplateSnapshotData;
use crate::error::CoreError;
use crate::modules::ModuleMapSnapshotData;

use super::ExtensionTranspiler;

pub type WithRuntimeCb = dyn Fn(&mut JsRuntimeForSnapshot);

pub type SnapshotDataId = u32;

/// We use this constant a few times
const ULEN: usize = std::mem::size_of::<usize>();

/// The v8 lifetime is different than the sidecar data, so we
/// allow for it to be split out.
pub(crate) struct V8Snapshot(pub(crate) &'static [u8]);

pub(crate) fn deconstruct(
  slice: &'static [u8],
) -> (V8Snapshot, SerializableSnapshotSidecarData<'static>) {
  let len =
    usize::from_le_bytes(slice[slice.len() - ULEN..].try_into().unwrap());
  let data = SerializableSnapshotSidecarData::from_slice(
    &slice[len..slice.len() - ULEN],
  );
  (V8Snapshot(&slice[0..len]), data)
}

pub(crate) fn serialize(
  v8_data: v8::StartupData,
  sidecar_data: SerializableSnapshotSidecarData,
) -> Box<[u8]> {
  let v8_data_len = v8_data.len();
  let sidecar_data = sidecar_data.into_bytes();
  let mut data = Vec::with_capacity(v8_data_len + sidecar_data.len() + ULEN);

  // add ulen
  data.extend_from_slice(&v8_data);
  data.extend_from_slice(&sidecar_data);
  data.extend_from_slice(&v8_data_len.to_le_bytes());

  data.into_boxed_slice()
}

#[derive(Default)]
pub struct SnapshotLoadDataStore {
  data: Vec<Option<v8::Global<v8::Data>>>,
}

impl SnapshotLoadDataStore {
  pub fn get<'s, 'i, T>(
    &mut self,
    scope: &mut v8::PinScope<'s, 'i>,
    id: SnapshotDataId,
  ) -> v8::Global<T>
  where
    v8::Local<'s, T>: TryFrom<v8::Local<'s, v8::Data>>,
  {
    let Some(data) = self.data.get_mut(id as usize) else {
      panic!(
        "Attempted to read snapshot data out of range: {id} (of {})",
        self.data.len()
      );
    };
    let Some(data) = data.take() else {
      panic!("Attempted to read the snapshot data at index {id} twice");
    };
    let local = v8::Local::new(scope, data);
    let local = v8::Local::<T>::try_from(local).unwrap_or_else(|_| {
      panic!(
        "Invalid data type at index {id}, expected '{}'",
        std::any::type_name::<T>()
      )
    });
    v8::Global::new(scope, local)
  }
}

#[derive(Default)]
pub struct SnapshotStoreDataStore {
  data: Vec<v8::Global<v8::Data>>,
}

impl SnapshotStoreDataStore {
  pub fn register<T>(&mut self, global: v8::Global<T>) -> SnapshotDataId
  where
    for<'s> v8::Local<'s, v8::Data>: From<v8::Local<'s, T>>,
  {
    let id = self.data.len();
    // TODO(mmastrac): v8::Global needs From/Into
    // SAFETY: Because we've tested that Local<Data>: From<Local<T>>, we can assume this is safe.
    unsafe {
      self.data.push(
        std::mem::transmute::<v8::Global<T>, v8::Global<v8::Data>>(global),
      );
    }
    id as _
  }
}

/// Options for [`create_snapshot`].
///
/// See: [example][1].
///
/// [1]: https://github.com/denoland/deno_core/tree/main/core/examples/snapshot
pub struct CreateSnapshotOptions {
  /// The directory which Cargo will compile everything into.
  ///
  /// This should always be the CARGO_MANIFEST_DIR environment variable.
  pub cargo_manifest_dir: &'static str,

  /// An optional starting snapshot atop which to build this snapshot.
  ///
  /// Passed to: [`RuntimeOptions::startup_snapshot`]
  pub startup_snapshot: Option<&'static [u8]>,

  /// Passed to [`RuntimeOptions::skip_op_registration`] while initializing the snapshot runtime.
  pub skip_op_registration: bool,

  /// Extensions to include within the generated snapshot.
  ///
  /// Passed to [`RuntimeOptions::extensions`]
  pub extensions: Vec<Extension>,

  /// An optional transpiler to modify the module source before inclusion in the snapshot.
  ///
  /// For example, this might transpile from TypeScript to JavaScript.
  ///
  /// Passed to: [`RuntimeOptions::extension_transpiler`]
  pub extension_transpiler: Option<Rc<ExtensionTranspiler>>,

  /// An optional callback to perform further modification of the runtime before
  /// taking the snapshot.
  pub with_runtime_cb: Option<Box<WithRuntimeCb>>,
}

/// See [`create_snapshot`] for usage overview.
pub struct CreateSnapshotOutput {
  /// Any files marked as LoadedFromFsDuringSnapshot are collected here and should be
  /// printed as 'cargo:rerun-if-changed' lines from your build script.
  pub files_loaded_during_snapshot: Vec<PathBuf>,

  /// The resulting snapshot file's bytes.
  pub output: Box<[u8]>,
}

/// Create a snapshot of a JavaScript runtime, which may yield better startup
/// time.
///
/// At a high level, the steps are:
///
///  * In your project's `build.rs` file:
///    * Call `create_snapshot()` from your `build.rs` file.
///    * Output the resulting snapshot to a path, preferably in [OUT_DIR].
///    * Make sure to print a `cargo:rerun-if-changed` line for each
///      [`CreateSnapshotOutput::files_loaded_during_snapshot`].
///  * In your project's source:
///    * Load the bytes of the generated snapshot file
///      ([`include_bytes`] is useful here)
///    * Pass those bytes to [`deno_core::JsRuntime::new`] via
///      [`RuntimeOptions::startup_snapshot`]
///
/// For a concrete example, see [core/examples/snapshot/][example].
///
/// [example]: https://github.com/denoland/deno_core/tree/main/core/examples/snapshot
/// [OUT_DIR]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
#[must_use = "The files listed by create_snapshot should be printed as 'cargo:rerun-if-changed' lines"]
pub fn create_snapshot(
  create_snapshot_options: CreateSnapshotOptions,
  warmup_script: Option<&'static str>,
) -> Result<CreateSnapshotOutput, CoreError> {
  let mut mark = Instant::now();
  #[allow(clippy::print_stdout)]
  {
    println!("Creating a snapshot...",);
  }

  // Get the extensions for a second pass if we want to warm up the snapshot.
  let warmup_exts = warmup_script.map(|_| {
    create_snapshot_options
      .extensions
      .iter()
      .map(|e| e.for_warmup())
      .collect::<Vec<_>>()
  });

  let mut js_runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
    startup_snapshot: create_snapshot_options.startup_snapshot,
    extensions: create_snapshot_options.extensions,
    extension_transpiler: create_snapshot_options.extension_transpiler,
    skip_op_registration: create_snapshot_options.skip_op_registration,
    ..Default::default()
  });

  #[allow(clippy::print_stdout)]
  {
    println!("JsRuntimeForSnapshot prepared, took {:#?}", mark.elapsed(),);
  }
  mark = Instant::now();

  let files_loaded_during_snapshot = js_runtime
    .files_loaded_from_fs_during_snapshot()
    .iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();

  if let Some(ref with_runtime_cb) = create_snapshot_options.with_runtime_cb {
    with_runtime_cb(&mut js_runtime);
  }

  let mut snapshot = js_runtime.snapshot();
  if let Some(warmup_script) = warmup_script {
    let leaked_snapshot = Box::leak(snapshot);
    let warmup_exts = warmup_exts.unwrap();

    // Warm up the snapshot bootstrap.
    //
    // - Create a new isolate with cold snapshot blob.
    // - Run warmup script in new context.
    // - Serialize the new context into a new snapshot blob.
    let mut js_runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      startup_snapshot: Some(leaked_snapshot),
      extensions: warmup_exts,
      skip_op_registration: true,
      ..Default::default()
    });

    if let Some(with_runtime_cb) = create_snapshot_options.with_runtime_cb {
      with_runtime_cb(&mut js_runtime);
    }

    js_runtime.execute_script("warmup", warmup_script)?;

    snapshot = js_runtime.snapshot();
  }

  #[allow(clippy::print_stdout)]
  {
    println!(
      "Snapshot size: {}, took {:#?}",
      snapshot.len(),
      mark.elapsed(),
    );
  }
  mark = Instant::now();

  #[allow(clippy::print_stdout)]
  {
    println!(
      "Snapshot written, took: {:#?}",
      Instant::now().saturating_duration_since(mark),
    );
  }

  Ok(CreateSnapshotOutput {
    files_loaded_during_snapshot,
    output: snapshot,
  })
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

/// The data we intend to snapshot, separated from any V8 objects that
/// are stored in the [`SnapshotLoadDataStore`]/[`SnapshotStoreDataStore`].
#[derive(Serialize, Deserialize)]
pub(crate) struct SnapshottedData<'snapshot> {
  pub js_handled_promise_rejection_cb: Option<u32>,
  pub ext_import_meta_proto: Option<u32>,
  pub module_map_data: ModuleMapSnapshotData,
  pub function_templates_data: FunctionTemplateSnapshotData,
  pub extensions: Vec<&'snapshot str>,
  pub op_count: usize,
  pub source_count: usize,
  pub addl_refs_count: usize,
  #[serde(borrow)]
  pub ext_source_maps: HashMap<&'snapshot str, &'snapshot [u8]>,
  #[serde(borrow)]
  pub external_strings: Vec<&'snapshot [u8]>,
}

/// Snapshot sidecar data, containing a [`SnapshottedData`] and the length of the
/// associated data array. This is the final form of the [`SnapshottedData`] before
/// we hand it off to serde.
#[derive(Serialize, Deserialize)]
pub(crate) struct SerializableSnapshotSidecarData<'snapshot> {
  data_count: u32,
  #[serde(borrow)]
  pub snapshot_data: SnapshottedData<'snapshot>,
}

impl<'snapshot> SerializableSnapshotSidecarData<'snapshot> {
  fn from_slice(slice: &'snapshot [u8]) -> Self {
    bincode::deserialize(slice).expect("Failed to deserialize snapshot data")
  }

  fn into_bytes(self) -> Box<[u8]> {
    bincode::serialize(&self).unwrap().into_boxed_slice()
  }
}

/// Given the sidecar data and a scope to extract data from, reconstructs the
/// `SnapshottedData` and `SnapshotLoadDataStore`.
pub(crate) fn load_snapshotted_data_from_snapshot<'snapshot>(
  scope: &mut v8::PinScope<()>,
  context: v8::Local<v8::Context>,
  raw_data: SerializableSnapshotSidecarData<'snapshot>,
) -> (SnapshottedData<'snapshot>, SnapshotLoadDataStore) {
  let scope = &mut v8::ContextScope::new(scope, context);
  let mut data = SnapshotLoadDataStore::default();
  for i in 0..raw_data.data_count {
    let item = scope
      .get_context_data_from_snapshot_once::<v8::Data>(i as usize)
      .unwrap();
    let item = v8::Global::new(scope, item);
    data.data.push(Some(item));
  }

  (raw_data.snapshot_data, data)
}

/// Given a `SnapshottedData` and `SnapshotStoreDataStore`, attaches the data to the
/// context and returns the serialized sidecar data.
pub(crate) fn store_snapshotted_data_for_snapshot<'snapshot>(
  scope: &mut v8::PinScope,
  context: v8::Global<v8::Context>,
  snapshotted_data: SnapshottedData<'snapshot>,
  data_store: SnapshotStoreDataStore,
) -> SerializableSnapshotSidecarData<'snapshot> {
  let context = v8::Local::new(scope, context);
  let raw_snapshot_data = SerializableSnapshotSidecarData {
    data_count: data_store.data.len() as _,
    snapshot_data: snapshotted_data,
  };

  for data in data_store.data {
    let data = v8::Local::new(scope, data);
    scope.add_context_data(context, data);
  }

  raw_snapshot_data
}

/// Returns an isolate set up for snapshotting.
pub(crate) fn create_snapshot_creator(
  external_refs: Cow<'static, [v8::ExternalReference]>,
  maybe_startup_snapshot: Option<V8Snapshot>,
  params: v8::CreateParams,
) -> v8::OwnedIsolate {
  if let Some(snapshot) = maybe_startup_snapshot {
    v8::Isolate::snapshot_creator_from_existing_snapshot(
      v8::StartupData::from(snapshot.0),
      Some(external_refs),
      Some(params),
    )
  } else {
    v8::Isolate::snapshot_creator(Some(external_refs), Some(params))
  }
}
