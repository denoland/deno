// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_net::ops_tls::TlsStream;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls_pemfile;
use std::fs;
use std::io::BufReader;
use std::io::Cursor;
use std::io::{Read, Write};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use test_util as util;
use tokio::task::LocalSet;

#[macro_export]
macro_rules! itest(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    (test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
}
);

#[macro_export]
macro_rules! itest_flaky(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[flaky_test::flaky_test]
  fn $name() {
    (test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
}
);

// These files have `_tests.rs` suffix to make it easier to tell which file is
// the test (ex. `lint_tests.rs`) and which is the implementation (ex. `lint.rs`)
// when both are open, especially for two tabs in VS Code

#[path = "bundle_tests.rs"]
mod bundle;
#[path = "cache_tests.rs"]
mod cache;
#[path = "compat_tests.rs"]
mod compat;
#[path = "compile_tests.rs"]
mod compile;
#[path = "coverage_tests.rs"]
mod coverage;
#[path = "doc_tests.rs"]
mod doc;
#[path = "eval_tests.rs"]
mod eval;
#[path = "fmt_tests.rs"]
mod fmt;
#[path = "info_tests.rs"]
mod info;
#[path = "inspector_tests.rs"]
mod inspector;
#[path = "install_tests.rs"]
mod install;
#[path = "lint_tests.rs"]
mod lint;
#[path = "lsp_tests.rs"]
mod lsp;
#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "test_tests.rs"]
mod test;
#[path = "upgrade_tests.rs"]
mod upgrade;
#[path = "vendor_tests.rs"]
mod vendor;
#[path = "watcher_tests.rs"]
mod watcher;
#[path = "worker_tests.rs"]
mod worker;

#[test]
fn help_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--help")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn version_short_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("-V")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn version_long_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--version")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

itest!(types {
  args: "types",
  output: "types.out",
});

#[test]
fn cache_test() {
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    url::Url::parse("http://localhost:4545/006_url_imports.ts").unwrap();
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg("-L")
    .arg("debug")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());

  let out = std::str::from_utf8(&output.stderr).unwrap();
  // Check if file and dependencies are written successfully
  assert!(out.contains("host.writeFile(\"deno://subdir/print_hello.js\")"));
  assert!(out.contains("host.writeFile(\"deno://subdir/mod2.js\")"));
  assert!(out.contains("host.writeFile(\"deno://006_url_imports.js\")"));

  let prg = util::deno_exe_path();
  let output = Command::new(&prg)
    .env("DENO_DIR", deno_dir.path())
    .env("HTTP_PROXY", "http://nil")
    .env("NO_COLOR", "1")
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");

  let str_output = std::str::from_utf8(&output.stdout).unwrap();

  let module_output_path = util::testdata_path().join("006_url_imports.ts.out");
  let mut module_output = String::new();
  let mut module_output_file = fs::File::open(module_output_path).unwrap();
  module_output_file
    .read_to_string(&mut module_output)
    .unwrap();

  assert_eq!(module_output, str_output);
}

