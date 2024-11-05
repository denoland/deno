// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

pub static NODEJS_MIRROR: LazyLock<NodeJsMirror> =
  LazyLock::new(|| NodeJsMirror::default());

#[derive(Default)]
pub struct NodeJsMirror {
  cache: Mutex<HashMap<String, Bytes>>,
}

impl NodeJsMirror {
  pub fn get_header_bytes(&self, file: &str, version: &str) -> Option<Bytes> {
    let mut cache = self.cache.lock();
    let entry = cache.entry(version.into());
    match entry {
      std::collections::hash_map::Entry::Occupied(occupied) => {
        Some(occupied.get().clone())
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let contents = testdata_path().join("assets").join(file);
        let contents = contents
          .read_to_bytes_if_exists()
          .ok()
          .map(|b| Bytes::from(b))?;
        vacant.insert(contents.clone());
        Some(contents)
      }
    }
  }
  pub fn get_header_checksum(&self, version: &str) -> Option<String> {
    let file = format!("{version}-headers.tar.gz");
    let bytes = self.get_header_bytes(&file, version)?;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    Some(faster_hex::hex_string(hasher.finalize().as_ref()))
  }
}

/// Server for node JS header tarballs, used by `node-gyp` in tests
pub async fn nodejs_org_mirror(port: u16) {
  let addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    ServerOptions {
      addr,
      error_msg: "nodejs mirror server error",
      kind: ServerKind::Auto,
    },
    |req| async move {
      let path = req.uri().path();
      if path.contains("-headers.tar.gz") || path.contains("SHASUMS256.txt") {
        let mut parts = path.split('/');
        let _ = parts.next(); // empty
        let Some(version) = parts.next() else {
          return not_found(format!("missing node version in path: {path}"));
        };
        // node header download
        let Some(file) = parts.next() else {
          return not_found(format!("missing file version in path: {path}"));
        };
        if file == "SHASUMS256.txt" {
          let Some(checksum) = NODEJS_MIRROR.get_header_checksum(version)
          else {
            return not_found(format!("failed to get header checksum: {path}"));
          };
          let checksum_file =
            format!("{checksum}  node-{version}-headers.tar.gz\n");
          return Ok(Response::new(string_body(&checksum_file)));
        }
        let Some(bytes) = NODEJS_MIRROR.get_header_bytes(file, version) else {
          return not_found(format!(
            "couldn't find headers for version {version}, missing file: {file}"
          ));
        };
        Ok(Response::new(UnsyncBoxBody::new(Full::new(bytes))))
      } else {
        return not_found(format!("unexpected request path: {path}"));
      }
    },
  )
  .await
}

fn not_found(
  msg: impl AsRef<str>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(string_body(msg.as_ref()))
    .map_err(|e| e.into())
}
