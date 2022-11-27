// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::Path;
use std::path::PathBuf;

// This is a shim that allows to generate documentation on docs.rs
#[cfg(not(feature = "docsrs"))]
mod not_docs {
  use super::*;
  use deno_cache::SqliteBackedCache;
  use deno_core::snapshot_util::*;
  use deno_core::Extension;

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

    fn check_unstable(
      &self,
      _state: &deno_core::OpState,
      _api_name: &'static str,
    ) {
      unreachable!("snapshotting!")
    }
  }

  impl deno_ffi::FfiPermissions for Permissions {
    fn check(
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

  impl deno_flash::FlashPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
      &mut self,
      _host: &(T, Option<u16>),
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_node::NodePermissions for Permissions {
    fn check_read(
      &mut self,
      _p: &Path,
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

  fn create_runtime_snapshot(snapshot_path: PathBuf, files: Vec<PathBuf>) {
    let extensions_with_js: Vec<Extension> = vec![
      deno_webidl::init(),
      deno_console::init(),
      deno_url::init(),
      deno_tls::init(),
      deno_web::init::<Permissions>(
        deno_web::BlobStore::default(),
        Default::default(),
      ),
      deno_fetch::init::<Permissions>(Default::default()),
      deno_cache::init::<SqliteBackedCache>(None),
      deno_websocket::init::<Permissions>("".to_owned(), None, None),
      deno_webstorage::init(None),
      deno_crypto::init(None),
      deno_webgpu::init(false),
      deno_broadcast_channel::init(
        deno_broadcast_channel::InMemoryBroadcastChannel::default(),
        false, // No --unstable.
      ),
      deno_node::init::<Permissions>(None),
      deno_ffi::init::<Permissions>(false),
      deno_net::init::<Permissions>(
        None, false, // No --unstable.
        None,
      ),
      deno_napi::init::<Permissions>(false),
      deno_http::init(),
      deno_flash::init::<Permissions>(false), // No --unstable
    ];

    create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions: vec![],
      extensions_with_js,
      additional_files: files,
      compression_cb: Some(Box::new(|vec, snapshot_slice| {
        lzzzz::lz4_hc::compress_to_vec(
          snapshot_slice,
          vec,
          lzzzz::lz4_hc::CLEVEL_MAX,
        )
        .expect("snapshot compression failed");
      })),
    });
  }

  pub fn build_snapshot(runtime_snapshot_path: PathBuf) {
    let js_files = get_js_files(env!("CARGO_MANIFEST_DIR"), "js");
    create_runtime_snapshot(runtime_snapshot_path, js_files);
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
    std::fs::write(&runtime_snapshot_path, snapshot_slice).unwrap();
    return;
  }

  #[cfg(not(feature = "docsrs"))]
  not_docs::build_snapshot(runtime_snapshot_path)
}
