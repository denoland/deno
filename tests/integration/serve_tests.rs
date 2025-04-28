// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::time::Duration;

use pretty_assertions::assert_eq;
use regex::Regex;
use reqwest::RequestBuilder;
use test_util as util;
use test_util::DenoChild;
use tokio::time::timeout;

struct ServeClient {
  child: RefCell<DenoChild>,
  client: reqwest::Client,
  output_buf: RefCell<Vec<u8>>,
  endpoint: RefCell<Option<String>>,
}

impl Drop for ServeClient {
  fn drop(&mut self) {
    let mut child = self.child.borrow_mut();
    child.kill().unwrap();
    child.wait().unwrap();
  }
}

struct ServeClientBuilder(util::TestCommandBuilder, Option<String>);

impl ServeClientBuilder {
  fn build(self) -> ServeClient {
    let Some(entry_point) = self.1 else {
      panic!("entry point required");
    };
    let cmd = self.0.arg(entry_point);
    let child = cmd.spawn().unwrap();

    ServeClient::with_child(child)
  }

  fn map(
    self,
    f: impl FnOnce(util::TestCommandBuilder) -> util::TestCommandBuilder,
  ) -> Self {
    Self(f(self.0), self.1)
  }

  fn entry_point(self, file: impl AsRef<str>) -> Self {
    Self(self.0, Some(file.as_ref().into()))
  }

  fn worker_count(self, n: Option<u64>) -> Self {
    self.map(|t| {
      let t = t.arg("--parallel");
      if let Some(n) = n {
        t.env("DENO_JOBS", n.to_string())
      } else {
        t
      }
    })
  }

  fn new() -> Self {
    Self(
      util::deno_cmd()
        .env("NO_COLOR", "1")
        .current_dir(util::testdata_path())
        .arg("serve")
        .arg("--port")
        .arg("0")
        .stdout_piped()
        .stderr_piped(),
      None,
    )
  }
}

impl ServeClient {
  fn builder() -> ServeClientBuilder {
    ServeClientBuilder::new()
  }

  fn with_child(child: DenoChild) -> Self {
    Self {
      child: RefCell::new(child),
      output_buf: Default::default(),
      endpoint: Default::default(),
      client: reqwest::Client::builder()
        .add_root_certificate(
          reqwest::Certificate::from_pem(include_bytes!(
            "../testdata/tls/RootCA.crt"
          ))
          .unwrap(),
        )
        // disable connection pooling so we create a new connection per request
        // which allows us to distribute requests across workers
        .pool_max_idle_per_host(0)
        .pool_idle_timeout(Duration::from_nanos(1))
        .http2_prior_knowledge()
        .build()
        .unwrap(),
    }
  }

  fn kill(self) {
    let mut child = self.child.borrow_mut();
    child.kill().unwrap();
    child.wait().unwrap();
  }

  fn output(self) -> String {
    let mut child = self.child.borrow_mut();
    child.kill().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    child.wait().unwrap();

    let mut output_buf = self.output_buf.borrow_mut();

    stderr.read_to_end(&mut output_buf).unwrap();

    String::from_utf8(std::mem::take(&mut *output_buf)).unwrap()
  }

  fn get(&self) -> RequestBuilder {
    let endpoint = self.endpoint();
    self.client.get(&*endpoint)
  }

  fn endpoint(&self) -> String {
    if let Some(e) = self.endpoint.borrow().as_ref() {
      return e.to_string();
    };
    let mut buffer = self.output_buf.borrow_mut();
    let mut temp_buf = [0u8; 64];
    let mut child = self.child.borrow_mut();
    let stderr = child.stderr.as_mut().unwrap();
    let port_regex =
      regex::bytes::Regex::new(r"Listening on https?:[^:]+:(\d+)/").unwrap();

    let start = std::time::Instant::now();
    // try to find the port number in the output
    // it may not be the first line, so we need to read the output in a loop
    let port = loop {
      if start.elapsed() > Duration::from_secs(5) {
        panic!(
          "timed out waiting for serve to start. serve output:\n{}",
          String::from_utf8_lossy(&buffer)
        );
      }
      let read = stderr.read(&mut temp_buf).unwrap();
      buffer.extend_from_slice(&temp_buf[..read]);
      if let Some(p) = port_regex
        .captures(&buffer)
        .and_then(|c| c.get(1))
        .map(|v| std::str::from_utf8(v.as_bytes()).unwrap().to_owned())
      {
        break p;
      }
      // this is technically blocking, but it's just a test and
      // I don't want to switch RefCell to Mutex just for this
      std::thread::sleep(Duration::from_millis(10));
    };

    eprintln!("stderr: {}", String::from_utf8_lossy(&temp_buf));

    self
      .endpoint
      .replace(Some(format!("http://127.0.0.1:{port}")));

    return self.endpoint.borrow().clone().unwrap();
  }
}

