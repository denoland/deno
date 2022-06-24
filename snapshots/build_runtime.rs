// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use std::convert::TryFrom;
use std::path::Path;

use deno_runtime::deno_broadcast_channel;
use deno_runtime::deno_console;
use deno_runtime::deno_core;
use deno_runtime::deno_crypto;
use deno_runtime::deno_fetch;
use deno_runtime::deno_ffi;
use deno_runtime::deno_http;
use deno_runtime::deno_net;
use deno_runtime::deno_tls;
use deno_runtime::deno_url;
use deno_runtime::deno_web;
use deno_runtime::deno_webgpu;
use deno_runtime::deno_webidl;
use deno_runtime::deno_websocket;
use deno_runtime::deno_webstorage;

use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

pub fn create_runtime_snapshot(snapshot_path: &Path) {
  let extensions: Vec<Extension> = vec![
    deno_webidl::init(),
    deno_console::init(),
    deno_url::init(),
    deno_tls::init(),
    deno_web::init::<Permissions>(
      deno_web::BlobStore::default(),
      Default::default(),
    ),
    deno_fetch::init::<Permissions>(Default::default()),
    deno_websocket::init::<Permissions>("".to_owned(), None, None),
    deno_webstorage::init(None),
    deno_crypto::init(None),
    deno_webgpu::init(false),
    deno_broadcast_channel::init(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
      false, // No --unstable.
    ),
    deno_ffi::init::<Permissions>(false),
    deno_net::init::<Permissions>(
      None, false, // No --unstable.
      None,
    ),
    deno_http::init(),
    // Runtime JS
    deno_runtime::js::init(),
  ];

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions,
    ..Default::default()
  });
  write_runtime_snapshot(js_runtime, snapshot_path);
}

// TODO(bartlomieju): this module contains a lot of duplicated
// logic with `build_tsc.rs`
fn write_runtime_snapshot(mut js_runtime: JsRuntime, snapshot_path: &Path) {
  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());

  let compressed_snapshot_with_size = {
    let mut vec = vec![];

    vec.extend_from_slice(
      &u32::try_from(snapshot.len())
        .expect("snapshot larger than 4gb")
        .to_le_bytes(),
    );

    lzzzz::lz4_hc::compress_to_vec(
      snapshot_slice,
      &mut vec,
      lzzzz::lz4_hc::CLEVEL_MAX,
    )
    .expect("snapshot compression failed");

    vec
  };

  println!(
    "Snapshot compressed size: {}",
    compressed_snapshot_with_size.len()
  );

  std::fs::write(&snapshot_path, compressed_snapshot_with_size).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

struct Permissions;

impl deno_fetch::FetchPermissions for Permissions {
  fn check_net_url(
    &mut self,
    _url: &deno_core::url::Url,
  ) -> Result<(), deno_core::error::AnyError> {
    unreachable!("snapshotting!")
  }

  fn check_read(
    &mut self,
    _p: &Path,
  ) -> Result<(), deno_core::error::AnyError> {
    unreachable!("snapshotting!")
  }
}

impl deno_websocket::WebSocketPermissions for Permissions {
  fn check_net_url(
    &mut self,
    _url: &deno_core::url::Url,
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

impl deno_net::NetPermissions for Permissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
  ) -> Result<(), deno_core::error::AnyError> {
    unreachable!("snapshotting!")
  }

  fn check_read(
    &mut self,
    _p: &Path,
  ) -> Result<(), deno_core::error::AnyError> {
    unreachable!("snapshotting!")
  }

  fn check_write(
    &mut self,
    _p: &Path,
  ) -> Result<(), deno_core::error::AnyError> {
    unreachable!("snapshotting!")
  }
}
