// Copyright 2018-2025 the Deno authors. MIT license.

//! Server for NodeJS header tarballs, used by `node-gyp` in tests to download headers
//!
//! Loads from `testdata/assets`, if we update our node version in `process.versions` we'll need to
//! update the header tarball there.

#![allow(clippy::print_stderr)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::LazyLock;

use bytes::Bytes;
use http::Response;
use http::StatusCode;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::Full;
use parking_lot::Mutex;

use crate::servers::hyper_utils::run_server;
use crate::servers::hyper_utils::ServerKind;
use crate::servers::hyper_utils::ServerOptions;
use crate::servers::string_body;
use crate::testdata_path;
use crate::PathRef;

pub async fn deno_upgrade_test_server(port: u16) {
  let addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    ServerOptions {
      addr,
      error_msg: "deno upgrade test server error",
      kind: ServerKind::Auto,
    },
    |req| async move {
      let path = req.uri().path();

      let mut parts = path.split('/');
      let part1: Vec<_> = parts.clone().collect();
      eprintln!("parts {:#?}", part1);
      let _ = parts.next(); // empty
      let Some(channel) = parts.next() else {
        return not_found(format!("unexpected request path: {path}"));
      };

      let mut version = None;
      let mut file = None;
      let mut is_canary = false;

      match channel {
        "release" => {
          let _ = parts.next(); // "download" string
          version = parts.next();
          file = parts.next();
        }
        "canary" => {
          version = parts.next();
          file = parts.next();
          is_canary = true;
        }
        "rc_or_lts" => {
          version = parts.next();
          file = parts.next();
        }
        _ => {
          return not_found(format!("unexpected request path: {path}"));
        }
      }

      let Some(version) = version else {
        return not_found(format!("missing version in path: {path}"));
      };
      let Some(file) = file else {
        return not_found(format!("missing version in path: {path}"));
      };

      eprintln!("version {} canary? {} file {}", version, is_canary, file);
      not_found(format!("unexpected request path: {path}"))
    },
  )
  .await
}

fn not_found(
  msg: impl AsRef<str>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let msg = msg.as_ref();
  eprintln!(
    "test_server warning: error likely occurred in deno_upgrade.rs: {msg}"
  );
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(string_body(msg))
    .map_err(|e| e.into())
}
