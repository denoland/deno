// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_runtime::ops::tls::rustls;
use deno_runtime::ops::tls::webpki;
use deno_runtime::ops::tls::TlsStream;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::sync::Arc;
use test_util as util;
use tokio::task::LocalSet;

mod integration;

#[test]
fn typecheck_declarations_ns() {
  let status = util::deno_cmd()
    .arg("test")
    .arg("--doc")
    .arg(util::root_path().join("cli/dts/lib.deno.ns.d.ts"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn typecheck_declarations_unstable() {
  let status = util::deno_cmd()
    .arg("test")
    .arg("--doc")
    .arg("--unstable")
    .arg(util::root_path().join("cli/dts/lib.deno.unstable.d.ts"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn js_unit_tests_lint() {
  let status = util::deno_cmd()
    .arg("lint")
    .arg("--unstable")
    .arg(util::root_path().join("cli/tests/unit"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn js_unit_tests() {
  let _g = util::http_server();

  // Note that the unit tests are not safe for concurrency and must be run with a concurrency limit
  // of one because there are some chdir tests in there.
  // TODO(caspervonb) split these tests into two groups: parallel and serial.
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    .arg("--location=http://js-unit-tests/foo/bar")
    .arg("-A")
    .arg("cli/tests/unit")
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}

#[tokio::test]
async fn listen_tls_alpn() {
  // TLS streams require the presence of an ambient local task set to gracefully
  // close dropped connections in the background.
  LocalSet::new()
    .run_until(async {
      let mut child = util::deno_cmd()
        .current_dir(util::root_path())
        .arg("run")
        .arg("--unstable")
        .arg("--quiet")
        .arg("--allow-net")
        .arg("--allow-read")
        .arg("./cli/tests/listen_tls_alpn.ts")
        .arg("4504")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
      let stdout = child.stdout.as_mut().unwrap();
      let mut buffer = [0; 5];
      let read = stdout.read(&mut buffer).unwrap();
      assert_eq!(read, 5);
      let msg = std::str::from_utf8(&buffer).unwrap();
      assert_eq!(msg, "READY");

      let mut cfg = rustls::ClientConfig::new();
      let reader =
        &mut BufReader::new(Cursor::new(include_bytes!("./tls/RootCA.crt")));
      cfg.root_store.add_pem_file(reader).unwrap();
      cfg.alpn_protocols.push("foobar".as_bytes().to_vec());
      let cfg = Arc::new(cfg);

      let hostname =
        webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();

      let tcp_stream = tokio::net::TcpStream::connect("localhost:4504")
        .await
        .unwrap();
      let mut tls_stream =
        TlsStream::new_client_side(tcp_stream, &cfg, hostname);
      tls_stream.handshake().await.unwrap();
      let (_, session) = tls_stream.get_ref();

      let alpn = session.get_alpn_protocol().unwrap();
      assert_eq!(std::str::from_utf8(alpn).unwrap(), "foobar");

      child.kill().unwrap();
      child.wait().unwrap();
    })
    .await;
}

#[tokio::test]
async fn listen_tls_alpn_fail() {
  // TLS streams require the presence of an ambient local task set to gracefully
  // close dropped connections in the background.
  LocalSet::new()
    .run_until(async {
      let mut child = util::deno_cmd()
        .current_dir(util::root_path())
        .arg("run")
        .arg("--unstable")
        .arg("--quiet")
        .arg("--allow-net")
        .arg("--allow-read")
        .arg("./cli/tests/listen_tls_alpn.ts")
        .arg("4505")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
      let stdout = child.stdout.as_mut().unwrap();
      let mut buffer = [0; 5];
      let read = stdout.read(&mut buffer).unwrap();
      assert_eq!(read, 5);
      let msg = std::str::from_utf8(&buffer).unwrap();
      assert_eq!(msg, "READY");

      let mut cfg = rustls::ClientConfig::new();
      let reader =
        &mut BufReader::new(Cursor::new(include_bytes!("./tls/RootCA.crt")));
      cfg.root_store.add_pem_file(reader).unwrap();
      cfg.alpn_protocols.push("boofar".as_bytes().to_vec());
      let cfg = Arc::new(cfg);

      let hostname =
        webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();

      let tcp_stream = tokio::net::TcpStream::connect("localhost:4505")
        .await
        .unwrap();
      let mut tls_stream =
        TlsStream::new_client_side(tcp_stream, &cfg, hostname);
      tls_stream.handshake().await.unwrap();
      let (_, session) = tls_stream.get_ref();

      assert!(session.get_alpn_protocol().is_none());

      child.kill().unwrap();
      child.wait().unwrap();
    })
    .await;
}
