// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod integration;

use deno_runtime::deno_net::ops_tls::TlsStream;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls_pemfile;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use test_util as util;
use test_util::TempDir;
use tokio::task::LocalSet;

mod cert {
  use super::*;
  itest_flaky!(cafile_url_imports {
    args:
      "run --quiet --reload --cert tls/RootCA.pem cert/cafile_url_imports.ts",
    output: "cert/cafile_url_imports.ts.out",
    http_server: true,
  });

  itest_flaky!(cafile_ts_fetch {
    args:
      "run --quiet --reload --allow-net --cert tls/RootCA.pem cert/cafile_ts_fetch.ts",
    output: "cert/cafile_ts_fetch.ts.out",
    http_server: true,
  });

  itest_flaky!(cafile_eval {
    args: "eval --cert tls/RootCA.pem fetch('https://localhost:5545/cert/cafile_ts_fetch.ts.out').then(r=>r.text()).then(t=>console.log(t.trimEnd()))",
    output: "cert/cafile_ts_fetch.ts.out",
    http_server: true,
  });

  itest_flaky!(cafile_info {
    args:
      "info --quiet --cert tls/RootCA.pem https://localhost:5545/cert/cafile_info.ts",
    output: "cert/cafile_info.ts.out",
    http_server: true,
  });

  itest_flaky!(cafile_url_imports_unsafe_ssl {
    args: "run --quiet --reload --unsafely-ignore-certificate-errors=localhost cert/cafile_url_imports.ts",
    output: "cert/cafile_url_imports_unsafe_ssl.ts.out",
    http_server: true,
  });

  itest_flaky!(cafile_ts_fetch_unsafe_ssl {
    args:
      "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors cert/cafile_ts_fetch.ts",
    output: "cert/cafile_ts_fetch_unsafe_ssl.ts.out",
    http_server: true,
  });

  itest!(deno_land_unsafe_ssl {
    args:
      "run --quiet --reload --allow-net --cert=tls/RootCA.pem --unsafely-ignore-certificate-errors=localhost cert/deno_land_unsafe_ssl.ts",
    output: "cert/deno_land_unsafe_ssl.ts.out",
    http_server: true,
  });

  itest!(ip_address_unsafe_ssl {
    args:
      "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=1.1.1.1 cert/ip_address_unsafe_ssl.ts",
    output: "cert/ip_address_unsafe_ssl.ts.out",
  });

  itest!(localhost_unsafe_ssl {
    args:
      "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=deno.land cert/cafile_url_imports.ts",
    output: "cert/localhost_unsafe_ssl.ts.out",
    http_server: true,
    exit_code: 1,
  });

  #[flaky_test::flaky_test]
  fn cafile_env_fetch() {
    use deno_core::url::Url;
    let _g = util::http_server();
    let deno_dir = TempDir::new();
    let module_url =
      Url::parse("https://localhost:5545/cert/cafile_url_imports.ts").unwrap();
    let cafile = util::testdata_path().join("tls/RootCA.pem");
    let output = Command::new(util::deno_exe_path())
      .env("DENO_DIR", deno_dir.path())
      .env("DENO_CERT", cafile)
      .current_dir(util::testdata_path())
      .arg("cache")
      .arg(module_url.to_string())
      .output()
      .expect("Failed to spawn script");
    assert!(output.status.success());
  }

  #[flaky_test::flaky_test]
  fn cafile_fetch() {
    use deno_core::url::Url;
    let _g = util::http_server();
    let deno_dir = TempDir::new();
    let module_url =
      Url::parse("http://localhost:4545/cert/cafile_url_imports.ts").unwrap();
    let cafile = util::testdata_path().join("tls/RootCA.pem");
    let output = Command::new(util::deno_exe_path())
      .env("DENO_DIR", deno_dir.path())
      .current_dir(util::testdata_path())
      .arg("cache")
      .arg("--cert")
      .arg(cafile)
      .arg(module_url.to_string())
      .output()
      .expect("Failed to spawn script");
    assert!(output.status.success());
    let out = std::str::from_utf8(&output.stdout).unwrap();
    assert_eq!(out, "");
  }

  #[flaky_test::flaky_test]
  fn cafile_install_remote_module() {
    let _g = util::http_server();
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let deno_dir = TempDir::new();
    let cafile = util::testdata_path().join("tls/RootCA.pem");

    let install_output = Command::new(util::deno_exe_path())
      .env("DENO_DIR", deno_dir.path())
      .current_dir(util::testdata_path())
      .arg("install")
      .arg("--cert")
      .arg(cafile)
      .arg("--root")
      .arg(temp_dir.path())
      .arg("-n")
      .arg("echo_test")
      .arg("https://localhost:5545/echo.ts")
      .output()
      .expect("Failed to spawn script");
    println!("{}", std::str::from_utf8(&install_output.stdout).unwrap());
    eprintln!("{}", std::str::from_utf8(&install_output.stderr).unwrap());
    assert!(install_output.status.success());

    let mut echo_test_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      echo_test_path = echo_test_path.with_extension("cmd");
    }
    assert!(echo_test_path.exists());

