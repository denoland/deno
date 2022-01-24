// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::futures;
use deno_core::futures::prelude::*;
use deno_core::futures::stream::SplitSink;
use deno_core::serde_json;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_websocket::tokio_tungstenite;
use deno_runtime::deno_websocket::tokio_tungstenite::tungstenite;
use std::io::BufRead;
use std::pin::Pin;
use test_util as util;
use tokio::net::TcpStream;

macro_rules! assert_starts_with {
  ($string:expr, $($test:expr),+) => {
    let string = $string; // This might be a function call or something
    if !($(string.starts_with($test))||+) {
      panic!("{:?} does not start with {:?}", string, [$($test),+]);
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

fn assert_stderr_for_inspect(
  stderr_lines: &mut impl std::iter::Iterator<Item = String>,
) {
  assert_stderr(
    stderr_lines,
    &["Visit chrome://inspect to connect to the debugger."],
  );
}

fn assert_stderr_for_inspect_brk(
  stderr_lines: &mut impl std::iter::Iterator<Item = String>,
) {
  assert_stderr(
    stderr_lines,
    &[
      "Visit chrome://inspect to connect to the debugger.",
      "Deno is waiting for debugger to connect.",
    ],
  );
}

async fn assert_inspector_messages(
  socket_tx: &mut SplitSink<
    tokio_tungstenite::WebSocketStream<
      tokio_tungstenite::MaybeTlsStream<TcpStream>,
    >,
    tungstenite::Message,
  >,
  messages: &[&str],
  socket_rx: &mut Pin<Box<dyn Stream<Item = String>>>,
  responses: &[&str],
  notifications: &[&str],
) {
  for msg in messages {
    socket_tx.send(msg.to_string().into()).await.unwrap();
  }

  let expected_messages = responses.len() + notifications.len();
  let mut responses_idx = 0;
  let mut notifications_idx = 0;

  for _ in 0..expected_messages {
    let msg = socket_rx.next().await.unwrap();

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
  let (_socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!("101 Switching Protocols", response.status().to_string());
  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_break_on_first_line() {
  let script = util::testdata_path().join("inspector/inspector2.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,
    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"Deno.core.print(\"hello from the inspector\\n\")","contextId":1,"includeCommandLineAPI":true,"silent":false,"returnByValue":true}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":4,"result":{"result":{"type":"undefined"}}}"#],
    &[],
  )
  .await;

  assert_eq!(&stdout_lines.next().unwrap(), "hello from the inspector");

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":5,"method":"Debugger.resume"}"#],
    &mut socket_rx,
    &[r#"{"id":5,"result":{}}"#],
    &[],
  )
  .await;

  assert_eq!(&stdout_lines.next().unwrap(), "hello from the script");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_pause() {
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
  let (mut socket, _) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");

  /// Returns the next websocket message as a string ignoring
  /// Debugger.scriptParsed messages.
  async fn ws_read_msg(
    socket: &mut tokio_tungstenite::WebSocketStream<
      tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
  ) -> String {
    use deno_core::futures::stream::StreamExt;
    while let Some(msg) = socket.next().await {
      let msg = msg.unwrap().to_string();
      // FIXME(bartlomieju): fails because there's a file loaded
      // called 150_errors.js
      // assert!(!msg.contains("error"));
      if !msg.contains("Debugger.scriptParsed") {
        return msg;
      }
    }
    unreachable!()
  }

  socket
    .send(r#"{"id":6,"method":"Debugger.enable"}"#.into())
    .await
    .unwrap();

  let msg = ws_read_msg(&mut socket).await;
  println!("response msg 1 {}", msg);
  assert_starts_with!(msg, r#"{"id":6,"result":{"debuggerId":"#);

  socket
    .send(r#"{"id":31,"method":"Debugger.pause"}"#.into())
    .await
    .unwrap();

  let msg = ws_read_msg(&mut socket).await;
  println!("response msg 2 {}", msg);
  assert_eq!(msg, r#"{"id":31,"result":{}}"#);

  child.kill().unwrap();
}

#[tokio::test]
async fn inspector_port_collision() {
  // Skip this test on WSL, which allows multiple processes to listen on the
  // same port, rather than making `bind()` fail with `EADDRINUSE`.
  if cfg!(target_os = "linux") && std::env::var_os("WSL_DISTRO_NAME").is_some()
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
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .env("NO_COLOR", "1")
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,
    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#
    ],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":4,"method":"Debugger.resume"}"#],
    &mut socket_rx,
    &[r#"{"id":4,"result":{}}"#],
    &[r#"{"method":"Debugger.resumed","params":{}}"#],
  )
  .await;

  for i in 0..128u32 {
    let request_id = i + 10;
    // Expect the number {i} on stdout.
    let s = i.to_string();
    assert_eq!(stdout_lines.next().unwrap(), s);

    assert_inspector_messages(
      &mut socket_tx,
      &[],
      &mut socket_rx,
      &[],
      &[
        r#"{"method":"Runtime.consoleAPICalled","#,
        r#"{"method":"Debugger.paused","#,
      ],
    )
    .await;

    assert_inspector_messages(
      &mut socket_tx,
      &[&format!(
        r#"{{"id":{},"method":"Debugger.resume"}}"#,
        request_id
      )],
      &mut socket_rx,
      &[&format!(r#"{{"id":{},"result":{{}}}}"#, request_id)],
      &[r#"{"method":"Debugger.resumed","params":{}}"#],
    )
    .await;
  }

  // Check that we can gracefully close the websocket connection.
  socket_tx.close().await.unwrap();
  socket_rx.for_each(|_| async {}).await;

  assert_eq!(&stdout_lines.next().unwrap(), "done");
  assert!(child.wait().unwrap().success());
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
  let mut child = util::deno_cmd()
    .arg("repl")
    .arg(inspect_flag_with_unique_port("--inspect"))
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines = std::io::BufReader::new(stderr)
    .lines()
    .map(|r| r.unwrap())
    .filter(|s| s.as_str() != "Debugger session started.");
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  let stdin = child.stdin.take().unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines = std::io::BufReader::new(stdout)
    .lines()
    .map(|r| r.unwrap())
    .filter(|s| !s.starts_with("Deno "));

  assert_stderr_for_inspect(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,
    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  assert_eq!(
    &stdout_lines.next().unwrap(),
    "exit using ctrl+d or close()"
  );

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":3,"method":"Runtime.compileScript","params":{"expression":"Deno.cwd()","sourceURL":"","persistScript":false,"executionContextId":1}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#], &[]
  ).await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"Deno.cwd()","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":4,"result":{"result":{"type":"string","value":""#],
    &[],
  ).await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":5,"method":"Runtime.evaluate","params":{"expression":"console.error('done');","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":5,"result":{"result":{"type":"undefined"}}}"#],
    &[r#"{"method":"Runtime.consoleAPICalled"#],
  ).await;

  assert_eq!(&stderr_lines.next().unwrap(), "done");

  drop(stdin);
  child.wait().unwrap();
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
  let mut child = util::deno_cmd()
    .arg("test")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,
    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"Deno.core.print(\"hello from the inspector\\n\")","contextId":1,"includeCommandLineAPI":true,"silent":false,"returnByValue":true}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":4,"result":{"result":{"type":"undefined"}}}"#],
    &[],
  )
  .await;

  assert_eq!(&stdout_lines.next().unwrap(), "hello from the inspector");

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":5,"method":"Debugger.resume"}"#],
    &mut socket_rx,
    &[r#"{"id":5,"result":{}}"#],
    &[],
  )
  .await;

  assert_starts_with!(&stdout_lines.next().unwrap(), "running 1 test from");
  assert!(&stdout_lines
    .next()
    .unwrap()
    .contains("test has finished running"));

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_with_ts_files() {
  let script = util::testdata_path().join("inspector/test.ts");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = (msg.starts_with(r#"{"method":"Debugger.scriptParsed","#)
        && msg.contains("testdata/inspector"))
        || !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,
    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  // receive messages with sources from this test
  let script1 = socket_rx.next().await.unwrap();
  assert!(script1.contains("testdata/inspector/test.ts"));
  let script1_id = {
    let v: serde_json::Value = serde_json::from_str(&script1).unwrap();
    v["params"]["scriptId"].as_str().unwrap().to_string()
  };
  let script2 = socket_rx.next().await.unwrap();
  assert!(script2.contains("testdata/inspector/foo.ts"));
  let script2_id = {
    let v: serde_json::Value = serde_json::from_str(&script2).unwrap();
    v["params"]["scriptId"].as_str().unwrap().to_string()
  };
  let script3 = socket_rx.next().await.unwrap();
  assert!(script3.contains("testdata/inspector/bar.js"));
  let script3_id = {
    let v: serde_json::Value = serde_json::from_str(&script3).unwrap();
    v["params"]["scriptId"].as_str().unwrap().to_string()
  };

  assert_inspector_messages(
    &mut socket_tx,
    &[],
    &mut socket_rx,
    &[r#"{"id":2,"result":{"debuggerId":"#],
    &[],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      &format!(r#"{{"id":4,"method":"Debugger.getScriptSource","params":{{"scriptId":"{}"}}}}"#, script1_id),
      &format!(r#"{{"id":5,"method":"Debugger.getScriptSource","params":{{"scriptId":"{}"}}}}"#, script2_id),
      &format!(r#"{{"id":6,"method":"Debugger.getScriptSource","params":{{"scriptId":"{}"}}}}"#, script3_id),
    ],
    &mut socket_rx,
    &[
      r#"{"id":4,"result":{"scriptSource":"import { foo } from \"./foo.ts\";\nimport { bar } from \"./bar.js\";\nconsole.log(foo());\nconsole.log(bar());\n//# sourceMappingURL=data:application/json;base64,"#,
      r#"{"id":5,"result":{"scriptSource":"class Foo {\n    hello() {\n        return \"hello\";\n    }\n}\nexport function foo() {\n    const f = new Foo();\n    return f.hello();\n}\n//# sourceMappingURL=data:application/json;base64,"#,
      r#"{"id":6,"result":{"scriptSource":"export function bar() {\n  return \"world\";\n}\n"#,
    ],
    &[],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[r#"{"id":7,"method":"Debugger.resume"}"#],
    &mut socket_rx,
    &[r#"{"id":7,"result":{}}"#],
    &[],
  )
  .await;

  assert_eq!(&stdout_lines.next().unwrap(), "hello");
  assert_eq!(&stdout_lines.next().unwrap(), "world");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_memory() {
  let script = util::testdata_path().join("inspector/memory.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,

    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#,
      r#"{"id":4,"method":"HeapProfiler.enable"}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#, r#"{"id":4,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  socket_tx
    .send(
      r#"{"id":5,"method":"Runtime.getHeapUsage", "params": {}}"#
        .to_string()
        .into(),
    )
    .await
    .unwrap();
  let msg = socket_rx.next().await.unwrap();
  let json_msg: serde_json::Value = serde_json::from_str(&msg).unwrap();
  assert_eq!(json_msg["id"].as_i64().unwrap(), 5);
  let result = &json_msg["result"];
  assert!(
    result["usedSize"].as_i64().unwrap()
      <= result["totalSize"].as_i64().unwrap()
  );

  socket_tx.send(
    r#"{"id":6,"method":"HeapProfiler.takeHeapSnapshot","params": {"reportProgress": true, "treatGlobalObjectsAsRoots": true, "captureNumberValue": false}}"#
      .to_string().into()
  ).await.unwrap();

  let mut progress_report_completed = false;
  loop {
    let msg = socket_rx.next().await.unwrap();

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

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_profile() {
  let script = util::testdata_path().join("inspector/memory.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx = socket_rx
    .map(|msg| msg.unwrap().to_string())
    .filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    })
    .boxed_local();

  assert_stderr_for_inspect_brk(&mut stderr_lines);

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":1,"method":"Runtime.enable"}"#,
      r#"{"id":2,"method":"Debugger.enable"}"#,

    ],
    &mut socket_rx,
    &[
      r#"{"id":1,"result":{}}"#,
      r#"{"id":2,"result":{"debuggerId":"#,
    ],
    &[
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#,
      r#"{"id":4,"method":"Profiler.enable"}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":3,"result":{}}"#, r#"{"id":4,"result":{}}"#],
    &[r#"{"method":"Debugger.paused","#],
  )
  .await;

  assert_inspector_messages(
    &mut socket_tx,
    &[
      r#"{"id":5,"method":"Profiler.setSamplingInterval","params":{"interval": 100}}"#,
      r#"{"id":6,"method":"Profiler.start","params":{}}"#,
    ],
    &mut socket_rx,
    &[r#"{"id":5,"result":{}}"#, r#"{"id":6,"result":{}}"#],
    &[],
  )
  .await;

  tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

  socket_tx
    .send(
      r#"{"id":7,"method":"Profiler.stop", "params": {}}"#.to_string().into(),
    )
    .await
    .unwrap();
  let msg = socket_rx.next().await.unwrap();
  let json_msg: serde_json::Value = serde_json::from_str(&msg).unwrap();
  assert_eq!(json_msg["id"].as_i64().unwrap(), 7);
  let result = &json_msg["result"];
  let profile = &result["profile"];
  assert!(
    profile["startTime"].as_i64().unwrap()
      < profile["endTime"].as_i64().unwrap()
  );
  profile["samples"].as_array().unwrap();
  profile["nodes"].as_array().unwrap();

  child.kill().unwrap();
  child.wait().unwrap();
}
