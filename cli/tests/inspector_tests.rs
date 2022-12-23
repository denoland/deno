// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::futures::prelude::*;
use deno_core::futures::stream::SplitSink;
use deno_core::futures::stream::SplitStream;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_websocket::tokio_tungstenite;
use deno_runtime::deno_websocket::tokio_tungstenite::tungstenite;
use std::io::BufRead;
use std::process::Child;
use test_util as util;
use test_util::TempDir;
use tokio::net::TcpStream;
use util::http_server;

mod inspector {
  use super::*;

  struct InspectorTester {
    socket_tx: SplitSink<
      tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<TcpStream>,
      >,
      tungstenite::Message,
    >,
    socket_rx: SplitStream<
      tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<TcpStream>,
      >,
    >,
    notification_filter: Box<dyn FnMut(&str) -> bool + 'static>,
    child: Child,
    stderr_lines: Box<dyn Iterator<Item = String>>,
    stdout_lines: Box<dyn Iterator<Item = String>>,
  }

  fn ignore_script_parsed(msg: &str) -> bool {
    !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#)
  }

  impl InspectorTester {
    async fn create<F>(mut child: Child, notification_filter: F) -> Self
    where
      F: FnMut(&str) -> bool + 'static,
    {
      let stdout = child.stdout.take().unwrap();
      let stdout_lines =
        std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

      let stderr = child.stderr.take().unwrap();
      let mut stderr_lines =
        std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

      let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

      let (socket, response) =
        tokio_tungstenite::connect_async(ws_url).await.unwrap();
      assert_eq!(response.status(), 101); // Switching protocols.

      let (socket_tx, socket_rx) = socket.split();

      Self {
        socket_tx,
        socket_rx,
        notification_filter: Box::new(notification_filter),
        child,
        stderr_lines: Box::new(stderr_lines),
        stdout_lines: Box::new(stdout_lines),
      }
    }

    async fn send_many(&mut self, messages: &[serde_json::Value]) {
      // TODO(bartlomieju): add graceful error handling
      for msg in messages {
        let result = self
          .socket_tx
          .send(msg.to_string().into())
          .await
          .map_err(|e| e.into());
        self.handle_error(result);
      }
    }

    async fn send(&mut self, message: serde_json::Value) {
      self.send_many(&[message]).await;
    }

    fn handle_error<T>(&mut self, result: Result<T, AnyError>) -> T {
      match result {
        Ok(result) => result,
        Err(err) => {
          let mut stdout = vec![];
          for line in self.stdout_lines.by_ref() {
            stdout.push(line);
          }
          let mut stderr = vec![];
          for line in self.stderr_lines.by_ref() {
            stderr.push(line);
          }
          let stdout = stdout.join("\n");
          let stderr = stderr.join("\n");
          self.child.kill().unwrap();

          panic!(
            "Inspector test failed with error: {:?}.\nstdout:\n{}\nstderr:\n{}",
            err, stdout, stderr
          );
        }
      }
    }

    async fn recv(&mut self) -> String {
      loop {
        let result = self.socket_rx.next().await.unwrap().map_err(|e| e.into());
        let message = self.handle_error(result).to_string();
        if (self.notification_filter)(&message) {
          return message;
        }
      }
    }

    async fn recv_as_json(&mut self) -> serde_json::Value {
      let msg = self.recv().await;
      serde_json::from_str(&msg).unwrap()
    }

    async fn assert_received_messages(
      &mut self,
      responses: &[&str],
      notifications: &[&str],
    ) {
      let expected_messages = responses.len() + notifications.len();
      let mut responses_idx = 0;
      let mut notifications_idx = 0;

      for _ in 0..expected_messages {
        let msg = self.recv().await;

        if msg.starts_with(r#"{"id":"#) {
          assert!(
            msg.starts_with(responses[responses_idx]),
            "Doesn't start with {}, instead received {}",
            responses[responses_idx],
            msg
          );
          responses_idx += 1;
        } else {
          assert!(
            msg.starts_with(notifications[notifications_idx]),
            "Doesn't start with {}, instead received {}",
            notifications[notifications_idx],
            msg
          );
          notifications_idx += 1;
        }
      }
    }

