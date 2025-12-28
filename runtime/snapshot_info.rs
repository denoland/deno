// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::Extension;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::shared::runtime;

pub fn get_extensions_in_snapshot() -> Vec<Extension> {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let fs = std::sync::Arc::new(deno_fs::RealFs);
  vec![
    deno_telemetry::deno_telemetry::init(),
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      Default::default(),
      Default::default(),
      deno_web::InMemoryBroadcastChannel::default(),
    ),
    deno_webgpu::deno_webgpu::init(),
    deno_canvas::deno_canvas::init(),
    deno_fetch::deno_fetch::init(Default::default()),
    deno_cache::deno_cache::init(None),
    deno_websocket::deno_websocket::init(),
    deno_webstorage::deno_webstorage::init(None),
    deno_crypto::deno_crypto::init(None),
    deno_ffi::deno_ffi::init(None),
    deno_net::deno_net::init(None, None),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::init(
      deno_kv::sqlite::SqliteDbHandler::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init(None),
    deno_http::deno_http::init(deno_http::Options::default()),
    deno_io::deno_io::init(Some(Default::default())),
    deno_fs::deno_fs::init(fs.clone()),
    deno_os::deno_os::init(Default::default()),
    deno_process::deno_process::init(Default::default()),
    deno_node::deno_node::init::<
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
    deno_bundle_runtime::deno_bundle_runtime::init(None),
    ops::bootstrap::deno_bootstrap::init(None, false),
    runtime::init(),
    ops::web_worker::deno_web_worker::init(),
  ]
}
