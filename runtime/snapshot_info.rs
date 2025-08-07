// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use deno_core::Extension;
use deno_permissions::CheckedPath;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionCheckError;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;

use crate::ops;
use crate::shared::runtime;

#[derive(Clone)]
pub struct Permissions;

impl deno_websocket::WebSocketPermissions for Permissions {
  fn check_net_url(
    &mut self,
    _url: &deno_core::url::Url,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_web::TimersPermission for Permissions {
  fn allow_hrtime(&mut self) -> bool {
    unreachable!("snapshotting!")
  }
}

impl deno_fetch::FetchPermissions for Permissions {
  fn check_net_url(
    &mut self,
    _url: &deno_core::url::Url,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_open<'a>(
    &mut self,
    _path: Cow<'a, Path>,
    _open_access: OpenAccessKind,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_net_vsock(
    &mut self,
    _cid: u32,
    _port: u32,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_ffi::FfiPermissions for Permissions {
  fn check_partial_no_path(&mut self) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_partial_with_path<'a>(
    &mut self,
    _path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_napi::NapiPermissions for Permissions {
  fn check<'a>(
    &mut self,
    _path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_node::NodePermissions for Permissions {
  fn check_net_url(
    &mut self,
    _url: &deno_core::url::Url,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
  fn check_net(
    &mut self,
    _host: (&str, Option<u16>),
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
  fn check_open<'a>(
    &mut self,
    _path: Cow<'a, Path>,
    _open_access: OpenAccessKind,
    _api_name: Option<&str>,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
  fn query_read_all(&mut self) -> bool {
    unreachable!("snapshotting!")
  }
  fn check_sys(
    &mut self,
    _kind: &str,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_net::NetPermissions for Permissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_open<'a>(
    &mut self,
    _path: Cow<'a, Path>,
    _open_access: OpenAccessKind,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_vsock(
    &mut self,
    _cid: u32,
    _port: u32,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_fs::FsPermissions for Permissions {
  fn check_open<'a>(
    &self,
    _path: Cow<'a, Path>,
    _access_kind: OpenAccessKind,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_open_blind<'a>(
    &self,
    _path: Cow<'a, Path>,
    _access_kind: OpenAccessKind,
    _display: &str,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_read_all(
    &self,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_partial<'a>(
    &self,
    _path: Cow<'a, Path>,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_all(
    &self,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_kv::sqlite::SqliteDbHandlerPermissions for Permissions {
  fn check_open<'a>(
    &mut self,
    _p: Cow<'a, Path>,
    _open_access: OpenAccessKind,
    _api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    unreachable!("snapshotting!");
  }
}

pub fn get_extensions_in_snapshot() -> Vec<Extension> {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let fs = std::sync::Arc::new(deno_fs::RealFs);
  vec![
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
    deno_ffi::deno_ffi::init::<Permissions>(None),
    deno_net::deno_net::init::<Permissions>(None, None),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::init(
      deno_kv::sqlite::SqliteDbHandler::<Permissions>::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init::<Permissions>(None),
    deno_http::deno_http::init(deno_http::Options::default()),
    deno_io::deno_io::init(Some(Default::default())),
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
    ops::bootstrap::deno_bootstrap::init(None, false),
    runtime::init(),
    ops::web_worker::deno_web_worker::init(),
  ]
}