    fn stderr_line(&mut self) -> String {
      self.stderr_lines.next().unwrap()
    }

    fn stdout_line(&mut self) -> String {
      self.stdout_lines.next().unwrap()
    }

    fn assert_stderr_for_inspect(&mut self) {
      assert_stderr(
        &mut self.stderr_lines,
        &["Visit chrome://inspect to connect to the debugger."],
      );
    }

    fn assert_stderr_for_inspect_brk(&mut self) {
      assert_stderr(
        &mut self.stderr_lines,
        &[
          "Visit chrome://inspect to connect to the debugger.",
          "Deno is waiting for debugger to connect.",
        ],
      );
    }
  }

  macro_rules! assert_starts_with {
    ($string:expr, $($test:expr),+) => {
      let string = $string; // This might be a function call or something
      if !($(string.starts_with($test))||+) {
        panic!("{:?} does not start with {:?}", string, [$($test),+]);
      }
    }
  }

  fn assert_stderr(
    stderr_lines: &mut impl std::iter::Iterator<Item = String>,
    expected_lines: &[&str],
  ) {
    let mut expected_index = 0;

    loop {
      let line = skip_check_line(stderr_lines);

      assert_eq!(line, expected_lines[expected_index]);
      expected_index += 1;

      if expected_index >= expected_lines.len() {
        break;
      }
    }
  }

  fn inspect_flag_with_unique_port(flag_prefix: &str) -> String {
    use std::sync::atomic::{AtomicU16, Ordering};
    static PORT: AtomicU16 = AtomicU16::new(9229);
    let port = PORT.fetch_add(1, Ordering::Relaxed);
    format!("{}=127.0.0.1:{}", flag_prefix, port)
  }

  fn extract_ws_url_from_stderr(
    stderr_lines: &mut impl std::iter::Iterator<Item = String>,
  ) -> url::Url {
    let stderr_first_line = skip_check_line(stderr_lines);
    assert_starts_with!(&stderr_first_line, "Debugger listening on ");
    let v: Vec<_> = stderr_first_line.match_indices("ws:").collect();
    assert_eq!(v.len(), 1);
    let ws_url_index = v[0].0;
    let ws_url = &stderr_first_line[ws_url_index..];
    url::Url::parse(ws_url).unwrap()
  }

  fn skip_check_line(
    stderr_lines: &mut impl std::iter::Iterator<Item = String>,
  ) -> String {
    loop {
      let mut line = stderr_lines.next().unwrap();
      line = util::strip_ansi_codes(&line).to_string();

      if line.starts_with("Check") {
        continue;
      }

      return line;
    }
  }

  #[tokio::test]
  async fn inspector_connect() {
    let script = util::testdata_path().join("inspector/inspector1.js");
    let mut child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr = child.stderr.as_mut().unwrap();
    let mut stderr_lines =
      std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
    let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

    // We use tokio_tungstenite as a websocket client because warp (which is
    // a dependency of Deno) uses it.
    let (_socket, response) =
      tokio_tungstenite::connect_async(ws_url).await.unwrap();
    assert_eq!("101 Switching Protocols", response.status().to_string());
    child.kill().unwrap();
    child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_break_on_first_line() {
    let script = util::testdata_path().join("inspector/inspector2.js");
    let child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({
        "id":4,
        "method":"Runtime.evaluate",
        "params":{
          "expression":"Deno.core.print(\"hello from the inspector\\n\")",
          "contextId":1,
          "includeCommandLineAPI":true,
          "silent":false,
          "returnByValue":true
        }
      }))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":4,"result":{"result":{"type":"undefined"}}}"#],
        &[],
      )
      .await;

