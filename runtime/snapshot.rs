// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::Extension;
use deno_core::snapshot::*;
use deno_core::v8;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::ops::bootstrap::SnapshotOptions;
use crate::shared::runtime;

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
    deno_canvas::deno_canvas::lazy_init(),
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

  #[allow(clippy::print_stdout)]
  for path in output.files_loaded_during_snapshot {
    println!("cargo:rerun-if-changed={}", path.display());
  }
}