    let output = Command::new(echo_test_path)
      .current_dir(temp_dir.path())
      .arg("foo")
      .env("PATH", util::target_dir())
      .output()
      .expect("failed to spawn script");
    let stdout = std::str::from_utf8(&output.stdout).unwrap().trim();
    assert!(stdout.ends_with("foo"));
  }

  #[flaky_test::flaky_test]
  fn cafile_bundle_remote_exports() {
    let _g = util::http_server();

    // First we have to generate a bundle of some remote module that has exports.
    let mod1 = "https://localhost:5545/subdir/mod1.ts";
    let cafile = util::testdata_path().join("tls/RootCA.pem");
    let t = TempDir::new();
    let bundle = t.path().join("mod1.bundle.js");
    let mut deno = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("bundle")
      .arg("--cert")
      .arg(cafile)
      .arg(mod1)
      .arg(&bundle)
      .spawn()
      .expect("failed to spawn script");
    let status = deno.wait().expect("failed to wait for the child process");
    assert!(status.success());
    assert!(bundle.is_file());

    // Now we try to use that bundle from another module.
    let test = t.path().join("test.js");
    std::fs::write(
      &test,
      "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
    )
    .expect("error writing file");

    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--check")
      .arg(&test)
      .output()
      .expect("failed to spawn script");
    // check the output of the test.ts program.
    assert!(std::str::from_utf8(&output.stdout)
      .unwrap()
      .trim()
      .ends_with("Hello"));
    assert_eq!(output.stderr, b"");
  }

  #[tokio::test]
  async fn listen_tls_alpn() {
    // TLS streams require the presence of an ambient local task set to gracefully
    // close dropped connections in the background.
    LocalSet::new()
      .run_until(async {
        let mut child = util::deno_cmd()
          .current_dir(util::testdata_path())
          .arg("run")
          .arg("--unstable")
          .arg("--quiet")
          .arg("--allow-net")
          .arg("--allow-read")
          .arg("./cert/listen_tls_alpn.ts")
          .arg("4504")
          .stdout(std::process::Stdio::piped())
          .spawn()
          .unwrap();
        let stdout = child.stdout.as_mut().unwrap();
        let mut msg = [0; 5];
        let read = stdout.read(&mut msg).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&msg, b"READY");

        let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
          "./testdata/tls/RootCA.crt"
        )));
        let certs = rustls_pemfile::certs(&mut reader).unwrap();
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_parsable_certificates(&certs);
        let mut cfg = rustls::ClientConfig::builder()
          .with_safe_defaults()
          .with_root_certificates(root_store)
          .with_no_client_auth();
        cfg.alpn_protocols.push(b"foobar".to_vec());
        let cfg = Arc::new(cfg);

        let hostname = rustls::ServerName::try_from("localhost").unwrap();

        let tcp_stream = tokio::net::TcpStream::connect("localhost:4504")
          .await
          .unwrap();
        let mut tls_stream =
          TlsStream::new_client_side(tcp_stream, cfg, hostname);

        tls_stream.handshake().await.unwrap();

        let (_, rustls_connection) = tls_stream.get_ref();
        let alpn = rustls_connection.alpn_protocol().unwrap();
        assert_eq!(alpn, b"foobar");

        let status = child.wait().unwrap();
        assert!(status.success());
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
          .current_dir(util::testdata_path())
          .arg("run")
          .arg("--unstable")
          .arg("--quiet")
          .arg("--allow-net")
          .arg("--allow-read")
          .arg("./cert/listen_tls_alpn_fail.ts")
          .arg("4505")
          .stdout(std::process::Stdio::piped())
          .spawn()
          .unwrap();
        let stdout = child.stdout.as_mut().unwrap();
        let mut msg = [0; 5];
        let read = stdout.read(&mut msg).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&msg, b"READY");

        let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
          "./testdata/tls/RootCA.crt"
        )));
        let certs = rustls_pemfile::certs(&mut reader).unwrap();
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_parsable_certificates(&certs);
        let mut cfg = rustls::ClientConfig::builder()
          .with_safe_defaults()
          .with_root_certificates(root_store)
          .with_no_client_auth();
        cfg.alpn_protocols.push(b"boofar".to_vec());
        let cfg = Arc::new(cfg);

        let hostname = rustls::ServerName::try_from("localhost").unwrap();

        let tcp_stream = tokio::net::TcpStream::connect("localhost:4505")
          .await
          .unwrap();
        let mut tls_stream =
          TlsStream::new_client_side(tcp_stream, cfg, hostname);

        tls_stream.handshake().await.unwrap_err();

        let (_, rustls_connection) = tls_stream.get_ref();
        assert!(rustls_connection.alpn_protocol().is_none());

        let status = child.wait().unwrap();
        assert!(status.success());
      })
      .await;
  }
}
