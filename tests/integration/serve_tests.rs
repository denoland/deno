// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[cfg(windows)]
use std::io::Read;

#[cfg(windows)]
use regex::Regex;
#[cfg(windows)]
use test_util as util;

#[cfg(windows)]
#[tokio::test]
async fn deno_serve_port_0() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("serve")
    .arg("--port")
    .arg("0")
    .arg("./serve/port_0.ts")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 52];
  let _read = stdout.read(&mut buffer).unwrap();
  let msg = std::str::from_utf8(&buffer).unwrap();
  let port_regex = Regex::new(r"(\d+)").unwrap();
  let port = port_regex.find(msg).unwrap().as_str();

  let cert = reqwest::Certificate::from_pem(include_bytes!(
    "../testdata/tls/RootCA.crt"
  ))
  .unwrap();

  let client = reqwest::Client::builder()
    .add_root_certificate(cert)
    .http2_prior_knowledge()
    .build()
    .unwrap();

  let res = client
    .get(&format!("http://127.0.0.1:{port}"))
    .send()
    .await
    .unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "deno serve --port 0 works!");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn deno_serve_no_args() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("serve")
    .arg("--port")
    .arg("0")
    .arg("./serve/no_args.ts")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 52];
  let _read = stdout.read(&mut buffer).unwrap();
  let msg = std::str::from_utf8(&buffer).unwrap();
  let port_regex = Regex::new(r"(\d+)").unwrap();
  let port = port_regex.find(msg).unwrap().as_str();

  let cert = reqwest::Certificate::from_pem(include_bytes!(
    "../testdata/tls/RootCA.crt"
  ))
  .unwrap();

  let client = reqwest::Client::builder()
    .add_root_certificate(cert)
    .http2_prior_knowledge()
    .build()
    .unwrap();

  let res = client
    .get(&format!("http://127.0.0.1:{port}"))
    .send()
    .await
    .unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "deno serve with no args in fetch() works!");

  child.kill().unwrap();
  child.wait().unwrap();
}
