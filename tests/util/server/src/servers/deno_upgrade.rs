// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::print_stderr)]

use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http::Response;
use http::StatusCode;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::Full;
use zip::ZipWriter;

use crate::root_path;
use crate::servers::hyper_utils::run_server;
use crate::servers::hyper_utils::ServerKind;
use crate::servers::hyper_utils::ServerOptions;
use crate::servers::string_body;

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
      let _: Vec<_> = parts.clone().collect();
      let _ = parts.next(); // empty
      let Some(channel) = parts.next() else {
        return not_found(format!("unexpected request path: {path}"));
      };

      let version;
      let file;
      match channel {
        "stable" => {
          let _ = parts.next(); // "download" string
          version = parts.next();
          file = parts.next();
        }
        "canary" => {
          version = parts.next();
          file = parts.next();
        }
        "rc" => {
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

      eprintln!("version {} file {}", version, file);
      let binary_path = root_path().join("target/debug/deno");
      eprintln!("binary path: {:?}", binary_path);
      let obj = std::fs::read(binary_path).unwrap();

      let mut zip_writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));

      let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);
      zip_writer.start_file("deno", options).unwrap();

      libsui::Macho::from(obj)
        .unwrap()
        .write_section("denover", channel.to_owned().into_bytes())
        .unwrap()
        .write_section("denoversion", version.to_owned().into_bytes())
        .unwrap()
        .build_and_sign(&mut zip_writer)
        .unwrap();

      let out = zip_writer.finish().unwrap().into_inner();

      Ok(Response::new(UnsyncBoxBody::new(Full::new(out.into()))))
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