#[tokio::test]
async fn deno_serve_port_0() {
  let client = ServeClient::builder()
    .entry_point("./serve/port_0.ts")
    .build();
  let res = client.get().send().await.unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "deno serve --port 0 works!");
  client.kill();
}

#[tokio::test]
async fn deno_serve_no_args() {
  let client = ServeClient::builder()
    .entry_point("./serve/no_args.ts")
    .build();
  let res = client.get().send().await.unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "deno serve with no args in fetch() works!");
}

#[tokio::test]
async fn deno_serve_parallel() {
  let client = ServeClient::builder()
    .entry_point("./serve/parallel.ts")
    .worker_count(Some(4))
    .build();

  let mut serve_counts = HashMap::<u32, u32>::new();

  tokio::time::sleep(Duration::from_millis(1000)).await;

  let serve_regex =
    Regex::new(r"\[serve\-worker\-(\d+)\s*\] serving request").unwrap();

  for _ in 0..100 {
    let response = timeout(Duration::from_secs(2), client.get().send())
      .await
      .unwrap()
      .unwrap();
    assert_eq!(200, response.status());
    let body = response.text().await.unwrap();
    assert_eq!(body, "deno serve parallel");
    tokio::time::sleep(Duration::from_millis(1)).await;
  }

  let output = client.output();

  let listening_regex =
    Regex::new(r"Listening on http[\w:/\.]+ with (\d+) threads").unwrap();

  eprintln!("serve output:\n{output}");
  assert_eq!(
    listening_regex
      .captures(&output)
      .unwrap()
      .get(1)
      .unwrap()
      .as_str()
      .trim(),
    "4"
  );

  // make sure all workers have at least started
  let mut started = [false; 4];
  let start_regex =
    Regex::new(r"\[serve\-worker\-(\d+)\s*\] starting serve").unwrap();
  for capture in start_regex.captures_iter(&output) {
    if let Some(worker_number) =
      capture.get(1).and_then(|m| m.as_str().parse::<u32>().ok())
    {
      started[worker_number as usize] = true;
    }
  }
  assert!(started.iter().all(|&b| b));

  for capture in serve_regex.captures_iter(&output) {
    if let Some(worker_number) =
      capture.get(1).and_then(|m| m.as_str().parse::<u32>().ok())
    {
      *serve_counts.entry(worker_number).or_default() += 1;
    }
  }

  #[cfg(not(target_vendor = "apple"))] // FIXME: flaky on macOS, it tends to not distribute requests evenly
  assert!(
    serve_counts.values().filter(|&&n| n > 2).count() >= 2,
    "bad {serve_counts:?}"
  );
}

#[tokio::test]
async fn deno_run_serve_with_tcp_from_env() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-net")
    .arg("./serve/run_serve.ts")
    .env("DENO_SERVE_ADDRESS", "tcp:127.0.0.1:0")
    .stderr_piped()
    .spawn()
    .unwrap();
  let stderr = BufReader::new(child.stderr.as_mut().unwrap());
  let msg = stderr.lines().next().unwrap().unwrap();

  // Deno.serve() listens on 0.0.0.0 by default. This checks DENO_SERVE_ADDRESS
  // is not ignored by ensuring it's listening on 127.0.0.1.
  let port_regex = Regex::new(r"http:\/\/127\.0\.0\.1:(\d+)").unwrap();
  let port = port_regex.captures(&msg).unwrap().get(1).unwrap().as_str();

  let client = reqwest::Client::builder().build().unwrap();

  let res = client
    .get(format!("http://127.0.0.1:{port}"))
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
    .arg(format!("--allow-read={}", sock.display()))
    .arg(format!("--allow-write={}", sock.display()))
    .arg("./serve/run_serve.ts")
    .env("DENO_SERVE_ADDRESS", format!("unix:{}", sock.display()))
    .stderr_piped()
    .spawn()
    .unwrap();
  let stderr = BufReader::new(child.stderr.as_mut().unwrap());
  stderr.lines().next().unwrap().unwrap();

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
