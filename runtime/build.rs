// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod shared;

use std::env;
use std::path::PathBuf;

#[cfg(all(
  not(feature = "docsrs"),
  not(feature = "dont_create_runtime_snapshot")
))]
mod startup_snapshot {
  use super::*;
  use deno_cache::SqliteBackedCache;
  use deno_core::error::AnyError;
  use deno_core::op2;
  use deno_core::snapshot_util::*;
  use deno_core::Extension;
  use deno_core::OpState;
  use deno_http::DefaultHttpPropertyExtractor;
  use shared::maybe_transpile_source;
  use shared::runtime;
  use std::path::Path;

  // Keep in sync with `runtime/ops/bootstrap.rs`
  #[derive(serde::Serialize, Default)]
  #[serde(rename_all = "camelCase")]
  pub struct SnapshotOptions {
    pub deno_version: String,
    pub ts_version: String,
    pub v8_version: &'static str,
    pub target: String,
  }

  // TODO(@littledivy): Remove this once we get rid of deno_runtime snapshots.
  #[op2]
  #[serde]
  pub fn op_snapshot_options(_: &mut OpState) -> SnapshotOptions {
    SnapshotOptions::default()
  }

  deno_core::extension!(snapshot, ops = [op_snapshot_options],);

  #[derive(Clone)]
  struct Permissions;

  impl deno_fetch::FetchPermissions for Permissions {
    fn check_net_url(
      &mut self,
      _url: &deno_core::url::Url,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_websocket::WebSocketPermissions for Permissions {
    fn check_net_url(
      &mut self,
      _url: &deno_core::url::Url,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_web::TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
      unreachable!("snapshotting!")
    }
  }

  impl deno_ffi::FfiPermissions for Permissions {
    fn check_partial(
      &mut self,
      _path: Option<&Path>,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_napi::NapiPermissions for Permissions {
    fn check(
      &mut self,
      _path: Option<&Path>,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_node::NodePermissions for Permissions {
    fn check_net_url(
      &mut self,
      _url: &deno_core::url::Url,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
    fn check_read(&self, _p: &Path) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
    fn check_read_with_api_name(
      &self,
      _: &Path,
      _: std::option::Option<&str>,
    ) -> Result<(), deno_core::anyhow::Error> {
      unreachable!("snapshotting!")
    }
    fn check_sys(
      &self,
      _kind: &str,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_net::NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
      &mut self,
      _host: &(T, Option<u16>),
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_fs::FsPermissions for Permissions {
    fn check_read(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read_blind(
      &mut self,
      _path: &Path,
      _display: &str,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_partial(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_blind(
      &mut self,
      _path: &Path,
      _display: &str,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_kv::sqlite::SqliteDbHandlerPermissions for Permissions {
    fn check_read(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }
  }

  pub fn create_runtime_snapshot(snapshot_path: PathBuf) {
    // NOTE(bartlomieju): ordering is important here, keep it in sync with
    // `runtime/worker.rs`, `runtime/web_worker.rs` and `runtime/snapshot.rs`!
    let fs = std::sync::Arc::new(deno_fs::RealFs);
    let mut extensions: Vec<Extension> = vec![
      deno_webidl::deno_webidl::init_ops_and_esm(),
      deno_console::deno_console::init_ops_and_esm(),
      deno_url::deno_url::init_ops_and_esm(),
      deno_web::deno_web::init_ops_and_esm::<Permissions>(
        Default::default(),
        Default::default(),
      ),
      deno_webgpu::deno_webgpu::init_ops_and_esm(),
      deno_fetch::deno_fetch::init_ops_and_esm::<Permissions>(
        Default::default(),
      ),
      deno_cache::deno_cache::init_ops_and_esm::<SqliteBackedCache>(None),
      deno_websocket::deno_websocket::init_ops_and_esm::<Permissions>(
        "".to_owned(),
        None,
        None,
      ),
      deno_webstorage::deno_webstorage::init_ops_and_esm(None),
      deno_crypto::deno_crypto::init_ops_and_esm(None),
      deno_broadcast_channel::deno_broadcast_channel::init_ops_and_esm(
        deno_broadcast_channel::InMemoryBroadcastChannel::default(),
      ),
      deno_ffi::deno_ffi::init_ops_and_esm::<Permissions>(),
      deno_net::deno_net::init_ops_and_esm::<Permissions>(None, None),
      deno_tls::deno_tls::init_ops_and_esm(),
      deno_kv::deno_kv::init_ops_and_esm(deno_kv::sqlite::SqliteDbHandler::<
        Permissions,
      >::new(None, None)),
      deno_cron::deno_cron::init_ops_and_esm(
        deno_cron::local::LocalCronHandler::new(),
      ),
      deno_napi::deno_napi::init_ops_and_esm::<Permissions>(),
      deno_http::deno_http::init_ops_and_esm::<DefaultHttpPropertyExtractor>(),
      deno_io::deno_io::init_ops_and_esm(Default::default()),
      deno_fs::deno_fs::init_ops_and_esm::<Permissions>(fs.clone()),
      deno_node::deno_node::init_ops_and_esm::<Permissions>(None, fs),
      runtime::init_ops_and_esm(),
      snapshot::init_ops_and_esm(),
    ];

    for extension in &mut extensions {
      for source in extension.esm_files.to_mut() {
        maybe_transpile_source(source).unwrap();
      }
      for source in extension.js_files.to_mut() {
        maybe_transpile_source(source).unwrap();
      }
    }

    let output = create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions,
      compression_cb: None,
      with_runtime_cb: None,
      skip_op_registration: false,
    });
    for path in output.files_loaded_during_snapshot {
      println!("cargo:rerun-if-changed={}", path.display());
    }
  }
}

fn main() {
  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

  // If we're building on docs.rs we just create
  // and empty snapshot file and return, because `rusty_v8`
  // doesn't actually compile on docs.rs
  if env::var_os("DOCS_RS").is_some() {
    let snapshot_slice = &[];
    #[allow(clippy::needless_borrow)]
    #[allow(clippy::disallowed_methods)]
    #[allow(clippy::needless_borrows_for_generic_args)]
    std::fs::write(&runtime_snapshot_path, snapshot_slice).unwrap();
  }

  #[cfg(all(
    not(feature = "docsrs"),
    not(feature = "dont_create_runtime_snapshot")
  ))]
  startup_snapshot::create_runtime_snapshot(runtime_snapshot_path)
}
