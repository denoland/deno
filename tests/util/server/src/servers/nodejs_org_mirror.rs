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
use http_body_util::Full;
use http_body_util::combinators::UnsyncBoxBody;
use parking_lot::Mutex;

use crate::PathRef;
use crate::servers::hyper_utils::ServerKind;
use crate::servers::hyper_utils::ServerOptions;
use crate::servers::hyper_utils::run_server;
use crate::servers::string_body;
use crate::testdata_path;

/// a little helper extension trait to log errors but convert to option
trait OkWarn<T, E> {
  fn ok_warn(self) -> Option<T>;
}

impl<T, E> OkWarn<T, E> for Result<T, E>
where
  E: std::fmt::Display,
{
  fn ok_warn(self) -> Option<T> {
    self
      .inspect_err(|err| {
        eprintln!(
          "test_server warning: error occurred in nodejs_org_mirror.rs: {err}"
        )
      })
      .ok()
  }
}

pub static NODEJS_MIRROR: LazyLock<NodeJsMirror> =
  LazyLock::new(NodeJsMirror::default);

#[derive(Default)]
pub struct NodeJsMirror {
  cache: Mutex<HashMap<String, Bytes>>,
  checksum_cache: Mutex<HashMap<String, String>>,
}

fn asset_file_path(file: &str) -> PathRef {
  testdata_path().join("assets").join("node-gyp").join(file)
}

impl NodeJsMirror {
  pub fn get_header_bytes(&self, file: &str) -> Option<Bytes> {
    let mut cache = self.cache.lock();
    let entry = cache.entry(file.to_owned());
    match entry {
      std::collections::hash_map::Entry::Occupied(occupied) => {
        Some(occupied.get().clone())
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let contents = asset_file_path(file);
        let contents = contents
          .read_to_bytes_if_exists()
          .ok_warn()
          .map(Bytes::from)?;
        vacant.insert(contents.clone());
        Some(contents)
      }
    }
  }

  fn get_checksum(&self, file: &str, bytes: Bytes) -> String {
    use sha2::Digest;
    if let Some(checksum) = self.checksum_cache.lock().get(file).cloned() {
      return checksum;
    }
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let checksum = faster_hex::hex_string(hasher.finalize().as_ref());
    self
      .checksum_cache
      .lock()
      .insert(file.to_owned(), checksum.clone());
    checksum
  }

  pub fn get_checksum_file(&self, version: &str) -> Option<String> {
    let mut entries = Vec::with_capacity(2);

    let header_file = header_tar_name(version);
    let header_bytes = self.get_header_bytes(&header_file)?;
    let header_checksum = self.get_checksum(&header_file, header_bytes);
    entries.push((header_file, header_checksum));

    if cfg!(windows) {
      if !cfg!(target_arch = "x86_64") {
        panic!("unsupported target arch on windows, only support x86_64");
      }
      let Some(bytes) = self.get_node_lib_bytes(version, "win-x64") else {
        eprintln!("test server failed to get node lib");
        return None;
      };
      {
        let file = format!("{version}/win-x64/node.lib");
        let checksum = self.get_checksum(&file, bytes);
        let filename_for_checksum =
          file.trim_start_matches(&format!("{version}/"));
        entries.push((filename_for_checksum.to_owned(), checksum));
      }
    }

    Some(
      entries
        .into_iter()
        .map(|(file, checksum)| format!("{checksum}  {file}"))
        .collect::<Vec<_>>()
        .join("\n"),
    )
  }

  pub fn get_node_lib_bytes(
    &self,
    version: &str,
    platform: &str,
  ) -> Option<Bytes> {
    let mut cache = self.cache.lock();
    let file_name = format!("{version}/{platform}/node.lib");
    let entry = cache.entry(file_name);
    match entry {
      std::collections::hash_map::Entry::Occupied(occupied) => {
        Some(occupied.get().clone())
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let tarball_filename =
          format!("{version}__{platform}__node.lib.tar.gz");
        let contents = asset_file_path(&tarball_filename);
        let contents = contents.read_to_bytes_if_exists().ok_warn()?;
        let extracted = Bytes::from(extract_tarball(&contents)?);
        vacant.insert(extracted.clone());
        Some(extracted)
      }
    }
  }
}

fn header_tar_name(version: &str) -> String {
  format!("node-{version}-headers.tar.gz")
}

fn extract_tarball(compressed: &[u8]) -> Option<Vec<u8>> {
  let mut out = Vec::with_capacity(compressed.len());
  let decoder = flate2::read::GzDecoder::new(compressed);
  let mut archive = tar::Archive::new(decoder);
  for file in archive.entries().ok_warn()? {
    let mut file = file.ok_warn()?;

    std::io::copy(&mut file, &mut out).ok_warn()?;
  }
  Some(out)
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
      if path.contains("-headers.tar.gz")
        || path.contains("SHASUMS256.txt")
        || path.contains("node.lib")
      {
        let mut parts = path.split('/');
        let _ = parts.next(); // empty
        let Some(version) = parts.next() else {
          return not_found(format!("missing node version in path: {path}"));
        };
        let Some(file) = parts.next() else {
          return not_found(format!("missing file version in path: {path}"));
        };
        if file == "SHASUMS256.txt" {
          let Some(checksum_file) = NODEJS_MIRROR.get_checksum_file(version)
          else {
            return not_found(format!("failed to get header checksum: {path}"));
          };
          return Ok(Response::new(string_body(&checksum_file)));
        } else if !file.contains("headers") {
          let platform = file;
          let Some(file) = parts.next() else {
            return not_found("expected file");
          };
          if file != "node.lib" {
            return not_found(format!(
              "unexpected file name, expected node.lib, got: {file}"
            ));
          }
          let Some(bytes) = NODEJS_MIRROR.get_node_lib_bytes(version, platform)
          else {
            return not_found("expected node lib bytes");
          };

          return Ok(Response::new(UnsyncBoxBody::new(Full::new(bytes))));
        }

        let Some(bytes) = NODEJS_MIRROR.get_header_bytes(file) else {
          return not_found(format!(
            "couldn't find headers for version {version}, missing file: {file}"
          ));
        };
        Ok(Response::new(UnsyncBoxBody::new(Full::new(bytes))))
      } else {
        not_found(format!("unexpected request path: {path}"))
      }
    },
  )
  .await
}

fn not_found(
  msg: impl AsRef<str>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let msg = msg.as_ref();
  eprintln!(
    "test_server warning: error likely occurred in nodejs_org_mirror.rs: {msg}"
  );
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(string_body(msg))
    .map_err(|e| e.into())
}
