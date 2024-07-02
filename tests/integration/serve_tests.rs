// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;

use deno_fetch::reqwest;
use pretty_assertions::assert_eq;
use regex::Regex;
use test_util as util;

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

#[tokio::test]
async fn deno_run_serve_with_tcp_from_env() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-env=DENO_SERVE_ADDRESS")
    .arg("--allow-net")
    .arg("./serve/run_serve.ts")
    .env("DENO_SERVE_ADDRESS", format!("tcp/127.0.0.1:0"))
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = BufReader::new(child.stdout.as_mut().unwrap());
  let msg = stdout.lines().next().unwrap().unwrap();

  // Deno.serve() listens on 0.0.0.0 by default. This checks DENO_SERVE_ADDRESS
  // is not ignored by ensuring it's listening on 127.0.0.1.
  let port_regex = Regex::new(r"http:\/\/127\.0\.0\.1:(\d+)").unwrap();
  let port = port_regex.captures(&msg).unwrap().get(1).unwrap().as_str();

  let client = reqwest::Client::builder().build().unwrap();

  let res = client
    .get(&format!("http://127.0.0.1:{port}"))
    .send()
    .await
    .unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "Deno.serve() works!");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
#[cfg(unix)]
async fn deno_run_serve_with_unix_socket_from_env() {
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;
  use tokio::net::UnixStream;

  let dir = tempfile::TempDir::new().unwrap();
  let sock = dir.path().join("listen.sock");
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-env=DENO_SERVE_ADDRESS")
    .arg(format!("--allow-read={}", sock.display()))
    .arg(format!("--allow-write={}", sock.display()))
    .arg("./serve/run_serve.ts")
    .env("DENO_SERVE_ADDRESS", format!("unix/{}", sock.display()))
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = BufReader::new(child.stdout.as_mut().unwrap());
  stdout.lines().next().unwrap().unwrap();

  // reqwest does not support connecting to unix sockets yet, so here we send the http
  // payload directly
  let mut conn = UnixStream::connect(dir.path().join("listen.sock"))
    .await
    .unwrap();
  conn.write_all(b"GET / HTTP/1.0\r\n\r\n").await.unwrap();
  let mut response = String::new();
  conn.read_to_string(&mut response).await.unwrap();
  assert!(response.ends_with("\r\nDeno.serve() works!"));

  child.kill().unwrap();
  child.wait().unwrap();
}