#[test]
fn cache_invalidation_test() {
  let deno_dir = TempDir::new().expect("tempdir fail");
  let fixture_path = deno_dir.path().join("fixture.ts");
  {
    let mut file = std::fs::File::create(fixture_path.clone())
      .expect("could not create fixture");
    file
      .write_all(b"console.log(\"42\");")
      .expect("could not write fixture");
  }
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(fixture_path.to_str().unwrap())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  {
    let mut file = std::fs::File::create(fixture_path.clone())
      .expect("could not create fixture");
    file
      .write_all(b"console.log(\"43\");")
      .expect("could not write fixture");
  }
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(fixture_path.to_str().unwrap())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn cache_invalidation_test_no_check() {
  let deno_dir = TempDir::new().expect("tempdir fail");
  let fixture_path = deno_dir.path().join("fixture.ts");
  {
    let mut file = std::fs::File::create(fixture_path.clone())
      .expect("could not create fixture");
    file
      .write_all(b"console.log(\"42\");")
      .expect("could not write fixture");
  }
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(fixture_path.to_str().unwrap())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  {
    let mut file = std::fs::File::create(fixture_path.clone())
      .expect("could not create fixture");
    file
      .write_all(b"console.log(\"43\");")
      .expect("could not write fixture");
  }
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(fixture_path.to_str().unwrap())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn ts_dependency_recompilation() {
  let t = TempDir::new().expect("tempdir fail");
  let ats = t.path().join("a.ts");

  std::fs::write(
    &ats,
    "
    import { foo } from \"./b.ts\";

    function print(str: string): void {
        console.log(str);
    }

    print(foo);",
  )
  .unwrap();

  let bts = t.path().join("b.ts");
  std::fs::write(
    &bts,
    "
    export const foo = \"foo\";",
  )
  .unwrap();

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  assert!(stdout_output.ends_with("foo"));
  assert!(stderr_output.starts_with("Check"));

  // Overwrite contents of b.ts and run again
  std::fs::write(
    &bts,
    "
    export const foo = 5;",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  // error: TS2345 [ERROR]: Argument of type '5' is not assignable to parameter of type 'string'.
  assert!(stderr_output.contains("TS2345"));
  assert!(!output.status.success());
  assert!(stdout_output.is_empty());
}

#[test]
fn ts_no_recheck_on_redirect() {
  let deno_dir = util::new_deno_dir();
  let e = util::deno_exe_path();

  let redirect_ts = util::testdata_path().join("017_import_redirect.ts");
  assert!(redirect_ts.is_file());
  let mut cmd = Command::new(e.clone());
  cmd.env("DENO_DIR", deno_dir.path());
  let mut initial = cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(redirect_ts.clone())
    .spawn()
    .expect("failed to span script");
  let status_initial =
    initial.wait().expect("failed to wait for child process");
  assert!(status_initial.success());

  let mut cmd = Command::new(e);
  cmd.env("DENO_DIR", deno_dir.path());
  let output = cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(redirect_ts)
    .output()
    .expect("failed to spawn script");

  assert!(std::str::from_utf8(&output.stderr).unwrap().is_empty());
}

#[test]
fn ts_reload() {
  let hello_ts = util::testdata_path().join("002_hello.ts");
  assert!(hello_ts.is_file());

  let deno_dir = TempDir::new().expect("tempdir fail");
  let mut initial = util::deno_cmd_with_deno_dir(deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg(&hello_ts)
    .spawn()
    .expect("failed to spawn script");
  let status_initial =
    initial.wait().expect("failed to wait for child process");
  assert!(status_initial.success());

  let output = util::deno_cmd_with_deno_dir(deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg("--reload")
    .arg("-L")
    .arg("debug")
    .arg(&hello_ts)
    .output()
    .expect("failed to spawn script");

  // check the output of the the bundle program.
  let output_path = hello_ts.canonicalize().unwrap();
  assert!(
    dbg!(std::str::from_utf8(&output.stderr).unwrap().trim()).contains(
      &format!(
        "host.getSourceFile(\"{}\", Latest)",
        url::Url::from_file_path(&output_path).unwrap().as_str()
      )
    )
  );
}

#[test]
fn timeout_clear() {
  // https://github.com/denoland/deno/issues/7599

  use std::time::Duration;
  use std::time::Instant;

  let source_code = r#"
const handle = setTimeout(() => {
  console.log("timeout finish");
}, 10000);
clearTimeout(handle);
console.log("finish");
"#;

  let mut p = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let start = Instant::now();
  let status = p.wait().unwrap();
  let end = Instant::now();
  assert!(status.success());
  // check that program did not run for 10 seconds
  // for timeout to clear
  assert!(end - start < Duration::new(10, 0));
}

#[test]
fn compiler_api() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--unstable")
    .arg("--reload")
    .arg("--allow-read")
    .arg("compiler_api_test.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn broken_stdout() {
  let (reader, writer) = os_pipe::pipe().unwrap();
  // drop the reader to create a broken pipe
  drop(reader);

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("eval")
    .arg("console.log(3.14)")
    .stdout(writer)
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());
  let stderr = std::str::from_utf8(output.stderr.as_ref()).unwrap().trim();
  assert!(stderr.contains("Uncaught BrokenPipe"));
  assert!(!stderr.contains("panic"));
}

itest!(error_cause {
  args: "run error_cause.ts",
  output: "error_cause.ts.out",
  exit_code: 1,
});

itest!(error_cause_recursive {
  args: "run error_cause_recursive.ts",
  output: "error_cause_recursive.ts.out",
  exit_code: 1,
});

itest_flaky!(cafile_url_imports {
  args: "run --quiet --reload --cert tls/RootCA.pem cafile_url_imports.ts",
  output: "cafile_url_imports.ts.out",
  http_server: true,
});

itest_flaky!(cafile_ts_fetch {
  args:
    "run --quiet --reload --allow-net --cert tls/RootCA.pem cafile_ts_fetch.ts",
  output: "cafile_ts_fetch.ts.out",
  http_server: true,
});

itest_flaky!(cafile_eval {
  args: "eval --cert tls/RootCA.pem fetch('https://localhost:5545/cafile_ts_fetch.ts.out').then(r=>r.text()).then(t=>console.log(t.trimEnd()))",
  output: "cafile_ts_fetch.ts.out",
  http_server: true,
});

itest_flaky!(cafile_info {
  args:
    "info --quiet --cert tls/RootCA.pem https://localhost:5545/cafile_info.ts",
  output: "cafile_info.ts.out",
  http_server: true,
});

itest_flaky!(cafile_url_imports_unsafe_ssl {
  args: "run --quiet --reload --unsafely-ignore-certificate-errors=localhost cafile_url_imports.ts",
  output: "cafile_url_imports_unsafe_ssl.ts.out",
  http_server: true,
});

itest_flaky!(cafile_ts_fetch_unsafe_ssl {
  args:
    "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors cafile_ts_fetch.ts",
  output: "cafile_ts_fetch_unsafe_ssl.ts.out",
  http_server: true,
});

itest!(deno_land_unsafe_ssl {
  args:
    "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=deno.land deno_land_unsafe_ssl.ts",
  output: "deno_land_unsafe_ssl.ts.out",
});

itest!(localhost_unsafe_ssl {
  args:
    "run --quiet --reload --allow-net --unsafely-ignore-certificate-errors=deno.land cafile_url_imports.ts",
  output: "localhost_unsafe_ssl.ts.out",
  http_server: true,
  exit_code: 1,
});

#[flaky_test::flaky_test]
fn cafile_env_fetch() {
  use deno_core::url::Url;
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    Url::parse("https://localhost:5545/cafile_url_imports.ts").unwrap();
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
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    Url::parse("http://localhost:4545/cafile_url_imports.ts").unwrap();
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
  let temp_dir = TempDir::new().expect("tempdir fail");
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let deno_dir = TempDir::new().expect("tempdir fail");
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
  let t = TempDir::new().expect("tempdir fail");
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

#[test]
fn websocket() {
  let _g = util::http_server();

  let script = util::testdata_path().join("websocket_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let status = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .spawn()
    .unwrap()
    .wait()
    .unwrap();

  assert!(status.success());
}

#[test]
fn websocketstream() {
  let _g = util::http_server();

  let script = util::testdata_path().join("websocketstream_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let status = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .spawn()
    .unwrap()
    .wait()
    .unwrap();

  assert!(status.success());
}

#[test]
fn websocket_server_multi_field_connection_header() {
  let script = util::testdata_path()
    .join("websocket_server_multi_field_connection_header_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let req = http::request::Builder::new()
    .header(http::header::CONNECTION, "keep-alive, Upgrade")
    .uri("ws://localhost:4319")
    .body(())
    .unwrap();
  assert!(
    deno_runtime::deno_websocket::tokio_tungstenite::tungstenite::connect(req)
      .is_ok()
  );
  assert!(child.wait().unwrap().success());
}

#[test]
fn websocket_server_idletimeout() {
  let script = util::testdata_path().join("websocket_server_idletimeout.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let req = http::request::Builder::new()
    .uri("ws://localhost:4509")
    .body(())
    .unwrap();
  let (_ws, _request) =
    deno_runtime::deno_websocket::tokio_tungstenite::tungstenite::connect(req)
      .unwrap();

  assert!(child.wait().unwrap().success());
}

#[cfg(not(windows))]
#[test]
fn set_raw_should_not_panic_on_no_tty() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("--unstable")
    .arg("Deno.setRaw(Deno.stdin.rid, true)")
    // stdin set to piped so it certainly does not refer to TTY
    .stdin(std::process::Stdio::piped())
    // stderr is piped so we can capture output.
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert!(stderr.contains("BadResource"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_resolve_dns() {
  use std::collections::BTreeMap;
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddr;
  use std::str::FromStr;
  use std::sync::Arc;
  use std::sync::RwLock;
  use std::time::Duration;
  use tokio::net::TcpListener;
  use tokio::net::UdpSocket;
  use tokio::sync::oneshot;
  use trust_dns_client::rr::LowerName;
  use trust_dns_client::rr::RecordType;
  use trust_dns_client::rr::RrKey;
  use trust_dns_server::authority::Catalog;
  use trust_dns_server::authority::ZoneType;
  use trust_dns_server::proto::rr::rdata::mx::MX;
  use trust_dns_server::proto::rr::rdata::soa::SOA;
  use trust_dns_server::proto::rr::rdata::srv::SRV;
  use trust_dns_server::proto::rr::rdata::txt::TXT;
  use trust_dns_server::proto::rr::record_data::RData;
  use trust_dns_server::proto::rr::resource::Record;
  use trust_dns_server::proto::rr::Name;
  use trust_dns_server::proto::rr::RecordSet;
  use trust_dns_server::store::in_memory::InMemoryAuthority;
  use trust_dns_server::ServerFuture;

  const DNS_PORT: u16 = 4553;

  // Setup DNS server for testing
  async fn run_dns_server(tx: oneshot::Sender<()>) {
    let catalog = {
      let records = {
        let mut map = BTreeMap::new();
        let lookup_name = "www.example.com".parse::<Name>().unwrap();
        let lookup_name_lower = LowerName::new(&lookup_name);

        // Inserts SOA record
        let soa = SOA::new(
          Name::from_str("net").unwrap(),
          Name::from_str("example").unwrap(),
          0,
          i32::MAX,
          i32::MAX,
          i32::MAX,
          0,
        );
        let rdata = RData::SOA(soa);
        let record = Record::from_rdata(Name::new(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map
          .insert(RrKey::new(Name::root().into(), RecordType::SOA), record_set);

        // Inserts A record
        let rdata = RData::A(Ipv4Addr::new(1, 2, 3, 4));
        let record = Record::from_rdata(lookup_name.clone(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::A),
          record_set,
        );

        // Inserts AAAA record
        let rdata = RData::AAAA(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        let record = Record::from_rdata(lookup_name.clone(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::AAAA),
          record_set,
        );

        // Inserts ANAME record
        let rdata = RData::ANAME(Name::from_str("aname.com").unwrap());
        let record = Record::from_rdata(lookup_name.clone(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::ANAME),
          record_set,
        );

        // Inserts CNAME record
        let rdata = RData::CNAME(Name::from_str("cname.com").unwrap());
        let record =
          Record::from_rdata(Name::from_str("foo").unwrap(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::CNAME),
          record_set,
        );

        // Inserts MX record
        let rdata = RData::MX(MX::new(0, Name::from_str("mx.com").unwrap()));
        let record = Record::from_rdata(lookup_name.clone(), u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::MX),
          record_set,
        );

        // Inserts PTR record
        let rdata = RData::PTR(Name::from_str("ptr.com").unwrap());
        let record = Record::from_rdata(
          Name::from_str("5.6.7.8").unwrap(),
          u32::MAX,
          rdata,
        );
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new("5.6.7.8".parse().unwrap(), RecordType::PTR),
          record_set,
        );

        // Inserts SRV record
        let rdata = RData::SRV(SRV::new(
          0,
          100,
          1234,
          Name::from_str("srv.com").unwrap(),
        ));
        let record = Record::from_rdata(
          Name::from_str("_Service._TCP.example.com").unwrap(),
          u32::MAX,
          rdata,
        );
        let record_set = RecordSet::from(record);
        map.insert(
          RrKey::new(lookup_name_lower.clone(), RecordType::SRV),
          record_set,
        );

        // Inserts TXT record
        let rdata =
          RData::TXT(TXT::new(vec!["foo".to_string(), "bar".to_string()]));
        let record = Record::from_rdata(lookup_name, u32::MAX, rdata);
        let record_set = RecordSet::from(record);
        map.insert(RrKey::new(lookup_name_lower, RecordType::TXT), record_set);

        map
      };

      let authority = Box::new(Arc::new(RwLock::new(
        InMemoryAuthority::new(
          Name::from_str("com").unwrap(),
          records,
          ZoneType::Primary,
          false,
        )
        .unwrap(),
      )));
      let mut c = Catalog::new();
      c.upsert(Name::root().into(), authority);
      c
    };

    let mut server_fut = ServerFuture::new(catalog);
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], DNS_PORT));
    let tcp_listener = TcpListener::bind(socket_addr).await.unwrap();
    let udp_socket = UdpSocket::bind(socket_addr).await.unwrap();
    server_fut.register_socket(udp_socket);
    server_fut.register_listener(tcp_listener, Duration::from_secs(2));

    // Notifies that the DNS server is ready
    tx.send(()).unwrap();

    server_fut.block_until_done().await.unwrap();
  }

  let (ready_tx, ready_rx) = oneshot::channel();
  let dns_server_fut = run_dns_server(ready_tx);
  let handle = tokio::spawn(dns_server_fut);

  // Waits for the DNS server to be ready
  ready_rx.await.unwrap();

  // Pass: `--allow-net`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--allow-net")
      .arg("resolve_dns.ts")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected =
      std::fs::read_to_string(util::testdata_path().join("resolve_dns.ts.out"))
        .unwrap();
    assert_eq!(expected, out);
  }

  // Pass: `--allow-net=127.0.0.1:4553`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--allow-net=127.0.0.1:4553")
      .arg("resolve_dns.ts")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected =
      std::fs::read_to_string(util::testdata_path().join("resolve_dns.ts.out"))
        .unwrap();
    assert_eq!(expected, out);
  }

  // Permission error: `--allow-net=deno.land`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--allow-net=deno.land")
      .arg("resolve_dns.ts")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) PermissionDenied: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  // Permission error: no permission specified
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("resolve_dns.ts")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) PermissionDenied: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  handle.abort();
}

#[test]
fn typecheck_declarations_ns() {
  let output = util::deno_cmd()
    .arg("test")
    .arg("--doc")
    .arg(util::root_path().join("cli/dts/lib.deno.ns.d.ts"))
    .output()
    .unwrap();
  println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
  println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
  assert!(output.status.success());
}

#[test]
fn typecheck_declarations_unstable() {
  let output = util::deno_cmd()
    .arg("test")
    .arg("--doc")
    .arg("--unstable")
    .arg(util::root_path().join("cli/dts/lib.deno.unstable.d.ts"))
    .output()
    .unwrap();
  println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
  println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
  assert!(output.status.success());
}

#[test]
fn typecheck_core() {
  let deno_dir = TempDir::new().expect("tempdir fail");
  let test_file = deno_dir.path().join("test_deno_core_types.ts");
  std::fs::write(
    &test_file,
    format!(
      "import \"{}\";",
      deno_core::resolve_path(
        util::root_path()
          .join("core/lib.deno_core.d.ts")
          .to_str()
          .unwrap()
      )
      .unwrap()
    ),
  )
  .unwrap();
  let output = util::deno_cmd_with_deno_dir(deno_dir.path())
    .arg("run")
    .arg(test_file.to_str().unwrap())
    .output()
    .unwrap();
  println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
  println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
  assert!(output.status.success());
}

#[test]
fn js_unit_tests_lint() {
  let status = util::deno_cmd()
    .arg("lint")
    .arg("--unstable")
    .arg(util::tests_path().join("unit"))
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
    .arg("--no-prompt")
    .arg("-A")
    .arg(util::tests_path().join("unit"))
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}

#[test]
fn basic_auth_tokens() {
  let _g = util::http_server();

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://127.0.0.1:4554/001_hello.js")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout_str.is_empty());

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{}", stderr_str);

  assert!(stderr_str
    .contains("Module not found \"http://127.0.0.1:4554/001_hello.js\"."));

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://127.0.0.1:4554/001_hello.js")
    .env("DENO_AUTH_TOKENS", "testuser123:testpassabc@127.0.0.1:4554")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{}", stderr_str);

  assert!(output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!(util::strip_ansi_codes(stdout_str), "Hello World");
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
        .arg("./listen_tls_alpn.ts")
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
        .arg("./listen_tls_alpn_fail.ts")
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
        TlsStream::new_client_side(tcp_stream, cfg, hostname);

      tls_stream.handshake().await.unwrap_err();

      let (_, rustls_connection) = tls_stream.get_ref();
      assert!(rustls_connection.alpn_protocol().is_none());

      let status = child.wait().unwrap();
      assert!(status.success());
    })
    .await;
}

#[tokio::test]
async fn http2_request_url() {
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
        .arg("./http2_request_url.ts")
        .arg("4506")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
      let stdout = child.stdout.as_mut().unwrap();
      let mut buffer = [0; 5];
      let read = stdout.read(&mut buffer).unwrap();
      assert_eq!(read, 5);
      let msg = std::str::from_utf8(&buffer).unwrap();
      assert_eq!(msg, "READY");

      let cert = reqwest::Certificate::from_pem(include_bytes!(
        "../testdata/tls/RootCA.crt"
      ))
      .unwrap();

      let client = reqwest::Client::builder()
        .add_root_certificate(cert)
        .http2_prior_knowledge()
        .build()
        .unwrap();

      let res = client.get("http://127.0.0.1:4506").send().await.unwrap();
      assert_eq!(200, res.status());

      let body = res.text().await.unwrap();
      assert_eq!(body, "http://127.0.0.1:4506/");

      child.kill().unwrap();
      child.wait().unwrap();
    })
    .await;
}
