// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;
use deno_core::snapshot::*;
use deno_core::v8;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::ops::bootstrap::SnapshotOptions;
use crate::shared::runtime;

/// A single lazy_loaded_* entry declared by an extension, in
/// `(specifier, absolute source path)` form. The build script uses this to
/// decide which sources still need to be embedded in the final binary.
#[derive(Clone, Debug)]
pub struct LazyExtensionFile {
  pub specifier: String,
  pub path: PathBuf,
  pub kind: LazyExtensionFileKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LazyExtensionFileKind {
  /// `lazy_loaded_js` — loaded on demand via `core.loadExtScript()`.
  Js,
  /// `lazy_loaded_esm` — loaded on demand via `op_lazy_load_esm` (i.e. the
  /// `createLazyLoader` factory exposed by `Deno.core`).
  Esm,
}

pub struct CreateRuntimeSnapshotOutput {
  /// Paths the snapshot read from disk; emit as `cargo:rerun-if-changed`.
  pub files_loaded_during_snapshot: Vec<PathBuf>,
  /// Specifiers of `lazy_loaded_*` files compiled into the snapshot blob.
  /// Their source already lives in the snapshot; no `include_str!` is needed.
  pub consumed_lazy_specifiers: Vec<String>,
  /// Every `lazy_loaded_*` file declared by any extension fed to the snapshot.
  /// The residual set the binary needs at runtime is
  /// `lazy_extension_files \ consumed_lazy_specifiers`.
  pub lazy_extension_files: Vec<LazyExtensionFile>,
}

pub fn create_runtime_snapshot(
  snapshot_path: PathBuf,
  snapshot_options: SnapshotOptions,
  // NOTE: For embedders that wish to add additional extensions to the snapshot
  custom_extensions: Vec<Extension>,
) -> CreateRuntimeSnapshotOutput {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let mut extensions: Vec<Extension> = vec![
    deno_telemetry::deno_telemetry::lazy_init(),
    deno_webidl::deno_webidl::lazy_init(),
    deno_web::deno_web::lazy_init(),
    deno_webgpu::deno_webgpu::lazy_init(),
    deno_image::deno_image::lazy_init(),
    deno_canvas::deno_canvas::lazy_init(),
    deno_fetch::deno_fetch::lazy_init(),
    deno_cache::deno_cache::lazy_init(),
    deno_websocket::deno_websocket::lazy_init(),
    deno_webstorage::deno_webstorage::lazy_init(),
    deno_crypto::deno_crypto::lazy_init(),
    deno_ffi::deno_ffi::lazy_init(),
    deno_net::deno_net::lazy_init(),
    deno_tls::deno_tls::lazy_init(),
    deno_kv::deno_kv::lazy_init(),
    deno_cron::deno_cron::init(Box::new(
      deno_cron::local::LocalCronHandler::new(),
    )),
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
    ops::bootstrap::deno_bootstrap::init(Some(snapshot_options), false),
    runtime::lazy_init(),
    ops::web_worker::deno_web_worker::lazy_init(),
  ];
  extensions.extend(custom_extensions);

  let lazy_extension_files = collect_lazy_extension_files(&extensions);

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
  let mut snapshot = std::fs::File::create(snapshot_path).unwrap();
  snapshot.write_all(&output.output).unwrap();

  #[allow(clippy::print_stdout, reason = "necessary for build code")]
  for path in &output.files_loaded_during_snapshot {
    println!("cargo:rerun-if-changed={}", path.display());
  }

  CreateRuntimeSnapshotOutput {
    files_loaded_during_snapshot: output.files_loaded_during_snapshot,
    consumed_lazy_specifiers: output.consumed_lazy_specifiers,
    lazy_extension_files,
  }
}

fn collect_lazy_extension_files(
  extensions: &[Extension],
) -> Vec<LazyExtensionFile> {
  let mut out = Vec::new();
  for ext in extensions {
    for file in &*ext.lazy_loaded_js_files {
      if let Some(entry) = lazy_file_entry(file, LazyExtensionFileKind::Js) {
        out.push(entry);
      }
    }
    for file in &*ext.lazy_loaded_esm_files {
      if let Some(entry) = lazy_file_entry(file, LazyExtensionFileKind::Esm) {
        out.push(entry);
      }
    }
  }
  out.sort_by(|a, b| a.specifier.cmp(&b.specifier));
  out.dedup_by(|a, b| a.specifier == b.specifier);
  out
}

fn lazy_file_entry(
  file: &ExtensionFileSource,
  kind: LazyExtensionFileKind,
) -> Option<LazyExtensionFile> {
  #[allow(deprecated, reason = "matching deprecated variant we still inspect")]
  let path = match &file.code {
    ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(p) => PathBuf::from(p),
    // After the macro change every lazy entry is LoadedFromFsDuringSnapshot;
    // the other variants only appear for ad-hoc entries pushed by customizers
    // (e.g. inline sources) that have no on-disk path and therefore can't be
    // re-embedded by the build script.
    ExtensionFileSourceCode::IncludedInBinary(_)
    | ExtensionFileSourceCode::LoadedFromMemoryDuringSnapshot(_)
    | ExtensionFileSourceCode::Computed(_) => return None,
  };
  Some(LazyExtensionFile {
    specifier: file.specifier.to_string(),
    path,
    kind,
  })
}
