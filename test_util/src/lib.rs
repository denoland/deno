// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Usage: provide a port as argument to run hyper_hello benchmark server
// otherwise this starts multiple servers on many ports for test endpoints.

#[macro_use]
extern crate lazy_static;

use futures::future::{self, FutureExt};
use os_pipe::pipe;
#[cfg(unix)]
pub use pty;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::io::Write;
use std::mem::replace;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tempfile::TempDir;
use warp::http::HeaderValue;
use warp::http::Response;
use warp::http::StatusCode;
use warp::http::Uri;
use warp::hyper::Body;
use warp::reply::with_header;
use warp::reply::Reply;
use warp::Filter;

const PORT: u16 = 4545;
const REDIRECT_PORT: u16 = 4546;
const ANOTHER_REDIRECT_PORT: u16 = 4547;
const DOUBLE_REDIRECTS_PORT: u16 = 4548;
const INF_REDIRECTS_PORT: u16 = 4549;
const REDIRECT_ABSOLUTE_PORT: u16 = 4550;
const HTTPS_PORT: u16 = 5545;
const WS_PORT: u16 = 4242;
const WSS_PORT: u16 = 4243;

pub const PERMISSION_VARIANTS: [&str; 5] =
  ["read", "write", "env", "net", "run"];
pub const PERMISSION_DENIED_PATTERN: &str = "PermissionDenied";

lazy_static! {
  // STRIP_ANSI_RE and strip_ansi_codes are lifted from the "console" crate.
  // Copyright 2017 Armin Ronacher <armin.ronacher@active-4.com>. MIT License.
  static ref STRIP_ANSI_RE: Regex = Regex::new(
          r"[\x1b\x9b][\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
  ).unwrap();

  static ref GUARD: Mutex<HttpServerCount> = Mutex::new(HttpServerCount::default());
}

pub fn root_path() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

pub fn prebuilt_path() -> PathBuf {
  third_party_path().join("prebuilt")
}

pub fn tests_path() -> PathBuf {
  root_path().join("cli").join("tests")
}

pub fn third_party_path() -> PathBuf {
  root_path().join("third_party")
}

pub fn target_dir() -> PathBuf {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  println!("target_dir {}", target_dir.display());
  target_dir.into()
}

pub fn deno_exe_path() -> PathBuf {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

pub fn prebuilt_tool_path(tool: &str) -> PathBuf {
  let mut exe = tool.to_string();
  exe.push_str(if cfg!(windows) { ".exe" } else { "" });
  prebuilt_path().join(platform_dir_name()).join(exe)
}

fn platform_dir_name() -> &'static str {
  if cfg!(target_os = "linux") {
    "linux64"
  } else if cfg!(target_os = "macos") {
    "mac"
  } else if cfg!(target_os = "windows") {
    "win"
  } else {
    unreachable!()
  }
}

