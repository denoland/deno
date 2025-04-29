// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::snapshot::*;
use deno_core::v8;
use deno_core::Extension;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::ops::bootstrap::SnapshotOptions;
use crate::shared::runtime;
use crate::snapshot_info::Permissions;

pub fn create_runtime_snapshot(
  snapshot_path: PathBuf,
  snapshot_options: SnapshotOptions,
  // NOTE: For embedders that wish to add additional extensions to the snapshot
  custom_extensions: Vec<Extension>,
) {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let fs = std::sync::Arc::new(deno_fs::RealFs);
  let mut extensions: Vec<Extension> = vec![
    deno_telemetry::deno_telemetry::init(),
    deno_webidl::deno_webidl::init(),
    deno_console::deno_console::init(),
    deno_url::deno_url::init(),
    deno_web::deno_web::init::<Permissions>(
      Default::default(),
      Default::default(),
    ),
    deno_webgpu::deno_webgpu::init(),
    deno_canvas::deno_canvas::init(),
    deno_fetch::deno_fetch::init::<Permissions>(Default::default()),
    deno_cache::deno_cache::init(None),
    deno_websocket::deno_websocket::init::<Permissions>(
      "".to_owned(),
      None,
      None,
    ),
    deno_webstorage::deno_webstorage::init(None),
    deno_crypto::deno_crypto::init(None),
    deno_broadcast_channel::deno_broadcast_channel::init(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
    ),
    deno_ffi::deno_ffi::init::<Permissions>(),
    deno_net::deno_net::init::<Permissions>(None, None),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::init(
      deno_kv::sqlite::SqliteDbHandler::<Permissions>::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init::<Permissions>(),
    deno_http::deno_http::init(deno_http::Options::default()),
    deno_io::deno_io::init(Default::default()),
    deno_fs::deno_fs::init::<Permissions>(fs.clone()),
    deno_os::deno_os::init(Default::default()),
    deno_process::deno_process::init(Default::default()),
    deno_node::deno_node::init::<
      Permissions,
      DenoInNpmPackageChecker,
      NpmResolver<sys_traits::impls::RealSys>,
      sys_traits::impls::RealSys,
    >(None, fs.clone()),
    ops::runtime::deno_runtime::init("deno:runtime".parse().unwrap()),
    ops::worker_host::deno_worker_host::init(
      Arc::new(|_| unreachable!("not used in snapshot.")),
      None,
    ),
    ops::fs_events::deno_fs_events::init(),
    ops::permissions::deno_permissions::init(),
    ops::tty::deno_tty::init(),
    ops::http::deno_http_runtime::init(),
    ops::bootstrap::deno_bootstrap::init(Some(snapshot_options)),
    runtime::init(),
    ops::web_worker::deno_web_worker::init(),
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
        let scope = &mut v8::HandleScope::new(isolate);

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
