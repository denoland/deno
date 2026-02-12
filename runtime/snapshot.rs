// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::Extension;
use deno_core::ExtensionFileSourceCode;
use deno_core::snapshot::*;
use deno_core::v8;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::ops::bootstrap::SnapshotOptions;
use crate::shared::runtime;

/// Collect all filesystem source file paths from extensions.
/// These are the JS/TS files loaded via `LoadedFromFsDuringSnapshot`.
fn collect_input_file_paths(extensions: &[Extension]) -> Vec<&'static str> {
  let mut paths = Vec::new();
  for ext in extensions {
    for source in ext
      .get_js_sources()
      .iter()
      .chain(ext.get_esm_sources())
      .chain(ext.get_lazy_loaded_esm_sources())
    {
      if let ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path) =
        &source.code
      {
        paths.push(*path);
      }
    }
  }
  paths.sort();
  paths
}

/// Compute a hash of all snapshot inputs: file paths, file contents,
/// and version strings from SnapshotOptions.
fn compute_input_hash(
  files: &[&str],
  options: &SnapshotOptions,
) -> String {
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  files.len().hash(&mut hasher);
  for path in files {
    path.hash(&mut hasher);
    match std::fs::read(path) {
      Ok(content) => content.hash(&mut hasher),
      Err(_) => 0u8.hash(&mut hasher),
    }
  }
  options.ts_version.hash(&mut hasher);
  options.v8_version.hash(&mut hasher);
  options.target.hash(&mut hasher);
  format!("{:x}", hasher.finish())
}

pub fn create_runtime_snapshot(
  snapshot_path: PathBuf,
  snapshot_options: SnapshotOptions,
  // NOTE: For embedders that wish to add additional extensions to the snapshot
  custom_extensions: Vec<Extension>,
) {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let mut extensions: Vec<Extension> = vec![
    deno_telemetry::deno_telemetry::lazy_init(),
    deno_webidl::deno_webidl::lazy_init(),
    deno_web::deno_web::lazy_init(),
    deno_webgpu::deno_webgpu::lazy_init(),
    deno_image::deno_image::lazy_init(),
    deno_fetch::deno_fetch::lazy_init(),
    deno_cache::deno_cache::lazy_init(),
    deno_websocket::deno_websocket::lazy_init(),
    deno_webstorage::deno_webstorage::lazy_init(),
    deno_crypto::deno_crypto::lazy_init(),
    deno_ffi::deno_ffi::lazy_init(),
    deno_net::deno_net::lazy_init(),
    deno_tls::deno_tls::lazy_init(),
    deno_kv::deno_kv::lazy_init::<deno_kv::sqlite::SqliteDbHandler>(),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::lazy_init(),
    deno_http::deno_http::lazy_init(),
    deno_io::deno_io::lazy_init(),
    deno_fs::deno_fs::lazy_init(),
    deno_os::deno_os::lazy_init(),
    deno_process::deno_process::lazy_init(),
    deno_node_crypto::deno_node_crypto::lazy_init(),
    deno_node_sqlite::deno_node_sqlite::lazy_init(),
    deno_node::deno_node::lazy_init::<
      DenoInNpmPackageChecker,
      NpmResolver<sys_traits::impls::RealSys>,
      sys_traits::impls::RealSys,
    >(),
    ops::runtime::deno_runtime::lazy_init(),
    ops::worker_host::deno_worker_host::lazy_init(),
    ops::fs_events::deno_fs_events::lazy_init(),
    ops::permissions::deno_permissions::lazy_init(),
    ops::tty::deno_tty::lazy_init(),
    ops::http::deno_http_runtime::lazy_init(),
    deno_bundle_runtime::deno_bundle_runtime::lazy_init(),
    ops::bootstrap::deno_bootstrap::init(Some(snapshot_options.clone()), false),
    runtime::lazy_init(),
    ops::web_worker::deno_web_worker::lazy_init(),
  ];
  extensions.extend(custom_extensions);

  // Check if snapshot inputs have changed since last generation.
  // If not, skip the expensive create_snapshot() call entirely.
  let input_file_paths = collect_input_file_paths(&extensions);
  let current_hash = compute_input_hash(&input_file_paths, &snapshot_options);
  let hash_path = snapshot_path.with_extension("hash");

  if let Ok(stored_hash) = std::fs::read_to_string(&hash_path) {
    if stored_hash == current_hash && snapshot_path.exists() {
      #[allow(clippy::print_stdout)]
      for path in &input_file_paths {
        println!("cargo:rerun-if-changed={}", path);
      }
      return;
    }
  }

  let output = create_snapshot(
    CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      startup_snapshot: None,
      extensions,
      extension_transpiler: Some(Rc::new(|specifier, source| {
        crate::transpile::maybe_transpile_source(specifier, source)
      })),
      with_runtime_cb: Some(Box::new(|rt| {
        let isolate = rt.v8_isolate();
        v8::scope!(scope, isolate);

        let tmpl = deno_node::init_global_template(
          scope,
          deno_node::ContextInitMode::ForSnapshot,
        );
        let ctx = deno_node::create_v8_context(
          scope,
          tmpl,
          deno_node::ContextInitMode::ForSnapshot,
          std::ptr::null_mut(),
        );
        assert_eq!(scope.add_context(ctx), deno_node::VM_CONTEXT_INDEX);
      })),
      skip_op_registration: false,
    },
    None,
  )
  .unwrap();
  let existing = std::fs::read(&snapshot_path).ok();
  if existing.as_deref() != Some(&*output.output) {
    let mut snapshot = std::fs::File::create(&snapshot_path).unwrap();
    snapshot.write_all(&output.output).unwrap();
  }

  // Save hash for next build
  std::fs::write(&hash_path, current_hash).unwrap();

  #[allow(clippy::print_stdout)]
  for path in output.files_loaded_during_snapshot {
    println!("cargo:rerun-if-changed={}", path.display());
  }
}
