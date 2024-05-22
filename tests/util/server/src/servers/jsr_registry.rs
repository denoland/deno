// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::tests_path;

use super::run_server;
use super::ServerKind;
use super::ServerOptions;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine as _;
use bytes::Bytes;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::Empty;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Mutex;

pub async fn registry_server(port: u16) {
  let registry_server_addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    ServerOptions {
      addr: registry_server_addr,
      error_msg: "Registry server error",
      kind: ServerKind::Auto,
    },
    registry_server_handler,
  )
  .await
}

pub async fn provenance_mock_server(port: u16) {
  let addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    ServerOptions {
      addr,
      error_msg: "Provenance mock server error",
      kind: ServerKind::Auto,
    },
    provenance_mock_server_handler,
  )
  .await
}

async fn provenance_mock_server_handler(
  req: Request<Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let path = req.uri().path();

  // OIDC request
  if path.starts_with("/gha_oidc") {
    let jwt_claim = json!({
      "sub": "divy",
      "email": "divy@deno.com",
      "iss": "https://github.com",
    });
    let token = format!(
      "AAA.{}.",
      STANDARD_NO_PAD.encode(serde_json::to_string(&jwt_claim).unwrap())
    );
    let body = serde_json::to_string_pretty(&json!({
      "value": token,
    }));
    let res = Response::new(UnsyncBoxBody::new(Full::from(body.unwrap())));
    return Ok(res);
  }

  // Fulcio
  if path.starts_with("/api/v2/signingCert") {
    let body = serde_json::to_string_pretty(&json!({
      "signedCertificateEmbeddedSct": {
        "chain": {
          "certificates": [
            "fake_certificate"
          ]
        }
      }
    }));
    let res = Response::new(UnsyncBoxBody::new(Full::from(body.unwrap())));
    return Ok(res);
  }

  // Rekor
  if path.starts_with("/api/v1/log/entries") {
    let body = serde_json::to_string_pretty(&json!({
      "transparency_log_1": {
        "logID": "test_log_id",
        "logIndex": 42069,
      }
    }));
    let res = Response::new(UnsyncBoxBody::new(Full::from(body.unwrap())));
    return Ok(res);
  }

  let empty_body = UnsyncBoxBody::new(Empty::new());
  let res = Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(empty_body)?;
  Ok(res)
}

async fn registry_server_handler(
  req: Request<Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let path = req.uri().path();

  // TODO(bartlomieju): add a proper router here
  if path.starts_with("/api/scope/") {
    let body = serde_json::to_string_pretty(&json!({})).unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  } else if path.starts_with("/api/scopes/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-sffg-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  } else if path.starts_with("/api/publish_status/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-qwer-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  }

  // serve the registry package files
  let mut file_path = tests_path().join("registry").join("jsr").to_path_buf();
  file_path.push(
    &req.uri().path()[1..]
      .replace("%2f", "/")
      .replace("%2F", "/"),
  );

  if let Ok(body) = tokio::fs::read(&file_path).await {
    let body = if let Some(version) = file_path
      .file_name()
      .unwrap()
      .to_string_lossy()
      .strip_suffix("_meta.json")
    {
      // fill the manifest with checksums found in the directory so that
      // we don't need to maintain them manually in the testdata directory
      let mut meta: serde_json::Value = serde_json::from_slice(&body)?;
      let mut manifest =
        manifest_sorted(meta.get("manifest").cloned().unwrap_or(json!({})));
      let version_dir = file_path.parent().unwrap().join(version);
      fill_manifest_at_dir(&mut manifest, &version_dir);
      meta
        .as_object_mut()
        .unwrap()
        .insert("manifest".to_string(), json!(manifest));
      serde_json::to_string(&meta).unwrap().into_bytes()
    } else {
      body
    };
    return Ok(Response::new(UnsyncBoxBody::new(
      http_body_util::Full::new(Bytes::from(body)),
    )));
  }

  let empty_body = UnsyncBoxBody::new(Empty::new());
  let res = Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(empty_body)?;
  Ok(res)
}

fn manifest_sorted(
  meta: serde_json::Value,
) -> BTreeMap<String, serde_json::Value> {
  let mut manifest = BTreeMap::new();
  if let serde_json::Value::Object(files) = meta {
    for (file, checksum) in files {
      manifest.insert(file.clone(), checksum.clone());
    }
  }
  manifest
}

fn fill_manifest_at_dir(
  manifest: &mut BTreeMap<String, serde_json::Value>,
  dir: &Path,
) {
  let file_system_manifest = get_manifest_entries_for_dir(dir);
  for (file_path, value) in file_system_manifest {
    manifest.entry(file_path).or_insert(value);
  }
}

static DIR_MANIFEST_CACHE: Lazy<
  Mutex<HashMap<String, BTreeMap<String, serde_json::Value>>>,
> = Lazy::new(Default::default);

fn get_manifest_entries_for_dir(
  dir: &Path,
) -> BTreeMap<String, serde_json::Value> {
  fn inner_fill(
    root_dir: &Path,
    dir: &Path,
    manifest: &mut BTreeMap<String, serde_json::Value>,
  ) {
    for entry in std::fs::read_dir(dir).unwrap() {
      let entry = entry.unwrap();
      let path = entry.path();
      if path.is_file() {
        let file_bytes = std::fs::read(&path).unwrap();
        let checksum = format!("sha256-{}", get_checksum(&file_bytes));
        let relative_path = path
          .to_string_lossy()
          .strip_prefix(&root_dir.to_string_lossy().to_string())
          .unwrap()
          .replace('\\', "/");
        manifest.insert(
          relative_path,
          json!({
            "size": file_bytes.len(),
            "checksum": checksum,
          }),
        );
      } else if path.is_dir() {
        inner_fill(root_dir, &path, manifest);
      }
    }
  }

  DIR_MANIFEST_CACHE
    .lock()
    .unwrap()
    .entry(dir.to_string_lossy().to_string())
    .or_insert_with(|| {
      let mut manifest = BTreeMap::new();
      inner_fill(dir, dir, &mut manifest);
      manifest
    })
    .clone()
}

fn get_checksum(bytes: &[u8]) -> String {
  use sha2::Digest;
  let mut hasher = sha2::Sha256::new();
  hasher.update(bytes);
  format!("{:x}", hasher.finalize())
}
