// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::Extension;
use deno_io::fs::FsError;
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

  fn check_read<'a>(
    &mut self,
    _resolved: bool,
    _p: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, FsError> {
    unreachable!("snapshotting!")
  }
}

impl deno_ffi::FfiPermissions for Permissions {
  fn check_partial_no_path(&mut self) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_partial_with_path(
    &mut self,
    _path: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_napi::NapiPermissions for Permissions {
  fn check(&mut self, _path: &str) -> Result<PathBuf, PermissionCheckError> {
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
  fn check_read_path<'a>(
    &mut self,
    _path: &'a Path,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
  fn check_read_with_api_name(
    &mut self,
    _p: &str,
    _api_name: Option<&str>,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
  fn query_read_all(&mut self) -> bool {
    unreachable!("snapshotting!")
  }
  fn check_write_with_api_name(
    &mut self,
    _p: &str,
    _api_name: Option<&str>,
  ) -> Result<PathBuf, PermissionCheckError> {
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

  fn check_read(
    &mut self,
    _p: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write(
    &mut self,
    _p: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_path<'a>(
    &mut self,
    _p: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
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
    &mut self,
    _resolved: bool,
    _read: bool,
    _write: bool,
    _path: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, FsError> {
    unreachable!("snapshotting!")
  }

  fn check_read(
    &mut self,
    _path: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_read_all(
    &mut self,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_read_blind(
    &mut self,
    _path: &Path,
    _display: &str,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write(
    &mut self,
    _path: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_partial(
    &mut self,
    _path: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_all(
    &mut self,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_blind(
    &mut self,
    _path: &Path,
    _display: &str,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_read_path<'a>(
    &mut self,
    _path: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write_path<'a>(
    &mut self,
    _path: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

impl deno_kv::sqlite::SqliteDbHandlerPermissions for Permissions {
  fn check_read(
    &mut self,
    _path: &str,
    _api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    unreachable!("snapshotting!")
  }

  fn check_write<'a>(
    &mut self,
    _path: &'a Path,
    _api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    unreachable!("snapshotting!")
  }
}

pub fn get_extensions_in_snapshot() -> Vec<Extension> {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  let fs = std::sync::Arc::new(deno_fs::RealFs);
  vec![
    deno_telemetry::deno_telemetry::init_ops(),
    deno_webidl::deno_webidl::init_ops(),
    deno_console::deno_console::init_ops(),
    deno_url::deno_url::init_ops(),
    deno_web::deno_web::init_ops::<Permissions>(
      Default::default(),
      Default::default(),
    ),
    deno_webgpu::deno_webgpu::init_ops(),
    deno_canvas::deno_canvas::init_ops(),
    deno_fetch::deno_fetch::init_ops::<Permissions>(Default::default()),
    deno_cache::deno_cache::init_ops(None),
    deno_websocket::deno_websocket::init_ops::<Permissions>(
      "".to_owned(),
      None,
      None,
    ),
    deno_webstorage::deno_webstorage::init_ops(None),
    deno_crypto::deno_crypto::init_ops(None),
    deno_broadcast_channel::deno_broadcast_channel::init_ops(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
    ),
    deno_ffi::deno_ffi::init_ops::<Permissions>(None),
    deno_net::deno_net::init_ops::<Permissions>(None, None),
    deno_tls::deno_tls::init_ops(),
    deno_kv::deno_kv::init_ops(
      deno_kv::sqlite::SqliteDbHandler::<Permissions>::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init_ops(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init_ops::<Permissions>(None),
    deno_http::deno_http::init_ops(deno_http::Options::default()),
    deno_io::deno_io::init_ops(Some(Default::default())),
    deno_fs::deno_fs::init_ops::<Permissions>(fs.clone()),
    deno_os::deno_os::init_ops(Default::default()),
    deno_process::deno_process::init_ops(Default::default()),
    deno_node::deno_node::init_ops::<
      Permissions,
      DenoInNpmPackageChecker,
      NpmResolver<sys_traits::impls::RealSys>,
      sys_traits::impls::RealSys,
    >(None, fs.clone()),
    runtime::init_ops(),
    ops::runtime::deno_runtime::init_ops("deno:runtime".parse().unwrap()),
    ops::worker_host::deno_worker_host::init_ops(
      Arc::new(|_| unreachable!("not used in snapshot.")),
      None,
    ),
    ops::fs_events::deno_fs_events::init_ops(),
    ops::permissions::deno_permissions::init_ops(),
    ops::tty::deno_tty::init_ops(),
    ops::http::deno_http_runtime::init_ops(),
    ops::bootstrap::deno_bootstrap::init_ops(None),
    ops::web_worker::deno_web_worker::init_ops(),
  ]
}