pub fn test_server_path() -> PathBuf {
  let mut p = target_dir().join("test_server");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

/// Benchmark server that just serves "hello world" responses.
async fn hyper_hello(port: u16) {
  println!("hyper hello");
  let route = warp::any().map(|| "Hello World!");
  warp::serve(route).bind(([127, 0, 0, 1], port)).await;
}

#[tokio::main]
pub async fn run_all_servers() {
  if let Some(port) = env::args().nth(1) {
    return hyper_hello(port.parse::<u16>().unwrap()).await;
  }

  let routes = warp::path::full().map(|path: warp::path::FullPath| {
    let p = path.as_str();
    assert_eq!(&p[0..1], "/");
    let url = format!("http://localhost:{}{}", PORT, p);
    let u = url.parse::<Uri>().unwrap();
    warp::redirect(u)
  });
  let redirect_server_fut =
    warp::serve(routes).bind(([127, 0, 0, 1], REDIRECT_PORT));

  let websocket_route = warp::ws().map(|ws: warp::ws::Ws| {
    ws.on_upgrade(|websocket| {
      use futures::stream::StreamExt;
      let (tx, rx) = websocket.split();
      rx.forward(tx).map(|result| {
        if let Err(e) = result {
          println!("websocket server error: {:?}", e);
        }
      })
    })
  });
  let ws_server_fut =
    warp::serve(websocket_route).bind(([127, 0, 0, 1], WS_PORT));
  let wss_server_fut = warp::serve(websocket_route)
    .tls()
    .cert_path("std/http/testdata/tls/localhost.crt")
    .key_path("std/http/testdata/tls/localhost.key")
    .bind(([127, 0, 0, 1], WSS_PORT));

  let routes = warp::path::full().map(|path: warp::path::FullPath| {
    let p = path.as_str();
    assert_eq!(&p[0..1], "/");
    let url = format!("http://localhost:{}/cli/tests/subdir{}", PORT, p);
    let u = url.parse::<Uri>().unwrap();
    warp::redirect(u)
  });
  let another_redirect_server_fut =
    warp::serve(routes).bind(([127, 0, 0, 1], ANOTHER_REDIRECT_PORT));

  let routes = warp::path::full().map(|path: warp::path::FullPath| {
    let p = path.as_str();
    assert_eq!(&p[0..1], "/");
    let url = format!("http://localhost:{}{}", REDIRECT_PORT, p);
    let u = url.parse::<Uri>().unwrap();
    warp::redirect(u)
  });
  let double_redirect_server_fut =
    warp::serve(routes).bind(([127, 0, 0, 1], DOUBLE_REDIRECTS_PORT));

  let routes = warp::path::full().map(|path: warp::path::FullPath| {
    let p = path.as_str();
    assert_eq!(&p[0..1], "/");
    let url = format!("http://localhost:{}{}", INF_REDIRECTS_PORT, p);
    let u = url.parse::<Uri>().unwrap();
    warp::redirect(u)
  });
  let inf_redirect_server_fut =
    warp::serve(routes).bind(([127, 0, 0, 1], INF_REDIRECTS_PORT));

  // redirect server that redirect to absolute paths under same host
  // redirects /REDIRECT/file_name to /file_name
  let routes = warp::path("REDIRECT")
    .and(warp::path::peek())
    .map(|path: warp::path::Peek| {
      let p = path.as_str();
      let url = format!("/{}", p);
      let u = url.parse::<Uri>().unwrap();
      warp::redirect(u)
    })
    .or(
      warp::path!("a" / "b" / "c")
        .and(warp::header::<String>("x-location"))
        .map(|token: String| {
          let uri: Uri = token.parse().unwrap();
          warp::redirect(uri)
        }),
    )
    .or(
      warp::any()
        .and(warp::path::peek())
        .and(warp::fs::dir(root_path()))
        .map(custom_headers),
    );
  let absolute_redirect_server_fut =
    warp::serve(routes).bind(([127, 0, 0, 1], REDIRECT_ABSOLUTE_PORT));

  let echo_server = warp::path("echo_server")
    .and(warp::post())
    .and(warp::body::bytes())
    .and(warp::header::optional::<String>("x-status"))
    .and(warp::header::optional::<String>("content-type"))
    .and(warp::header::optional::<String>("user-agent"))
    .map(
      |bytes: bytes::Bytes,
       status: Option<String>,
       content_type: Option<String>,
       user_agent: Option<String>|
       -> Box<dyn Reply> {
        let mut res = Response::new(Body::from(bytes));
        if let Some(v) = status {
          *res.status_mut() = StatusCode::from_bytes(v.as_bytes()).unwrap();
        }
        let h = res.headers_mut();
        if let Some(v) = content_type {
          h.insert("content-type", HeaderValue::from_str(&v).unwrap());
        }
        if let Some(v) = user_agent {
          h.insert("user-agent", HeaderValue::from_str(&v).unwrap());
        }
        Box::new(res)
      },
    );
  let echo_multipart_file = warp::path("echo_multipart_file")
    .and(warp::post())
    .and(warp::body::bytes())
    .map(|bytes: bytes::Bytes| -> Box<dyn Reply> {
      let start = b"--boundary\t \r\n\
                    Content-Disposition: form-data; name=\"field_1\"\r\n\
                    \r\n\
                    value_1 \r\n\
                    \r\n--boundary\r\n\
                    Content-Disposition: form-data; name=\"file\"; \
                    filename=\"file.bin\"\r\n\
                    Content-Type: application/octet-stream\r\n\
                    \r\n";
      let end = b"\r\n--boundary--\r\n";
      let b = [start as &[u8], &bytes, end].concat();

      let mut res = Response::new(Body::from(b));
      let h = res.headers_mut();
      h.insert(
        "content-type",
        HeaderValue::from_static("multipart/form-data;boundary=boundary"),
      );
      Box::new(res)
    });
  let multipart_form_data =
    warp::path("multipart_form_data.txt").map(|| -> Box<dyn Reply> {
      let b = "Preamble\r\n\
               --boundary\t \r\n\
               Content-Disposition: form-data; name=\"field_1\"\r\n\
               \r\n\
               value_1 \r\n\
               \r\n--boundary\r\n\
               Content-Disposition: form-data; name=\"field_2\";\
               filename=\"file.js\"\r\n\
               Content-Type: text/javascript\r\n\
               \r\n\
               console.log(\"Hi\")\
               \r\n--boundary--\r\n\
               Epilogue";
      let mut res = Response::new(Body::from(b));
      res.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("multipart/form-data;boundary=boundary"),
      );
      Box::new(res)
    });
  let bad_redirect = warp::path("bad_redirect").map(|| -> Box<dyn Reply> {
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::FOUND;
    Box::new(res)
  });

  let etag_script = warp::path!("etag_script.ts")
    .and(warp::header::optional::<String>("if-none-match"))
    .map(|if_none_match| -> Box<dyn Reply> {
      if if_none_match == Some("33a64df551425fcc55e".to_string()) {
        let r =
          warp::reply::with_status(warp::reply(), StatusCode::NOT_MODIFIED);
        let r = with_header(r, "Content-type", "application/typescript");
        let r = with_header(r, "ETag", "33a64df551425fcc55e");
        Box::new(r)
      } else {
        let mut res = Response::new(Body::from("console.log('etag')"));
        let h = res.headers_mut();
        h.insert(
          "Content-type",
          HeaderValue::from_static("application/typescript"),
        );
        h.insert("ETag", HeaderValue::from_static("33a64df551425fcc55e"));
        Box::new(res)
      }
    });
  let xtypescripttypes = warp::path!("xTypeScriptTypes.js")
    .map(|| {
      let mut res = Response::new(Body::from("export const foo = 'foo';"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      h.insert(
        "X-TypeScript-Types",
        HeaderValue::from_static("./xTypeScriptTypes.d.ts"),
      );
      res
    })
    .or(warp::path!("xTypeScriptTypes.d.ts").map(|| {
      let mut res = Response::new(Body::from("export const foo: 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("type_directives_redirect.js").map(|| {
      let mut res = Response::new(Body::from("export const foo = 'foo';"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      h.insert(
        "X-TypeScript-Types",
        HeaderValue::from_static(
          "http://localhost:4547/xTypeScriptTypesRedirect.d.ts",
        ),
      );
      res
    }))
    .or(warp::path!("type_headers_deno_types.foo.js").map(|| {
      let mut res = Response::new(Body::from("export function foo(text) { console.log(text); }"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      h.insert(
        "X-TypeScript-Types",
        HeaderValue::from_static(
          "http://localhost:4545/type_headers_deno_types.d.ts",
        ),
      );
      res
    }))
    .or(warp::path!("type_headers_deno_types.d.ts").map(|| {
      let mut res = Response::new(Body::from("export function foo(text: number): void;"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("type_headers_deno_types.foo.d.ts").map(|| {
      let mut res = Response::new(Body::from("export function foo(text: string): void;"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("cli"/"tests"/"subdir"/"xTypeScriptTypesRedirect.d.ts").map(|| {
      let mut res = Response::new(Body::from(
        "import './xTypeScriptTypesRedirected.d.ts';",
      ));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("cli"/"tests"/"subdir"/"xTypeScriptTypesRedirected.d.ts").map(|| {
      let mut res = Response::new(Body::from("export const foo: 'foo';"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("referenceTypes.js").map(|| {
      let mut res = Response::new(Body::from("/// <reference types=\"./xTypeScriptTypes.d.ts\" />\r\nexport const foo = \"foo\";\r\n"));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      res
    }))
    .or(warp::path!("cli"/"tests"/"subdir"/"file_with_:_in_name.ts").map(|| {
      let mut res = Response::new(Body::from(
        "console.log('Hello from file_with_:_in_name.ts');",
      ));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res
    }))
    .or(warp::path!("cli"/"tests"/"subdir"/"no_js_ext@1.0.0").map(|| {
      let mut res = Response::new(Body::from(
        r#"import { printHello } from "./mod2.ts";
        printHello();
        "#,
      ));
      let h = res.headers_mut();
      h.insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      res
    }));

  let content_type_handler = warp::any()
    .and(warp::path::peek())
    .and(warp::fs::dir(root_path()))
    .map(custom_headers)
    .or(etag_script)
    .or(xtypescripttypes)
    .or(echo_server)
    .or(echo_multipart_file)
    .or(multipart_form_data)
    .or(bad_redirect);

  let http_fut =
    warp::serve(content_type_handler.clone()).bind(([127, 0, 0, 1], PORT));

  let https_fut = warp::serve(content_type_handler.clone())
    .tls()
    .cert_path("std/http/testdata/tls/localhost.crt")
    .key_path("std/http/testdata/tls/localhost.key")
    .bind(([127, 0, 0, 1], HTTPS_PORT));

  let mut server_fut = async {
    futures::join!(
      http_fut,
      https_fut,
      redirect_server_fut,
      ws_server_fut,
      wss_server_fut,
      another_redirect_server_fut,
      inf_redirect_server_fut,
      double_redirect_server_fut,
      absolute_redirect_server_fut,
    )
  }
  .boxed();

  let mut did_print_ready = false;
  future::poll_fn(move |cx| {
    let poll_result = server_fut.poll_unpin(cx);
    if !replace(&mut did_print_ready, true) {
      println!("ready");
    }
    poll_result
  })
  .await;
}

fn custom_headers(path: warp::path::Peek, f: warp::fs::File) -> Box<dyn Reply> {
  let p = path.as_str();

  if p.ends_with("cli/tests/x_deno_warning.js") {
    let f = with_header(f, "Content-Type", "application/javascript");
    let f = with_header(f, "X-Deno-Warning", "foobar");
    return Box::new(f);
  }
  if p.ends_with("cli/tests/053_import_compression/brotli") {
    let f = with_header(f, "Content-Encoding", "br");
    let f = with_header(f, "Content-Type", "application/javascript");
    let f = with_header(f, "Content-Length", "26");
    return Box::new(f);
  }
  if p.ends_with("cli/tests/053_import_compression/gziped") {
    let f = with_header(f, "Content-Encoding", "gzip");
    let f = with_header(f, "Content-Type", "application/javascript");
    let f = with_header(f, "Content-Length", "39");
    return Box::new(f);
  }
  if p.contains("cli/tests/encoding/") {
    let charset = p
      .split_terminator('/')
      .last()
      .unwrap()
      .trim_end_matches(".ts");
    let f = with_header(
      f,
      "Content-Type",
      &format!("application/typescript;charset={}", charset)[..],
    );
    return Box::new(f);
  }

  let content_type = if p.contains(".t1.") {
    Some("text/typescript")
  } else if p.contains(".t2.") {
    Some("video/vnd.dlna.mpeg-tts")
  } else if p.contains(".t3.") {
    Some("video/mp2t")
  } else if p.contains(".t4.") {
    Some("application/x-typescript")
  } else if p.contains(".j1.") {
    Some("text/javascript")
  } else if p.contains(".j2.") {
    Some("application/ecmascript")
  } else if p.contains(".j3.") {
    Some("text/ecmascript")
  } else if p.contains(".j4.") {
    Some("application/x-javascript")
  } else if p.contains("form_urlencoded") {
    Some("application/x-www-form-urlencoded")
  } else if p.contains("unknown_ext") || p.contains("no_ext") {
    Some("text/typescript")
  } else if p.contains("mismatch_ext") || p.contains("no_js_ext") {
    Some("text/javascript")
  } else if p.ends_with(".ts") || p.ends_with(".tsx") {
    Some("application/typescript")
  } else if p.ends_with(".js") || p.ends_with(".jsx") {
    Some("application/javascript")
  } else if p.ends_with(".json") {
    Some("application/json")
  } else {
    None
  };

  if let Some(t) = content_type {
    Box::new(with_header(f, "Content-Type", t))
  } else {
    Box::new(f)
  }
}

#[derive(Default)]
struct HttpServerCount {
  count: usize,
  test_server: Option<Child>,
}

impl HttpServerCount {
  fn inc(&mut self) {
    self.count += 1;
    if self.test_server.is_none() {
      assert_eq!(self.count, 1);

      println!("test_server starting...");
      let mut test_server = Command::new(test_server_path())
        .current_dir(root_path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute test_server");
      let stdout = test_server.stdout.as_mut().unwrap();
      use std::io::{BufRead, BufReader};
      let lines = BufReader::new(stdout).lines();
      for maybe_line in lines {
        if let Ok(line) = maybe_line {
          if line.starts_with("ready") {
            break;
          }
        } else {
          panic!(maybe_line.unwrap_err());
        }
      }
      self.test_server = Some(test_server);
    }
  }

  fn dec(&mut self) {
    assert!(self.count > 0);
    self.count -= 1;
    if self.count == 0 {
      let mut test_server = self.test_server.take().unwrap();
      match test_server.try_wait() {
        Ok(None) => {
          test_server.kill().expect("failed to kill test_server");
          let _ = test_server.wait();
        }
        Ok(Some(status)) => {
          panic!("test_server exited unexpectedly {}", status)
        }
        Err(e) => panic!("test_server error: {}", e),
      }
    }
  }
}

impl Drop for HttpServerCount {
  fn drop(&mut self) {
    assert_eq!(self.count, 0);
    assert!(self.test_server.is_none());
  }
}

fn lock_http_server<'a>() -> MutexGuard<'a, HttpServerCount> {
  let r = GUARD.lock();
  if let Err(poison_err) = r {
    // If panics happened, ignore it. This is for tests.
    poison_err.into_inner()
  } else {
    r.unwrap()
  }
}

pub struct HttpServerGuard {}

impl Drop for HttpServerGuard {
  fn drop(&mut self) {
    let mut g = lock_http_server();
    g.dec();
  }
}

/// Adds a reference to a shared target/debug/test_server subprocess. When the
/// last instance of the HttpServerGuard is dropped, the subprocess will be
/// killed.
pub fn http_server() -> HttpServerGuard {
  let mut g = lock_http_server();
  g.inc();
  HttpServerGuard {}
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<str> {
  STRIP_ANSI_RE.replace_all(s, "")
}

pub fn run(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder.args(&cmd[1..]).stdin(Stdio::piped());

  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let status = prog.wait().expect("failed to wait on child");
  if expect_success != status.success() {
    panic!("Unexpected exit code: {:?}", status.code());
  }
}

pub fn run_collect(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) -> (String, String) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder
    .args(&cmd[1..])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = prog.wait_with_output().expect("failed to wait on child");
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{}>>>", stdout);
    eprintln!("stderr: <<<{}>>>", stderr);
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn run_and_collect_output(
  expect_success: bool,
  args: &str,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  let mut deno_process_builder = deno_cmd();
  deno_process_builder
    .args(args.split_whitespace())
    .current_dir(&tests_path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(envs) = envs {
    deno_process_builder.envs(envs);
  }
  let _http_guard = if need_http_server {
    Some(http_server())
  } else {
    None
  };
  let mut deno = deno_process_builder
    .spawn()
    .expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = deno.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = deno.wait_with_output().expect("failed to wait on child");
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{}>>>", stdout);
    eprintln!("stderr: <<<{}>>>", stderr);
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn new_deno_dir() -> TempDir {
  TempDir::new().expect("tempdir fail")
}

pub fn deno_cmd() -> Command {
  let e = deno_exe_path();
  let deno_dir = new_deno_dir();
  assert!(e.exists());
  let mut c = Command::new(e);
  c.env("DENO_DIR", deno_dir.path());
  c
}

pub fn run_powershell_script_file(
  script_file_path: &str,
  args: Vec<&str>,
) -> Result<(), i64> {
  let deno_dir = new_deno_dir();
  let mut command = Command::new("powershell.exe");

  command
    .env("DENO_DIR", deno_dir.path())
    .current_dir(root_path())
    .arg("-file")
    .arg(script_file_path);

  for arg in args {
    command.arg(arg);
  }

  let output = command.output().expect("failed to spawn script");
  let stdout = String::from_utf8(output.stdout).unwrap();
  let stderr = String::from_utf8(output.stderr).unwrap();
  println!("{}", stdout);
  if !output.status.success() {
    panic!(
      "{} executed with failing error code\n{}{}",
      script_file_path, stdout, stderr
    );
  }

  Ok(())
}

#[derive(Debug, Default)]
pub struct CheckOutputIntegrationTest {
  pub args: &'static str,
  pub output: &'static str,
  pub input: Option<&'static str>,
  pub output_str: Option<&'static str>,
  pub exit_code: i32,
  pub http_server: bool,
}

impl CheckOutputIntegrationTest {
  pub fn run(&self) {
    let args = self.args.split_whitespace();
    let root = root_path();
    let deno_exe = deno_exe_path();
    println!("root path {}", root.display());
    println!("deno_exe path {}", deno_exe.display());

    let _http_server_guard = if self.http_server {
      Some(http_server())
    } else {
      None
    };

    let (mut reader, writer) = pipe().unwrap();
    let tests_dir = root.join("cli").join("tests");
    let mut command = deno_cmd();
    println!("deno_exe args {}", self.args);
    println!("deno_exe tests path {:?}", &tests_dir);
    command.args(args);
    command.current_dir(&tests_dir);
    command.stdin(Stdio::piped());
    let writer_clone = writer.try_clone().unwrap();
    command.stderr(writer_clone);
    command.stdout(writer);

    let mut process = command.spawn().expect("failed to execute process");

    if let Some(input) = self.input {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{}", input).unwrap();
    }

    // Very important when using pipes: This parent process is still
    // holding its copies of the write ends, and we have to close them
    // before we read, otherwise the read end will never report EOF. The
    // Command object owns the writers now, and dropping it closes them.
    drop(command);

    let mut actual = String::new();
    reader.read_to_string(&mut actual).unwrap();

    let status = process.wait().expect("failed to finish process");

    if let Some(exit_code) = status.code() {
      if self.exit_code != exit_code {
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!(
          "bad exit code, expected: {:?}, actual: {:?}",
          self.exit_code, exit_code
        );
      }
    } else {
      #[cfg(unix)]
      {
        use std::os::unix::process::ExitStatusExt;
        let signal = status.signal().unwrap();
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!(
          "process terminated by signal, expected exit code: {:?}, actual signal: {:?}",
          self.exit_code, signal
        );
      }
      #[cfg(not(unix))]
      {
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!("process terminated without status code on non unix platform, expected exit code: {:?}", self.exit_code);
      }
    }

    actual = strip_ansi_codes(&actual).to_string();

    let expected = if let Some(s) = self.output_str {
      s.to_owned()
    } else {
      let output_path = tests_dir.join(self.output);
      println!("output path {}", output_path.display());
      std::fs::read_to_string(output_path).expect("cannot read output")
    };

    if !wildcard_match(&expected, &actual) {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      println!("EXPECTED\n{}\nEXPECTED", expected);
      panic!("pattern match failed");
    }
  }
}

pub fn wildcard_match(pattern: &str, s: &str) -> bool {
  pattern_match(pattern, s, "[WILDCARD]")
}

pub fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
  // Normalize line endings
  let mut s = s.replace("\r\n", "\n");
  let pattern = pattern.replace("\r\n", "\n");

  if pattern == wildcard {
    return true;
  }

  let parts = pattern.split(wildcard).collect::<Vec<&str>>();
  if parts.len() == 1 {
    return pattern == s;
  }

  if !s.starts_with(parts[0]) {
    return false;
  }

  // If the first line of the pattern is just a wildcard the newline character
  // needs to be pre-pended so it can safely match anything or nothing and
  // continue matching.
  if pattern.lines().next() == Some(wildcard) {
    s.insert_str(0, "\n");
  }

  let mut t = s.split_at(parts[0].len());

  for (i, part) in parts.iter().enumerate() {
    if i == 0 {
      continue;
    }
    dbg!(part, i);
    if i == parts.len() - 1 && (*part == "" || *part == "\n") {
      dbg!("exit 1 true", i);
      return true;
    }
    if let Some(found) = t.1.find(*part) {
      dbg!("found ", found);
      t = t.1.split_at(found + part.len());
    } else {
      dbg!("exit false ", i);
      return false;
    }
  }

  dbg!("end ", t.1.len());
  t.1.is_empty()
}

/// Kind of reflects `itest!()`. Note that the pty's output (which also contains
/// stdin content) is compared against the content of the `output` path.
#[cfg(unix)]
pub fn test_pty(args: &str, output_path: &str, input: &[u8]) {
  use pty::fork::Fork;

  let tests_path = tests_path();
  let fork = Fork::from_ptmx().unwrap();
  if let Ok(mut master) = fork.is_parent() {
    let mut output_actual = String::new();
    master.write_all(input).unwrap();
    master.read_to_string(&mut output_actual).unwrap();
    fork.wait().unwrap();

    let output_expected =
      std::fs::read_to_string(tests_path.join(output_path)).unwrap();
    if !wildcard_match(&output_expected, &output_actual) {
      println!("OUTPUT\n{}\nOUTPUT", output_actual);
      println!("EXPECTED\n{}\nEXPECTED", output_expected);
      panic!("pattern match failed");
    }
  } else {
    deno_cmd()
      .current_dir(tests_path)
      .env("NO_COLOR", "1")
      .args(args.split_whitespace())
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
  }
}

pub struct WrkOutput {
  pub latency: f64,
  pub requests: u64,
}

pub fn parse_wrk_output(output: &str) -> WrkOutput {
  lazy_static! {
    static ref REQUESTS_RX: Regex =
      Regex::new(r"Requests/sec:\s+(\d+)").unwrap();
    static ref LATENCY_RX: Regex =
      Regex::new(r"\s+99%(?:\s+(\d+.\d+)([a-z]+))").unwrap();
  }

  let mut requests = None;
  let mut latency = None;

  for line in output.lines() {
    if requests == None {
      if let Some(cap) = REQUESTS_RX.captures(line) {
        requests =
          Some(str::parse::<u64>(cap.get(1).unwrap().as_str()).unwrap());
      }
    }
    if latency == None {
      if let Some(cap) = LATENCY_RX.captures(line) {
        let time = cap.get(1).unwrap();
        let unit = cap.get(2).unwrap();

        latency = Some(
          str::parse::<f64>(time.as_str()).unwrap()
            * match unit.as_str() {
              "ms" => 1.0,
              "us" => 0.001,
              "s" => 1000.0,
              _ => unreachable!(),
            },
        );
      }
    }
  }

  WrkOutput {
    requests: requests.unwrap(),
    latency: latency.unwrap(),
  }
}

#[derive(Debug)]
pub struct StraceOutput {
  pub percent_time: f64,
  pub seconds: f64,
  pub usecs_per_call: Option<u64>,
  pub calls: u64,
  pub errors: u64,
}

pub fn parse_strace_output(output: &str) -> HashMap<String, StraceOutput> {
  let mut summary = HashMap::new();

  // Filter out non-relevant lines. See the error log at
  // https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
  // This is checked in testdata/strace_summary2.out
  let mut lines = output
    .lines()
    .filter(|line| !line.is_empty() && !line.contains("detached ..."));
  let count = lines.clone().count();

  if count < 4 {
    return summary;
  }

  let total_line = lines.next_back().unwrap();
  lines.next_back(); // Drop separator
  let data_lines = lines.skip(2);

  for line in data_lines {
    let syscall_fields = line.split_whitespace().collect::<Vec<_>>();
    let len = syscall_fields.len();
    let syscall_name = syscall_fields.last().unwrap();

    if 5 <= len && len <= 6 {
      summary.insert(
        syscall_name.to_string(),
        StraceOutput {
          percent_time: str::parse::<f64>(syscall_fields[0]).unwrap(),
          seconds: str::parse::<f64>(syscall_fields[1]).unwrap(),
          usecs_per_call: Some(str::parse::<u64>(syscall_fields[2]).unwrap()),
          calls: str::parse::<u64>(syscall_fields[3]).unwrap(),
          errors: if syscall_fields.len() < 6 {
            0
          } else {
            str::parse::<u64>(syscall_fields[4]).unwrap()
          },
        },
      );
    }
  }

  let total_fields = total_line.split_whitespace().collect::<Vec<_>>();
  summary.insert(
    "total".to_string(),
    StraceOutput {
      percent_time: str::parse::<f64>(total_fields[0]).unwrap(),
      seconds: str::parse::<f64>(total_fields[1]).unwrap(),
      usecs_per_call: None,
      calls: str::parse::<u64>(total_fields[2]).unwrap(),
      errors: str::parse::<u64>(total_fields[3]).unwrap(),
    },
  );

  summary
}

pub fn parse_max_mem(output: &str) -> Option<u64> {
  // Takes the output from "time -v" as input and extracts the 'maximum
  // resident set size' and returns it in bytes.
  for line in output.lines() {
    if line
      .to_lowercase()
      .contains("maximum resident set size (kbytes)")
    {
      let value = line.split(": ").nth(1).unwrap();
      return Some(str::parse::<u64>(value).unwrap() * 1024);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_wrk_output_1() {
    const TEXT: &str = include_str!("./testdata/wrk1.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 1837);
    assert!((wrk.latency - 6.25).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_2() {
    const TEXT: &str = include_str!("./testdata/wrk2.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 53435);
    assert!((wrk.latency - 6.22).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_3() {
    const TEXT: &str = include_str!("./testdata/wrk3.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 96037);
    assert!((wrk.latency - 6.36).abs() < f64::EPSILON);
  }

  #[test]
  fn strace_parse_1() {
    const TEXT: &str = include_str!("./testdata/strace_summary.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let munmap = strace.get("munmap").unwrap();
    assert_eq!(munmap.calls, 60);
    assert_eq!(munmap.errors, 0);

    // line with errors
    assert_eq!(strace.get("mkdir").unwrap().errors, 2);

    // last syscall line
    let prlimit = strace.get("prlimit64").unwrap();
    assert_eq!(prlimit.calls, 2);
    assert!((prlimit.percent_time - 0.0).abs() < f64::EPSILON);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 704);
    assert_eq!(strace.get("total").unwrap().errors, 5);
  }

  #[test]
  fn strace_parse_2() {
    const TEXT: &str = include_str!("./testdata/strace_summary2.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("futex").unwrap();
    assert_eq!(futex.calls, 449);
    assert_eq!(futex.errors, 94);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 821);
    assert_eq!(strace.get("total").unwrap().errors, 107);
  }

  #[test]
  fn test_wildcard_match() {
    let fixtures = vec![
      ("foobarbaz", "foobarbaz", true),
      ("[WILDCARD]", "foobarbaz", true),
      ("foobar", "foobarbaz", false),
      ("foo[WILDCARD]baz", "foobarbaz", true),
      ("foo[WILDCARD]baz", "foobazbar", false),
      ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", true),
      ("foo[WILDCARD]", "foobar", true),
      ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", true),
      // check with different line endings
      ("foo[WILDCARD]\nbaz[WILDCARD]\n", "foobar\nbazqat\n", true),
      (
        "foo[WILDCARD]\nbaz[WILDCARD]\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\n",
        "foobar\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\nbazqat\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
    ];

    // Iterate through the fixture lists, testing each one
    for (pattern, string, expected) in fixtures {
      let actual = wildcard_match(pattern, string);
      dbg!(pattern, string, expected);
      assert_eq!(actual, expected);
    }
  }

  #[test]
  fn max_mem_parse() {
    const TEXT: &str = include_str!("./testdata/time.out");
    let size = parse_max_mem(TEXT);

    assert_eq!(size, Some(120380 * 1024));
  }
}