    assert_eq!(
      &tester.stdout_lines.next().unwrap(),
      "hello from the inspector"
    );

    tester
      .send(json!({"id":5,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":5,"result":{}}"#], &[])
      .await;

    assert_eq!(
      &tester.stdout_lines.next().unwrap(),
      "hello from the script"
    );

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_pause() {
    let script = util::testdata_path().join("inspector/inspector1.js");
    let child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester
      .send(json!({"id":6,"method":"Debugger.enable"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":6,"result":{"debuggerId":"#], &[])
      .await;

    tester
      .send(json!({"id":31,"method":"Debugger.pause"}))
      .await;

    tester
      .assert_received_messages(&[r#"{"id":31,"result":{}}"#], &[])
      .await;

    tester.child.kill().unwrap();
  }

  #[tokio::test]
  async fn inspector_port_collision() {
    // Skip this test on WSL, which allows multiple processes to listen on the
    // same port, rather than making `bind()` fail with `EADDRINUSE`.
    if cfg!(target_os = "linux")
      && std::env::var_os("WSL_DISTRO_NAME").is_some()
    {
      return;
    }

    let script = util::testdata_path().join("inspector/inspector1.js");
    let inspect_flag = inspect_flag_with_unique_port("--inspect");

    let mut child1 = util::deno_cmd()
      .arg("run")
      .arg(&inspect_flag)
      .arg(script.clone())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr_1 = child1.stderr.as_mut().unwrap();
    let mut stderr_1_lines = std::io::BufReader::new(stderr_1)
      .lines()
      .map(|r| r.unwrap());
    let _ = extract_ws_url_from_stderr(&mut stderr_1_lines);

    let mut child2 = util::deno_cmd()
      .arg("run")
      .arg(&inspect_flag)
      .arg(script)
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr_2 = child2.stderr.as_mut().unwrap();
    let stderr_2_error_message = std::io::BufReader::new(stderr_2)
      .lines()
      .map(|r| r.unwrap())
      .inspect(|line| assert!(!line.contains("Debugger listening")))
      .find(|line| line.contains("Cannot start inspector server"));
    assert!(stderr_2_error_message.is_some());

    child1.kill().unwrap();
    child1.wait().unwrap();
    child2.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_does_not_hang() {
    let script = util::testdata_path().join("inspector/inspector3.js");
    let child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .env("NO_COLOR", "1")
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({"id":4,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":4,"result":{}}"#],
        &[r#"{"method":"Debugger.resumed","params":{}}"#],
      )
      .await;

    for i in 0..128u32 {
      let request_id = i + 10;
      // Expect the number {i} on stdout.
      let s = i.to_string();
      assert_eq!(tester.stdout_lines.next().unwrap(), s);

      tester
        .assert_received_messages(
          &[],
          &[
            r#"{"method":"Runtime.consoleAPICalled","#,
            r#"{"method":"Debugger.paused","#,
          ],
        )
        .await;

      tester
        .send(json!({"id":request_id,"method":"Debugger.resume"}))
        .await;
      tester
        .assert_received_messages(
          &[&format!(r#"{{"id":{},"result":{{}}}}"#, request_id)],
          &[r#"{"method":"Debugger.resumed","params":{}}"#],
        )
        .await;
    }

    // Check that we can gracefully close the websocket connection.
    tester.socket_tx.close().await.unwrap();
    tester.socket_rx.for_each(|_| async {}).await;

    assert_eq!(&tester.stdout_lines.next().unwrap(), "done");
    assert!(tester.child.wait().unwrap().success());
  }

  #[tokio::test]
  async fn inspector_without_brk_runs_code() {
    let script = util::testdata_path().join("inspector/inspector4.js");
    let mut child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr = child.stderr.as_mut().unwrap();
    let mut stderr_lines =
      std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
    let _ = extract_ws_url_from_stderr(&mut stderr_lines);

    // Check that inspector actually runs code without waiting for inspector
    // connection.
    let stdout = child.stdout.as_mut().unwrap();
    let mut stdout_lines =
      std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
    let stdout_first_line = stdout_lines.next().unwrap();
    assert_eq!(stdout_first_line, "hello");

    child.kill().unwrap();
    child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_runtime_evaluate_does_not_crash() {
    let child = util::deno_cmd()
      .arg("repl")
      .arg("--allow-read")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    let stdin = tester.child.stdin.take().unwrap();

    tester.assert_stderr_for_inspect();
    assert_starts_with!(&tester.stdout_line(), "Deno");
    assert_eq!(
      &tester.stdout_line(),
      "exit using ctrl+d, ctrl+c, or close()"
    );
    assert_eq!(&tester.stderr_line(), "Debugger session started.");

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({
        "id":3,
        "method":"Runtime.compileScript",
        "params":{
          "expression":"Deno.cwd()",
          "sourceURL":"",
          "persistScript":false,
          "executionContextId":1
        }
      }))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":3,"result":{}}"#], &[])
      .await;
    tester
      .send(json!({
        "id":4,
        "method":"Runtime.evaluate",
        "params":{
          "expression":"Deno.cwd()",
          "objectGroup":"console",
          "includeCommandLineAPI":true,
          "silent":false,
          "contextId":1,
          "returnByValue":true,
          "generatePreview":true,
          "userGesture":true,
          "awaitPromise":false,
          "replMode":true
        }
      }))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":4,"result":{"result":{"type":"string","value":""#],
        &[],
      )
      .await;
    tester
      .send(json!({
        "id":5,
        "method":"Runtime.evaluate",
        "params":{
          "expression":"console.error('done');",
          "objectGroup":"console",
          "includeCommandLineAPI":true,
          "silent":false,
          "contextId":1,
          "returnByValue":true,
          "generatePreview":true,
          "userGesture":true,
          "awaitPromise":false,
          "replMode":true
        }
      }))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":5,"result":{"result":{"type":"undefined"}}}"#],
        &[r#"{"method":"Runtime.consoleAPICalled"#],
      )
      .await;
    assert_eq!(&tester.stderr_line(), "done");
    drop(stdin);
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_json() {
    let script = util::testdata_path().join("inspector/inspector1.js");
    let mut child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr = child.stderr.as_mut().unwrap();
    let mut stderr_lines =
      std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
    let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);
    let mut url = ws_url.clone();
    let _ = url.set_scheme("http");
    url.set_path("/json");
    let resp = reqwest::get(url).await.unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let endpoint_list: Vec<deno_core::serde_json::Value> =
      serde_json::from_str(&resp.text().await.unwrap()).unwrap();
    let matching_endpoint = endpoint_list
      .iter()
      .find(|e| e["webSocketDebuggerUrl"] == ws_url.as_str());
    assert!(matching_endpoint.is_some());
    child.kill().unwrap();
  }

  #[tokio::test]
  async fn inspector_json_list() {
    let script = util::testdata_path().join("inspector/inspector1.js");
    let mut child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr = child.stderr.as_mut().unwrap();
    let mut stderr_lines =
      std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
    let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);
    let mut url = ws_url.clone();
    let _ = url.set_scheme("http");
    url.set_path("/json/list");
    let resp = reqwest::get(url).await.unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let endpoint_list: Vec<deno_core::serde_json::Value> =
      serde_json::from_str(&resp.text().await.unwrap()).unwrap();
    let matching_endpoint = endpoint_list
      .iter()
      .find(|e| e["webSocketDebuggerUrl"] == ws_url.as_str());
    assert!(matching_endpoint.is_some());
    child.kill().unwrap();
  }

  #[tokio::test]
  async fn inspector_connect_non_ws() {
    // https://github.com/denoland/deno/issues/11449
    // Verify we don't panic if non-WS connection is being established
    let script = util::testdata_path().join("inspector/inspector1.js");
    let mut child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect"))
      .arg(script)
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let stderr = child.stderr.as_mut().unwrap();
    let mut stderr_lines =
      std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
    let mut ws_url = extract_ws_url_from_stderr(&mut stderr_lines);
    // Change scheme to URL and try send a request. We're not interested
    // in the request result, just that the process doesn't panic.
    ws_url.set_scheme("http").unwrap();
    let resp = reqwest::get(ws_url).await.unwrap();
    assert_eq!("400 Bad Request", resp.status().to_string());
    child.kill().unwrap();
    child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_break_on_first_line_in_test() {
    let script = util::testdata_path().join("inspector/inspector_test.js");
    let child = util::deno_cmd()
      .arg("test")
      .arg("--quiet")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .env("NO_COLOR", "1")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({
        "id":4,
        "method":"Runtime.evaluate",
        "params":{
          "expression":"1 + 1",
          "contextId":1,
          "includeCommandLineAPI":true,
          "silent":false,
          "returnByValue":true
        }
      }))
      .await;
    tester.assert_received_messages(
      &[r#"{"id":4,"result":{"result":{"type":"number","value":2,"description":"2"}}}"#],
      &[],
    )
    .await;

    tester
      .send(json!({"id":5,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":5,"result":{}}"#], &[])
      .await;

    assert_starts_with!(&tester.stdout_line(), "running 1 test from");
    assert!(&tester.stdout_line().contains("basic test ... ok"));

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_with_ts_files() {
    let script = util::testdata_path().join("inspector/test.ts");
    let child = util::deno_cmd()
      .arg("run")
      .arg("--check")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    fn notification_filter(msg: &str) -> bool {
      (msg.starts_with(r#"{"method":"Debugger.scriptParsed","#)
        && msg.contains("testdata/inspector"))
        || !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#)
    }

    let mut tester = InspectorTester::create(child, notification_filter).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    // receive messages with sources from this test
    let script1 = tester.recv().await;
    assert!(script1.contains("testdata/inspector/test.ts"));
    let script1_id = {
      let v: serde_json::Value = serde_json::from_str(&script1).unwrap();
      v["params"]["scriptId"].as_str().unwrap().to_string()
    };
    let script2 = tester.recv().await;
    assert!(script2.contains("testdata/inspector/foo.ts"));
    let script2_id = {
      let v: serde_json::Value = serde_json::from_str(&script2).unwrap();
      v["params"]["scriptId"].as_str().unwrap().to_string()
    };
    let script3 = tester.recv().await;
    assert!(script3.contains("testdata/inspector/bar.js"));
    let script3_id = {
      let v: serde_json::Value = serde_json::from_str(&script3).unwrap();
      v["params"]["scriptId"].as_str().unwrap().to_string()
    };

    tester
      .assert_received_messages(&[r#"{"id":2,"result":{"debuggerId":"#], &[])
      .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester.send_many(
    &[
      json!({"id":4,"method":"Debugger.getScriptSource","params":{"scriptId":script1_id.as_str()}}),
      json!({"id":5,"method":"Debugger.getScriptSource","params":{"scriptId":script2_id.as_str()}}),
      json!({"id":6,"method":"Debugger.getScriptSource","params":{"scriptId":script3_id.as_str()}}),
    ])
    .await;
    tester.assert_received_messages(
      &[
        r#"{"id":4,"result":{"scriptSource":"import { foo } from \"./foo.ts\";\nimport { bar } from \"./bar.js\";\nconsole.log(foo());\nconsole.log(bar());\n//# sourceMappingURL=data:application/json;base64,"#,
        r#"{"id":5,"result":{"scriptSource":"class Foo {\n    hello() {\n        return \"hello\";\n    }\n}\nexport function foo() {\n    const f = new Foo();\n    return f.hello();\n}\n//# sourceMappingURL=data:application/json;base64,"#,
        r#"{"id":6,"result":{"scriptSource":"export function bar() {\n  return \"world\";\n}\n"#,
      ],
      &[],
    )
    .await;

    tester
      .send(json!({"id":7,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":7,"result":{}}"#], &[])
      .await;

    assert_eq!(&tester.stdout_line(), "hello");
    assert_eq!(&tester.stdout_line(), "world");

    tester.assert_received_messages(
      &[],
      &[
        r#"{"method":"Debugger.resumed","params":{}}"#,
        r#"{"method":"Runtime.consoleAPICalled","#,
        r#"{"method":"Runtime.consoleAPICalled","#,
        r#"{"method":"Runtime.executionContextDestroyed","params":{"executionContextId":1}}"#,
      ],
    )
    .await;

    assert_eq!(
      &tester.stdout_line(),
      "Program finished. Waiting for inspector to disconnect to exit the process..."
    );

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_memory() {
    let script = util::testdata_path().join("inspector/memory.js");
    let child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send_many(&[
        json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}),
        json!({"id":4,"method":"HeapProfiler.enable"}),
      ])
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#, r#"{"id":4,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({"id":5,"method":"Runtime.getHeapUsage", "params": {}}))
      .await;

    let json_msg = tester.recv_as_json().await;
    assert_eq!(json_msg["id"].as_i64().unwrap(), 5);
    let result = &json_msg["result"];
    assert!(
      result["usedSize"].as_i64().unwrap()
        <= result["totalSize"].as_i64().unwrap()
    );

    tester
      .send(json!({
        "id":6,
        "method":"HeapProfiler.takeHeapSnapshot",
        "params": {
          "reportProgress": true,
          "treatGlobalObjectsAsRoots": true,
          "captureNumberValue": false
        }
      }))
      .await;

    let mut progress_report_completed = false;
    loop {
      let msg = tester.recv().await;

      // TODO(bartlomieju): can be abstracted
      if !progress_report_completed
        && msg.starts_with(
          r#"{"method":"HeapProfiler.reportHeapSnapshotProgress","params""#,
        )
      {
        let json_msg: serde_json::Value = serde_json::from_str(&msg).unwrap();
        if let Some(finished) = json_msg["params"].get("finished") {
          progress_report_completed = finished.as_bool().unwrap();
        }
        continue;
      }

      if msg.starts_with(r#"{"method":"HeapProfiler.reportHeapSnapshotProgress","params":{"done":"#,) {
        continue;
      }

      if msg.starts_with(r#"{"id":6,"result":{}}"#) {
        assert!(progress_report_completed);
        break;
      }
    }

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_profile() {
    let script = util::testdata_path().join("inspector/memory.js");
    let child = util::deno_cmd()
      .arg("run")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send_many(&[
        json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}),
        json!({"id":4,"method":"Profiler.enable"}),
      ])
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#, r#"{"id":4,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester.send_many(
      &[
        json!({"id":5,"method":"Profiler.setSamplingInterval","params":{"interval": 100}}),
        json!({"id":6,"method":"Profiler.start","params":{}}),
      ],
    ).await;
    tester
      .assert_received_messages(
        &[r#"{"id":5,"result":{}}"#, r#"{"id":6,"result":{}}"#],
        &[],
      )
      .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    tester
      .send(json!({"id":7,"method":"Profiler.stop", "params": {}}))
      .await;
    let json_msg = tester.recv_as_json().await;
    assert_eq!(json_msg["id"].as_i64().unwrap(), 7);
    let result = &json_msg["result"];
    let profile = &result["profile"];
    assert!(
      profile["startTime"].as_i64().unwrap()
        < profile["endTime"].as_i64().unwrap()
    );
    profile["samples"].as_array().unwrap();
    profile["nodes"].as_array().unwrap();

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_break_on_first_line_npm_esm() {
    let _server = http_server();

    let child = util::deno_cmd()
      .arg("run")
      .arg("--quiet")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg("npm:@denotest/bin/cli-esm")
      .arg("this")
      .arg("is")
      .arg("a")
      .arg("test")
      .envs(util::env_vars_for_npm_tests())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({"id":4,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":4,"result":{}}"#], &[])
      .await;

    assert_eq!(&tester.stdout_line(), "this");
    assert_eq!(&tester.stdout_line(), "is");
    assert_eq!(&tester.stdout_line(), "a");
    assert_eq!(&tester.stdout_line(), "test");

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_break_on_first_line_npm_cjs() {
    let _server = http_server();
    let child = util::deno_cmd()
      .arg("run")
      .arg("--quiet")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg("npm:@denotest/bin/cli-cjs")
      .arg("this")
      .arg("is")
      .arg("a")
      .arg("test")
      .envs(util::env_vars_for_npm_tests())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({"id":4,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":4,"result":{}}"#], &[])
      .await;

    assert_eq!(&tester.stdout_line(), "this");
    assert_eq!(&tester.stdout_line(), "is");
    assert_eq!(&tester.stdout_line(), "a");
    assert_eq!(&tester.stdout_line(), "test");

    tester.child.kill().unwrap();
    tester.child.wait().unwrap();
  }

  #[tokio::test]
  async fn inspector_error_with_npm_import() {
    let script =
      util::testdata_path().join("inspector/error_with_npm_import.js");
    let _server = http_server();

    let child = util::deno_cmd()
      .arg("run")
      .arg("--quiet")
      .arg("-A")
      .arg(inspect_flag_with_unique_port("--inspect-brk"))
      .arg(script)
      .envs(util::env_vars_for_npm_tests())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();

    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;

    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(
        &[r#"{"id":3,"result":{}}"#],
        &[r#"{"method":"Debugger.paused","#],
      )
      .await;

    tester
      .send(json!({"id":4,"method":"Debugger.resume"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":4,"result":{}}"#], &[])
      .await;

    // TODO(bartlomieju): this is a partial fix, we should assert that
    // "Runtime.exceptionThrown" notification was sent, but a bindings for this
    // notification is not yet there
    assert_eq!(&tester.stderr_line(), "Debugger session started.");
    assert_eq!(&tester.stderr_line(), "error: Uncaught Error: boom!");

    assert_eq!(tester.child.wait().unwrap().code(), Some(1));
  }

  #[tokio::test]
  async fn inspector_wait() {
    let script = util::testdata_path().join("inspector/inspect_wait.js");
    let temp_dir = TempDir::new();

    let child = util::deno_cmd()
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--quiet")
      .arg("-A")
      .arg(inspect_flag_with_unique_port("--inspect-wait"))
      .arg(script)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert!(!temp_dir.path().join("hello.txt").exists());

    let mut tester = InspectorTester::create(child, ignore_script_parsed).await;

    tester.assert_stderr_for_inspect_brk();
    tester
      .send_many(&[
        json!({"id":1,"method":"Runtime.enable"}),
        json!({"id":2,"method":"Debugger.enable"}),
      ])
      .await;
    tester.assert_received_messages(
      &[
        r#"{"id":1,"result":{}}"#,
        r#"{"id":2,"result":{"debuggerId":"#,
      ],
      &[
        r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
      ],
    )
    .await;
    // TODO(bartlomieju): ideally this shouldn't be needed, but currently there's
    // no way to express that in inspector code. Most clients always send this
    // message anyway.
    tester
      .send(json!({"id":3,"method":"Runtime.runIfWaitingForDebugger"}))
      .await;
    tester
      .assert_received_messages(&[r#"{"id":3,"result":{}}"#], &[])
      .await;
    assert_eq!(&tester.stderr_line(), "Debugger session started.");
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(&tester.stderr_line(), "did run");
    assert!(temp_dir.path().join("hello.txt").exists());
    tester.child.kill().unwrap();
  }
}
