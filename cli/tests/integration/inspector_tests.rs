// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::futures;
use deno_core::futures::prelude::*;
use deno_core::serde_json;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_websocket::tokio_tungstenite;
use std::io::BufRead;
use test_util as util;

fn inspect_flag_with_unique_port(flag_prefix: &str) -> String {
  use std::sync::atomic::{AtomicU16, Ordering};
  static PORT: AtomicU16 = AtomicU16::new(9229);
  let port = PORT.fetch_add(1, Ordering::Relaxed);
  format!("{}=127.0.0.1:{}", flag_prefix, port)
}

fn extract_ws_url_from_stderr(
  stderr_lines: &mut impl std::iter::Iterator<Item = String>,
) -> url::Url {
  let stderr_first_line = stderr_lines.next().unwrap();
  assert!(stderr_first_line.starts_with("Debugger listening on "));
  let v: Vec<_> = stderr_first_line.match_indices("ws:").collect();
  assert_eq!(v.len(), 1);
  let ws_url_index = v[0].0;
  let ws_url = &stderr_first_line[ws_url_index..];
  url::Url::parse(ws_url).unwrap()
}

#[tokio::test]
async fn inspector_connect() {
  let script = util::testdata_path().join("inspector1.js");
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

#[derive(Debug)]
enum TestStep {
  StdOut(&'static str),
  StdErr(&'static str),
  WsRecv(&'static str),
  WsSend(&'static str),
}

#[tokio::test]
async fn inspector_break_on_first_line() {
  let script = util::testdata_path().join("inspector2.js");
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
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.paused","#),
    WsSend(
      r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"Deno.core.print(\"hello from the inspector\\n\")","contextId":1,"includeCommandLineAPI":true,"silent":false,"returnByValue":true}}"#,
    ),
    WsRecv(r#"{"id":4,"result":{"result":{"type":"undefined"}}}"#),
    StdOut("hello from the inspector"),
    WsSend(r#"{"id":5,"method":"Debugger.resume"}"#),
    WsRecv(r#"{"id":5,"result":{}}"#),
    StdOut("hello from the script"),
  ];

  for step in test_steps {
    match step {
      StdOut(s) => assert_eq!(&stdout_lines.next().unwrap(), s),
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
      _ => unreachable!(),
    }
  }

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_pause() {
  let script = util::testdata_path().join("inspector1.js");
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
  assert!(msg.starts_with(r#"{"id":6,"result":{"debuggerId":"#));

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

  let script = util::testdata_path().join("inspector1.js");
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
  let script = util::testdata_path().join("inspector3.js");
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
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.paused","#),
    WsSend(r#"{"id":4,"method":"Debugger.resume"}"#),
    WsRecv(r#"{"id":4,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.resumed","params":{}}"#),
  ];

  for step in test_steps {
    match step {
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
      _ => unreachable!(),
    }
  }

  for i in 0..128u32 {
    let request_id = i + 10;
    // Expect the number {i} on stdout.
    let s = i.to_string();
    assert_eq!(stdout_lines.next().unwrap(), s);
    // Expect console.log
    let s = r#"{"method":"Runtime.consoleAPICalled","#;
    assert!(socket_rx.next().await.unwrap().starts_with(s));
    // Expect hitting the `debugger` statement.
    let s = r#"{"method":"Debugger.paused","#;
    assert!(socket_rx.next().await.unwrap().starts_with(s));
    // Send the 'Debugger.resume' request.
    let s = format!(r#"{{"id":{},"method":"Debugger.resume"}}"#, request_id);
    socket_tx.send(s.into()).await.unwrap();
    // Expect confirmation of the 'Debugger.resume' request.
    let s = format!(r#"{{"id":{},"result":{{}}}}"#, request_id);
    assert_eq!(socket_rx.next().await.unwrap(), s);
    let s = r#"{"method":"Debugger.resumed","params":{}}"#;
    assert_eq!(socket_rx.next().await.unwrap(), s);
  }

  // Check that we can gracefully close the websocket connection.
  socket_tx.close().await.unwrap();
  socket_rx.for_each(|_| async {}).await;

  assert_eq!(&stdout_lines.next().unwrap(), "done");
  assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn inspector_without_brk_runs_code() {
  let script = util::testdata_path().join("inspector4.js");
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
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdin = child.stdin.take().unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines = std::io::BufReader::new(stdout)
    .lines()
    .map(|r| r.unwrap())
    .filter(|s| !s.starts_with("Deno "));

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    StdOut("exit using ctrl+d or close()"),
    WsSend(
      r#"{"id":4,"method":"Runtime.compileScript","params":{"expression":"Deno.cwd()","sourceURL":"","persistScript":false,"executionContextId":1}}"#,
    ),
    WsRecv(r#"{"id":4,"result":{}}"#),
    WsSend(
      r#"{"id":5,"method":"Runtime.evaluate","params":{"expression":"Deno.cwd()","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ),
    WsRecv(r#"{"id":5,"result":{"result":{"type":"string","value":""#),
    WsSend(
      r#"{"id":6,"method":"Runtime.evaluate","params":{"expression":"console.error('done');","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ),
    WsRecv(r#"{"method":"Runtime.consoleAPICalled"#),
    WsRecv(r#"{"id":6,"result":{"result":{"type":"undefined"}}}"#),
    StdErr("done"),
  ];

  for step in test_steps {
    match step {
      StdOut(s) => assert_eq!(&stdout_lines.next().unwrap(), s),
      StdErr(s) => assert_eq!(&stderr_lines.next().unwrap(), s),
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
    }
  }

  drop(stdin);
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_json() {
  let script = util::testdata_path().join("inspector1.js");
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
  let script = util::testdata_path().join("inspector1.js");
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
  let script = util::testdata_path().join("inspector1.js");
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
