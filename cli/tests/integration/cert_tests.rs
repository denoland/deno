// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_net::ops_tls::TlsStream;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls_pemfile;
use lsp_types::Url;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::sync::Arc;
use test_util as util;
use util::testdata_path;
use util::TestContext;

itest_flaky!(cafile_url_imports {
  args: "run --quiet --reload --cert tls/RootCA.pem cert/cafile_url_imports.ts",
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

// TODO(bartlomieju): reenable, this test was flaky on macOS CI during 1.30.3 release
// itest!(deno_land_unsafe_ssl {
//   args:
//     "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=deno.land cert/deno_land_unsafe_ssl.ts",
//   output: "cert/deno_land_unsafe_ssl.ts.out",
// });

itest!(ip_address_unsafe_ssl {
  args:
    "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=1.1.1.1 cert/ip_address_unsafe_ssl.ts",
  output: "cert/ip_address_unsafe_ssl.ts.out",
});

itest!(localhost_unsafe_ssl {
  args: "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=deno.land cert/cafile_url_imports.ts",
  output: "cert/localhost_unsafe_ssl.ts.out",
  http_server: true,
  exit_code: 1,
});

#[flaky_test::flaky_test]
fn cafile_env_fetch() {
  let module_url =
    Url::parse("https://localhost:5545/cert/cafile_url_imports.ts").unwrap();
  let context = TestContext::with_http_server();
  let cafile = testdata_path().join("tls/RootCA.pem");

  context
    .new_command()
    .args(format!("cache {module_url}"))
    .env("DENO_CERT", cafile)
    .run()
    .assert_exit_code(0)
    .skip_output_check();
}

#[flaky_test::flaky_test]
fn cafile_fetch() {
  let module_url =
    Url::parse("http://localhost:4545/cert/cafile_url_imports.ts").unwrap();
  let context = TestContext::with_http_server();
  let cafile = testdata_path().join("tls/RootCA.pem");
  context
    .new_command()
    .args(format!("cache --quiet --cert {} {}", cafile, module_url,))
    .run()
    .assert_exit_code(0)
    .assert_matches_text("");
}

#[test]
fn cafile_compile() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir().path();
  let output_exe = if cfg!(windows) {
    temp_dir.join("cert.exe")
  } else {
    temp_dir.join("cert")
  };
  let output = context.new_command()
    .args(format!("compile --quiet --cert ./tls/RootCA.pem --allow-net --output {} ./cert/cafile_ts_fetch.ts", output_exe))
    .run();
  output.skip_output_check();

  context
    .new_command()
    .name(output_exe)
    .run()
    .assert_matches_text("[WILDCARD]\nHello\n");
}

#[flaky_test::flaky_test]
fn cafile_install_remote_module() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir();
  let bin_dir = temp_dir.path().join("bin");
  bin_dir.create_dir_all();
  let cafile = util::testdata_path().join("tls/RootCA.pem");

  let install_output = context
    .new_command()
    .args_vec([
      "install",
      "--cert",
      &cafile.to_string_lossy(),
      "--root",
      &temp_dir.path().to_string_lossy(),
      "-n",
      "echo_test",
      "https://localhost:5545/echo.ts",
    ])
    .split_output()
    .run();
  println!("{}", install_output.stdout());
  eprintln!("{}", install_output.stderr());
  install_output.assert_exit_code(0);

  let mut echo_test_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    echo_test_path = echo_test_path.with_extension("cmd");
  }
  assert!(echo_test_path.exists());

  let output = context
    .new_command()
    .name(echo_test_path)
    .args("foo")
    .env("PATH", util::target_dir())
    .run();
  output.assert_matches_text("[WILDCARD]foo");
}

#[flaky_test::flaky_test]
fn cafile_bundle_remote_exports() {
  let context = TestContext::with_http_server();

  // First we have to generate a bundle of some remote module that has exports.
  let mod1 = "https://localhost:5545/subdir/mod1.ts";
  let cafile = util::testdata_path().join("tls/RootCA.pem");
  let t = context.temp_dir();
  let bundle = t.path().join("mod1.bundle.js");
  context
    .new_command()
    .args_vec([
      "bundle",
      "--cert",
      &cafile.to_string_lossy(),
      mod1,
      &bundle.to_string_lossy(),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  test.write(
    "import { printHello3 } from \"./mod1.bundle.js\";
printHello3();",
  );

  context
    .new_command()
    .args_vec(["run", "--quiet", "--check", &test.to_string_lossy()])
    .run()
    .assert_matches_text("[WILDCARD]Hello\n")
    .assert_exit_code(0);
}

#[tokio::test]
async fn listen_tls_alpn() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("./cert/listen_tls_alpn.ts")
    .arg("4504")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut msg = [0; 5];
  let read = stdout.read(&mut msg).unwrap();
  assert_eq!(read, 5);
  assert_eq!(&msg, b"READY");

  let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
    "../testdata/tls/RootCA.crt"
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
    TlsStream::new_client_side(tcp_stream, cfg, hostname, None);

  let handshake = tls_stream.handshake().await.unwrap();

  assert_eq!(handshake.alpn, Some(b"foobar".to_vec()));

  let status = child.wait().unwrap();
  assert!(status.success());
}

#[tokio::test]
async fn listen_tls_alpn_fail() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("./cert/listen_tls_alpn_fail.ts")
    .arg("4505")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut msg = [0; 5];
  let read = stdout.read(&mut msg).unwrap();
  assert_eq!(read, 5);
  assert_eq!(&msg, b"READY");

  let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
    "../testdata/tls/RootCA.crt"
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
    TlsStream::new_client_side(tcp_stream, cfg, hostname, None);

  tls_stream.handshake().await.unwrap_err();

  let status = child.wait().unwrap();
  assert!(status.success());
}
