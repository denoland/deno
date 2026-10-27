// Copyright 2018-2026 the Deno authors. MIT license.

use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use test_util::DenoChild;
use test_util::TestContext;
use test_util::TestContextBuilder;
use test_util::assertions::assert_json_subset;
use test_util::eprintln;
use test_util::test;
use tokio::sync::Mutex;
use tokio::time::timeout;
use uuid::Uuid;

use crate::jupyter_client::DealerSocket;
use crate::jupyter_client::ReqSocket;
use crate::jupyter_client::SubSocket;

/// Jupyter connection file format
#[derive(Serialize)]
struct ConnectionSpec {
  key: String,
  signature_scheme: String,
  transport: String,
  ip: String,
  hb_port: u16,
  control_port: u16,
  shell_port: u16,
  stdin_port: u16,
  iopub_port: u16,
  kernel_name: String,
}

impl ConnectionSpec {
  fn endpoint(&self, port: u16) -> String {
    format!("{}:{}", self.ip, port)
  }
}

fn pick_unused_port() -> (u16, std::net::TcpListener) {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
  (listener.local_addr().unwrap().port(), listener)
}

impl ConnectionSpec {
  fn new() -> (Self, Vec<std::net::TcpListener>) {
    let mut listeners = Vec::new();
    let (hb_port, l) = pick_unused_port();
    listeners.push(l);
    let (control_port, l) = pick_unused_port();
    listeners.push(l);
    let (shell_port, l) = pick_unused_port();
    listeners.push(l);
    let (stdin_port, l) = pick_unused_port();
    listeners.push(l);
    let (iopub_port, l) = pick_unused_port();
    listeners.push(l);

    (
      Self {
        key: "".into(),
        signature_scheme: "hmac-sha256".into(),
        transport: "tcp".into(),
        ip: "127.0.0.1".into(),
        hb_port,
        control_port,
        shell_port,
        stdin_port,
        iopub_port,
        kernel_name: "deno".into(),
      },
      listeners,
    )
  }
}

const DELIMITER: &[u8] = b"<IDS|MSG>";

#[derive(Debug, Clone)]
struct JupyterMsg {
  routing_prefix: Vec<Bytes>,
  signature: String,
  header: MsgHeader,
  parent_header: Value,
  metadata: Value,
  content: Value,
  buffers: Vec<Bytes>,
}

impl Default for JupyterMsg {
  fn default() -> Self {
    Self {
      routing_prefix: vec![Bytes::from(Uuid::new_v4().to_string())],
      signature: "".into(),
      header: MsgHeader::default(),
      parent_header: json!({}),
      metadata: json!({}),
      content: json!({}),
      buffers: Vec::new(),
    }
  }
}

#[derive(Serialize, Clone, Debug, Deserialize)]
struct MsgHeader {
  msg_id: Uuid,
  session: Uuid,
  date: DateTime<Utc>,
  username: String,
  msg_type: String,
  version: String,
}

impl MsgHeader {
  fn to_json(&self) -> Value {
    serde_json::to_value(self).unwrap()
  }
}

impl Default for MsgHeader {
  fn default() -> Self {
    Self {
      msg_id: Uuid::new_v4(),
      session: Uuid::new_v4(),
      date: chrono::Utc::now(),
      username: "test".into(),
      msg_type: "kernel_info_request".into(),
      version: "5.3".into(),
    }
  }
}

impl JupyterMsg {
  fn to_frames(&self) -> Vec<Bytes> {
    let mut frames: Vec<Bytes> = Vec::new();
    frames.extend(self.routing_prefix.clone());
    frames.push(Bytes::from_static(DELIMITER));
    frames.push(Bytes::from(self.signature.clone()));
    frames.push(Bytes::from(serde_json::to_vec(&self.header).unwrap()));
    frames.push(Bytes::from(self.parent_header.to_string()));
    frames.push(Bytes::from(self.metadata.to_string()));
    frames.push(Bytes::from(self.content.to_string()));
    frames.extend(self.buffers.clone());
    frames
  }

  fn from_frames(frames: Vec<Bytes>) -> Self {
    let delim_pos = frames
      .iter()
      .position(|f| f.as_ref() == DELIMITER)
      .unwrap_or(0);

    let routing_prefix = frames[..delim_pos].to_vec();
    let signature =
      String::from_utf8(frames[delim_pos + 1].to_vec()).unwrap_or_default();
    let header: MsgHeader =
      serde_json::from_slice(&frames[delim_pos + 2]).unwrap_or_default();
    let parent_header: Value =
      serde_json::from_slice(&frames[delim_pos + 3]).unwrap_or(json!({}));
    let metadata: Value =
      serde_json::from_slice(&frames[delim_pos + 4]).unwrap_or(json!({}));
    let content: Value =
      serde_json::from_slice(&frames[delim_pos + 5]).unwrap_or(json!({}));
    let buffers = frames[delim_pos + 6..].to_vec();

    Self {
      routing_prefix,
      signature,
      header,
      parent_header,
      metadata,
      content,
      buffers,
    }
  }

  fn new(session: Uuid, msg_type: impl AsRef<str>, content: Value) -> Self {
    Self {
      header: MsgHeader {
        session,
        msg_type: msg_type.as_ref().into(),
        ..Default::default()
      },
      content,
      ..Default::default()
    }
  }

  // Computes the HMAC-SHA256 signature over the four signed frames, exactly as
  // `to_frames` serializes them, and stores it as a lowercase hex string.
  fn sign(&mut self, key: &str) {
    use hmac::Hmac;
    use hmac::Mac;
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(&serde_json::to_vec(&self.header).unwrap());
    mac.update(self.parent_header.to_string().as_bytes());
    mac.update(self.metadata.to_string().as_bytes());
    mac.update(self.content.to_string().as_bytes());
    let digest = mac.finalize().into_bytes();
    self.signature = digest.iter().map(|b| format!("{b:02x}")).collect();
  }
}

#[derive(Clone)]
struct JupyterClient {
  recv_timeout: Duration,
  session: Uuid,
  heartbeat: Arc<Mutex<ReqSocket>>,
  control: Arc<Mutex<DealerSocket>>,
  shell: Arc<Mutex<DealerSocket>>,
  io_pub: Arc<Mutex<SubSocket>>,
  stdin: Arc<Mutex<DealerSocket>>,
}

#[derive(Debug, Clone, Copy)]
enum JupyterChannel {
  Control,
  Shell,
  Stdin,
  IoPub,
}

use JupyterChannel::*;

impl JupyterClient {
  async fn new(spec: &ConnectionSpec) -> Self {
    Self::new_with_timeout(spec, Duration::from_secs(10)).await
  }

  async fn new_with_timeout(spec: &ConnectionSpec, t: Duration) -> Self {
    let hb_addr = spec.endpoint(spec.hb_port);
    let ctrl_addr = spec.endpoint(spec.control_port);
    let shell_addr = spec.endpoint(spec.shell_port);
    let iopub_addr = spec.endpoint(spec.iopub_port);
    let stdin_addr = spec.endpoint(spec.stdin_port);

    let (heartbeat, control, shell, io_pub, stdin) = tokio::join!(
      async { ReqSocket::connect(&hb_addr).await.unwrap() },
      async { DealerSocket::connect(&ctrl_addr).await.unwrap() },
      async { DealerSocket::connect(&shell_addr).await.unwrap() },
      async { SubSocket::connect(&iopub_addr).await.unwrap() },
      async { DealerSocket::connect(&stdin_addr).await.unwrap() },
    );

    Self {
      session: Uuid::new_v4(),
      heartbeat: Arc::new(Mutex::new(heartbeat)),
      control: Arc::new(Mutex::new(control)),
      shell: Arc::new(Mutex::new(shell)),
      io_pub: Arc::new(Mutex::new(io_pub)),
      stdin: Arc::new(Mutex::new(stdin)),
      recv_timeout: t,
    }
  }

  async fn io_subscribe(&self, topic: &str) -> Result<()> {
    self.io_pub.lock().await.subscribe(topic).await?;
    Ok(())
  }

  async fn send_heartbeat(&self, data: &[u8]) -> Result<()> {
    self
      .heartbeat
      .lock()
      .await
      .send(Bytes::copy_from_slice(data))
      .await?;
    Ok(())
  }

  async fn recv_heartbeat(&self) -> Result<Bytes> {
    let mut hb = self.heartbeat.lock().await;
    let data = timeout(self.recv_timeout, hb.recv()).await??;
    Ok(data)
  }

  async fn send(
    &self,
    channel: JupyterChannel,
    msg_type: &str,
    content: Value,
  ) -> Result<JupyterMsg> {
    let msg = JupyterMsg::new(self.session, msg_type, content);
    self.send_msg(channel, &msg).await?;
    Ok(msg)
  }

  // Sends a pre-built message verbatim, so tests can control the signature.
  async fn send_msg(
    &self,
    channel: JupyterChannel,
    msg: &JupyterMsg,
  ) -> Result<()> {
    let bytes_frames: Vec<Bytes> = msg.to_frames();
    match channel {
      Control => {
        self
          .control
          .lock()
          .await
          .send_multipart(&bytes_frames)
          .await?;
      }
      Shell => {
        self
          .shell
          .lock()
          .await
          .send_multipart(&bytes_frames)
          .await?;
      }
      Stdin => {
        self
          .stdin
          .lock()
          .await
          .send_multipart(&bytes_frames)
          .await?;
      }
      IoPub => panic!("Cannot send over IOPub"),
    }
    Ok(())
  }

  async fn recv(&self, channel: JupyterChannel) -> Result<JupyterMsg> {
    let frames = match channel {
      Control => {
        let mut ctrl = self.control.lock().await;
        timeout(self.recv_timeout, ctrl.recv_multipart()).await??
      }
      Shell => {
        let mut sh = self.shell.lock().await;
        timeout(self.recv_timeout, sh.recv_multipart()).await??
      }
      Stdin => {
        let mut stdin = self.stdin.lock().await;
        timeout(self.recv_timeout, stdin.recv_multipart()).await??
      }
      IoPub => {
        let mut iopub = self.io_pub.lock().await;
        timeout(self.recv_timeout, iopub.recv_multipart()).await??
      }
    };
    Ok(JupyterMsg::from_frames(frames))
  }
}

async fn wait_or_kill(
  mut process: DenoChild,
  wait: Duration,
) -> Result<Output> {
  let start = std::time::Instant::now();
  while start.elapsed() < wait {
    if process.try_wait()?.is_some() {
      return Ok(process.wait_with_output()?);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  process.kill()?;
  Ok(process.wait_with_output()?)
}

struct JupyterServerProcess(Option<DenoChild>);

impl JupyterServerProcess {
  // Wait for the process to exit, or kill it after the given duration.
  #[allow(dead_code, reason = "used in some tests")]
  async fn wait_or_kill(mut self, wait: Duration) -> Output {
    wait_or_kill(self.0.take().unwrap(), wait).await.unwrap()
  }
}

impl Drop for JupyterServerProcess {
  fn drop(&mut self) {
    let Some(mut proc) = self.0.take() else {
      return;
    };
    if proc.try_wait().unwrap().is_some() {
      return;
    }
    proc.kill().unwrap();
  }
}

async fn server_ready_on(addr: &str) -> bool {
  matches!(
    timeout(
      Duration::from_millis(1000),
      tokio::net::TcpStream::connect(addr),
    )
    .await,
    Ok(Ok(_))
  )
}

async fn server_ready(conn: &ConnectionSpec) -> bool {
  let hb = conn.endpoint(conn.hb_port);
  let control = conn.endpoint(conn.control_port);
  let shell = conn.endpoint(conn.shell_port);
  let stdin = conn.endpoint(conn.stdin_port);
  let iopub = conn.endpoint(conn.iopub_port);
  let (a, b, c, d, e) = tokio::join!(
    server_ready_on(&hb),
    server_ready_on(&control),
    server_ready_on(&shell),
    server_ready_on(&stdin),
    server_ready_on(&iopub),
  );
  a && b && c && d && e
}

async fn setup_server() -> (TestContext, ConnectionSpec, JupyterServerProcess) {
  setup_server_with_key("").await
}

async fn setup_server_with_key(
  key: &str,
) -> (TestContext, ConnectionSpec, JupyterServerProcess) {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let (mut conn, mut listeners) = ConnectionSpec::new();
  conn.key = key.into();
  let conn_file = context.temp_dir().path().join("connection.json");
  conn_file.write_json(&conn);

  let start_process = |conn_file: &test_util::PathRef| {
    context
      .new_command()
      .args_vec(vec![
        "jupyter",
        "--kernel",
        "--conn",
        conn_file.to_string().as_str(),
      ])
      .spawn()
      .unwrap()
  };

  drop(listeners);

  let mut process = start_process(&conn_file);

  'outer: for i in 0..10 {
    for _ in 0..10 {
      if process.try_wait().unwrap().is_none() {
        if server_ready(&conn).await {
          break 'outer;
        }
      } else {
        break;
      }
      tokio::time::sleep(Duration::from_millis(500)).await;
    }

    (conn, listeners) = ConnectionSpec::new();
    conn.key = key.into();
    conn_file.write_json(&conn);
    drop(listeners);
    process = start_process(&conn_file);
    tokio::time::sleep(Duration::from_millis((i + 1) * 250)).await;
  }
  if process.try_wait().unwrap().is_some() || !server_ready(&conn).await {
    panic!("Failed to start Jupyter server");
  }
  (context, conn, JupyterServerProcess(Some(process)))
}

async fn setup() -> (TestContext, JupyterClient, JupyterServerProcess) {
  let (context, conn, process) = setup_server().await;
  let client = JupyterClient::new(&conn).await;
  client.io_subscribe("").await.unwrap();
  // Prime heartbeat
  client.send_heartbeat(b"ping").await.unwrap();
  let _ = client.recv_heartbeat().await.unwrap();

  (context, client, process)
}

#[test]
async fn jupyter_heartbeat_echoes() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client.send_heartbeat(b"ping").await?;
  let msg = client.recv_heartbeat().await?;
  // The kernel echoes back the exact bytes sent
  assert_eq!(msg, Bytes::from_static(b"ping"));

  Ok(())
}

#[test]
async fn jupyter_kernel_info() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(Control, "kernel_info_request", json!({}))
    .await?;
  let msg = client.recv(Control).await?;
  assert_eq!(msg.header.msg_type, "kernel_info_reply");
  assert_json_subset(
    msg.content,
    json!({
      "status": "ok",
      "implementation": "Deno kernel",
      "language_info": {
        "name": "typescript",
        "mimetype": "text/x.typescript",
        "file_extension": ".ts",
        "pygments_lexer": "typescript",
        "codemirror_mode": "typescript",
        "nbconvert_exporter": "script"
      },
    }),
  );

  Ok(())
}

#[test]
async fn jupyter_execute_request() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  let request = client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "console.log(\"asdf\")"
      }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content,
    json!({
      "status": "ok",
      "execution_count": 1,
    }),
  );

  let mut msgs = Vec::new();

  for _ in 0..4 {
    match client.recv(IoPub).await {
      Ok(msg) => msgs.push(msg),
      Err(e) => {
        if e.downcast_ref::<tokio::time::error::Elapsed>().is_some() {
          eprintln!("Timed out waiting for messages");
        }
        panic!("Error: {:#?}", e);
      }
    }
  }

  let execution_idle = msgs
    .iter()
    .find(|msg| {
      msg
        .content
        .get("execution_state")
        .map(|s| s == "idle")
        .unwrap_or(false)
    })
    .expect("execution_state idle not found");
  assert_eq!(execution_idle.parent_header, request.header.to_json());
  assert_json_subset(
    execution_idle.content.clone(),
    json!({ "execution_state": "idle" }),
  );

  let execution_result = msgs
    .iter()
    .find(|msg| msg.header.msg_type == "stream")
    .expect("stream not found");
  assert_eq!(execution_result.header.msg_type, "stream");
  assert_eq!(execution_result.parent_header, request.header.to_json());
  assert_json_subset(
    execution_result.content.clone(),
    json!({
      "name": "stdout",
      "text": "asdf\n",
    }),
  );

  Ok(())
}

// Regression test for denoland/deno#35290: a cell that throws must report the
// error. The JS-kernel rewrite (#34083) silently dropped exceptions, so errors
// became `status: "ok"` with no broadcast. The source-map work routes every
// thrown exception through `JupyterEvaluateOutcome.error` (a successful
// evaluation's cdp response lives under `.value`), so throws are always
// reported here.
#[test]
async fn jupyter_execute_error_reports_traceback() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  let request = client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "throw new Error(\"boom\")"
      }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content.clone(),
    json!({
      "status": "error",
      "execution_count": 1,
      "ename": "Error",
      "evalue": "boom",
    }),
  );
  let traceback = reply
    .content
    .get("traceback")
    .and_then(|t| t.as_array())
    .expect("traceback array");
  assert!(
    traceback
      .iter()
      .any(|line| line.as_str().map(|s| s.contains("boom")).unwrap_or(false)),
    "traceback should mention the error: {traceback:?}",
  );

  let mut msgs = Vec::new();
  for _ in 0..4 {
    match client.recv(IoPub).await {
      Ok(msg) => msgs.push(msg),
      Err(e) => {
        if e.downcast_ref::<tokio::time::error::Elapsed>().is_some() {
          eprintln!("Timed out waiting for messages");
        }
        panic!("Error: {:#?}", e);
      }
    }
  }

  let error_msg = msgs
    .iter()
    .find(|msg| msg.header.msg_type == "error")
    .expect("error message not broadcast on iopub");
  assert_eq!(error_msg.parent_header, request.header.to_json());
  assert_json_subset(
    error_msg.content.clone(),
    json!({
      "ename": "Error",
      "evalue": "boom",
    }),
  );

  Ok(())
}

// Reproduction for the VS Code "kernel died" hang when a cell calls `prompt()`:
// the kernel must send an `input_request` on the stdin channel, accept the
// `input_reply`, and complete the cell.
#[test]
async fn jupyter_execute_prompt_stdin() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  let request = client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "const name = prompt(\"Name?\"); console.log(`Hello, ${name}!`)"
      }),
    )
    .await?;

  // The kernel should ask the frontend for input on the stdin channel.
  let input_req = client.recv(Stdin).await?;
  assert_eq!(input_req.header.msg_type, "input_request");
  assert_json_subset(
    input_req.content.clone(),
    json!({ "prompt": "Name?", "password": false }),
  );
  assert_eq!(input_req.parent_header, request.header.to_json());

  // Reply with a value.
  client
    .send(
      Stdin,
      "input_reply",
      json!({ "value": "World", "status": "ok" }),
    )
    .await?;

  // The cell should now finish successfully.
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content,
    json!({ "status": "ok", "execution_count": 1 }),
  );

  // And it should have printed the answer.
  let mut printed = None;
  for _ in 0..6 {
    let msg = client.recv(IoPub).await?;
    if msg.header.msg_type == "stream" {
      printed = msg
        .content
        .get("text")
        .and_then(|t| t.as_str().map(String::from));
      if printed.is_some() {
        break;
      }
    }
  }
  assert_eq!(printed.as_deref(), Some("Hello, World!\n"));

  Ok(())
}

// While a `prompt()` is waiting for an `input_reply`, the kernel must stay
// responsive: the heartbeat must keep echoing (otherwise a frontend like VS Code
// declares the kernel dead and tears down the sockets, which is the "kernel
// died" + "Socket is closed EBADF" symptom).
#[test]
async fn jupyter_prompt_does_not_block_heartbeat() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "const name = prompt(\"Name?\"); name"
      }),
    )
    .await?;

  // Kernel asks for input.
  let input_req = client.recv(Stdin).await?;
  assert_eq!(input_req.header.msg_type, "input_request");

  // Now simulate a user who takes a while to type: the kernel must keep
  // answering heartbeats during this window.
  for i in 0..5 {
    tokio::time::sleep(Duration::from_millis(400)).await;
    client.send_heartbeat(format!("ping{i}").as_bytes()).await?;
    let pong = client.recv_heartbeat().await?;
    assert_eq!(
      pong,
      Bytes::from(format!("ping{i}")),
      "heartbeat stalled while a prompt was open (iteration {i})",
    );
  }

  // Finally answer and let the cell finish.
  client
    .send(
      Stdin,
      "input_reply",
      json!({ "value": "World", "status": "ok" }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  Ok(())
}

// The real reproduction of the VS Code "kernel died" hang: with a connection
// KEY configured (every frontend does this), a `prompt()` must complete. If the
// kernel can't verify the HMAC of the `input_reply`, `requestInput` loops
// forever and the cell never finishes.
#[test]
async fn jupyter_prompt_hmac_signed() -> Result<()> {
  const KEY: &str = "super-secret-connection-key";
  let (_ctx, conn, _process) = setup_server_with_key(KEY).await;
  let client = JupyterClient::new(&conn).await;
  client.io_subscribe("").await?;
  client.send_heartbeat(b"ping").await?;
  let _ = client.recv_heartbeat().await?;

  let mut req = JupyterMsg::new(
    client.session,
    "execute_request",
    json!({
      "silent": false,
      "store_history": true,
      "user_expressions": {},
      "allow_stdin": true,
      "stop_on_error": false,
      "code": "const name = prompt(\"Name?\"); name"
    }),
  );
  req.sign(KEY);
  client.send_msg(Shell, &req).await?;

  let input_req = client.recv(Stdin).await?;
  assert_eq!(input_req.header.msg_type, "input_request");

  let mut input_reply = JupyterMsg::new(
    client.session,
    "input_reply",
    json!({ "value": "World", "status": "ok" }),
  );
  input_reply.sign(KEY);
  client.send_msg(Stdin, &input_reply).await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  Ok(())
}

// Regression test for denoland/deno#22771: a `complete_request` whose code
// contains multi-byte (e.g. Korean) characters must not crash the kernel. The
// old Rust kernel sliced the cell text using the codepoint-based `cursor_pos`
// as a byte index and panicked ("byte index N is not a char boundary").
#[test]
async fn jupyter_complete_request_multibyte() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  // The exact shape from the issue: a Korean character earlier in the cell and
  // an identifier prefix at the very end where the cursor sits.
  let code = "function getLunchPlace(me) {\n  '서';\n}\n\nge";
  let cursor_pos = code.chars().count() as i64;
  client
    .send(
      Shell,
      "complete_request",
      json!({ "code": code, "cursor_pos": cursor_pos }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "complete_reply");
  // Cursor positions are reported in codepoints, not bytes/UTF-16 units.
  assert_json_subset(
    reply.content.clone(),
    json!({
      "status": "ok",
      "cursor_end": cursor_pos,
    }),
  );

  Ok(())
}

// Regression test for cursor_pos handling with astral (non-BMP) characters.
// `cursor_pos` is measured in Unicode codepoints, while JS strings are indexed
// by UTF-16 code units, so the kernel must translate between the two for both
// slicing the completion target and reporting `cursor_start`/`cursor_end`.
#[test]
async fn jupyter_complete_request_astral_cursor() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  // `"😀"; cons` — the emoji is a single codepoint but two UTF-16 units.
  let code = "\"😀\"; cons";
  let cursor_pos = code.chars().count() as i64; // 9 codepoints
  client
    .send(
      Shell,
      "complete_request",
      json!({ "code": code, "cursor_pos": cursor_pos }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "complete_reply");
  // The completion target is "cons", which starts at codepoint offset 5
  // (the emoji counts as one codepoint, not two). Before the fix the kernel
  // sliced with the codepoint cursor as a UTF-16 index and reported
  // cursor_start == 6.
  assert_json_subset(
    reply.content.clone(),
    json!({
      "status": "ok",
      "cursor_start": 5,
      "cursor_end": cursor_pos,
    }),
  );

  Ok(())
}

// Regression test for the HMAC signature verification of incoming messages.
// When a connection key is configured, the kernel must reject `execute_request`s
// whose signature doesn't verify (otherwise any local process able to reach the
// kernel's TCP ports could run arbitrary code), and accept correctly-signed
// ones.
#[test]
async fn jupyter_rejects_invalid_hmac_signature() -> Result<()> {
  const KEY: &str = "super-secret-connection-key";
  let (_ctx, conn, _process) = setup_server_with_key(KEY).await;
  let client = JupyterClient::new(&conn).await;
  client.io_subscribe("").await?;
  client.send_heartbeat(b"ping").await?;
  let _ = client.recv_heartbeat().await?;

  let content = json!({
    "silent": false,
    "store_history": false,
    "code": "1 + 1",
  });

  // A request carrying a bogus signature (the attacker doesn't know the key)
  // must be dropped: the kernel should not send back any execute_reply.
  let mut forged =
    JupyterMsg::new(client.session, "execute_request", content.clone());
  forged.signature = "00".repeat(32); // valid hex, wrong HMAC
  client.send_msg(Shell, &forged).await?;
  let dropped = timeout(Duration::from_secs(3), client.recv(Shell)).await;
  assert!(
    dropped.is_err(),
    "kernel must not reply to a request with an invalid HMAC signature, got: {dropped:#?}"
  );

  // A correctly-signed request from a peer that knows the key is processed.
  let mut signed = JupyterMsg::new(client.session, "execute_request", content);
  signed.sign(KEY);
  client.send_msg(Shell, &signed).await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  Ok(())
}

#[test]
async fn jupyter_store_history_false() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": false,
        "code": "console.log(\"asdf\")",
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content,
    json!({
      "status": "ok",
      "execution_count": 0,
    }),
  );

  Ok(())
}

#[test]
async fn jupyter_shutdown_reply() -> Result<()> {
  let (_ctx, client, process) = setup().await;
  client
    .send(Control, "shutdown_request", json!({ "restart": false }))
    .await?;
  let msg = client.recv(Control).await?;
  assert_eq!(msg.header.msg_type, "shutdown_reply");
  assert_json_subset(msg.content, json!({ "status": "ok", "restart": false }));

  let output = process.wait_or_kill(Duration::from_secs(5)).await;
  assert!(output.status.success());

  Ok(())
}

#[test]
async fn jupyter_shutdown_restart_reply() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(Control, "shutdown_request", json!({ "restart": true }))
    .await?;
  let msg = client.recv(Control).await?;
  assert_eq!(msg.header.msg_type, "shutdown_reply");
  assert_json_subset(msg.content, json!({ "status": "ok", "restart": true }));

  Ok(())
}

#[test]
async fn jupyter_interrupt_reply() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client.send(Control, "interrupt_request", json!({})).await?;
  let msg = client.recv(Control).await?;
  assert_eq!(msg.header.msg_type, "interrupt_reply");
  assert_json_subset(msg.content, json!({ "status": "ok" }));

  Ok(())
}

#[test]
async fn jupyter_interrupt_running_code() -> Result<()> {
  let (_ctx, client, _process) = setup().await;

  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "code": "while (true) {}",
      }),
    )
    .await?;

  tokio::time::sleep(Duration::from_millis(100)).await;

  client.send(Control, "interrupt_request", json!({})).await?;
  let interrupt_reply = client.recv(Control).await?;
  assert_eq!(interrupt_reply.header.msg_type, "interrupt_reply");

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");

  // Kernel should still be alive after interrupt
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "code": "42",
      }),
    )
    .await?;
  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  Ok(())
}

// Regression test for https://github.com/denoland/deno/issues/26816
#[test]
async fn jupyter_relative_import() -> Result<()> {
  let (ctx, client, _process) = setup().await;
  ctx
    .temp_dir()
    .path()
    .join("data.ts")
    .write("export function getData() { return 42; }\n");

  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "code":
          "import { getData } from './data.ts';\nconsole.log(getData());",
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content,
    json!({
      "status": "ok",
    }),
  );

  // confirm the imported function actually ran by watching for the stdout
  // stream on iopub
  let mut saw_stdout = false;
  for _ in 0..6 {
    let Ok(msg) = client.recv(IoPub).await else {
      break;
    };
    if msg.header.msg_type == "stream"
      && msg.content.get("name").and_then(|v| v.as_str()) == Some("stdout")
      && msg
        .content
        .get("text")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.contains("42"))
    {
      saw_stdout = true;
      break;
    }
  }
  assert!(saw_stdout, "expected stdout output containing 42");

  Ok(())
}

#[test]
async fn jupyter_http_server() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": false,
        "code": r#"Deno.serve({ port: 10234 }, (req) => Response.json({ hello: "world" }))"#,
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(
    reply.content,
    json!({ "status": "ok", "execution_count": 0 }),
  );

  for _ in 0..3 {
    let resp = reqwest::get("http://localhost:10234").await.unwrap();
    let text: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(text, json!({ "hello": "world" }));
  }

  Ok(())
}

#[test]
async fn jupyter_stdin_prompt_reply() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "prompt(\"name?\")",
      }),
    )
    .await?;

  // Kernel sends input_request on stdin; reply with input_reply.
  let req = client.recv(Stdin).await?;
  assert_eq!(req.header.msg_type, "input_request");
  assert_json_subset(
    req.content.clone(),
    json!({ "prompt": "name?", "password": false }),
  );

  client
    .send(Stdin, "input_reply", json!({ "value": "deno" }))
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  Ok(())
}

// Regression for the kernel-rewrite (#34083) that this PR also patches:
// `cli/js/jupyter_kernel.js` reads `evalResult?.value?.result` to publish
// an `execute_result` iopub message for cells whose final expression
// produces a value, but the Rust side was sending the
// `cdp::EvaluateResponse` flat (no `value` wrapper), so every successful
// expression cell silently went without an `execute_result` broadcast.
// The new `JupyterEvaluateOutcome { value, .. }` shape makes that path
// load-bearing; this test pins it down so future refactors don't drop
// `execute_result` again.
#[test]
async fn jupyter_execute_result_broadcast_for_expression() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": "1+1",
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  // Drain iopub looking for the execute_result for our `1+1` cell. We
  // skip `status`/`stream`/etc. and stop at the `idle` busy marker so
  // we don't hang waiting for messages that never come.
  let mut execute_result = None;
  for _ in 0..8 {
    let Ok(msg) = client.recv(IoPub).await else {
      break;
    };
    if msg.header.msg_type == "execute_result" {
      execute_result = Some(msg);
      break;
    }
    if msg.header.msg_type == "status"
      && msg.content.get("execution_state").and_then(|v| v.as_str())
        == Some("idle")
    {
      break;
    }
  }

  let msg =
    execute_result.expect("expected an execute_result iopub message for `1+1`");
  let data = msg
    .content
    .get("data")
    .and_then(|v| v.as_object())
    .cloned()
    .unwrap_or_default();
  let text_plain = data
    .get("text/plain")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  assert!(
    text_plain.contains('2'),
    "execute_result text/plain should contain `2`: {data:?}",
  );

  Ok(())
}

// Regression for denoland/deno#20643: errors thrown from a Jupyter cell
// must arrive in the `traceback` with line/column numbers that match the
// user's TypeScript, not the transpiled JavaScript. The bug originally
// reported a four-line cell whose `throw new Error("fail")` on line 4
// surfaced as `<anonymous>:5:7` (SWC adds a `"use strict";` line in front
// of ESM module output, shifting every line by one).
#[test]
async fn jupyter_execute_error_source_map_remaps_line() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  let code = "console.log(\"1\");\n\
              console.log(\"2\");\n\
              console.log(\"3\");\n\
              throw new Error(\"fail\");\n";
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": code,
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  let status = reply
    .content
    .get("status")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  assert_eq!(
    status, "error",
    "expected status=error: {:?}",
    reply.content
  );

  let ename = reply
    .content
    .get("ename")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  let evalue = reply
    .content
    .get("evalue")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  let traceback = reply
    .content
    .get("traceback")
    .and_then(|v| v.as_array())
    .cloned()
    .unwrap_or_default();
  assert_eq!(ename, "Error");
  assert_eq!(evalue, "fail");
  let joined = traceback
    .iter()
    .filter_map(|v| v.as_str())
    .collect::<Vec<_>>()
    .join("\n");
  assert!(
    joined.contains(":4:"),
    "traceback should point at the user's line 4: {joined:?}",
  );
  assert!(
    !joined.contains(":5:"),
    "traceback should not surface the transpiled line 5: {joined:?}",
  );
  // The issue also asked for the user's source line to appear beneath
  // the frame (Python/IPython-style), since Jupyter cells don't show
  // line numbers by default.
  assert!(
    joined.contains("throw new Error(\"fail\")"),
    "traceback should echo the user's source line: {joined:?}",
  );

  Ok(())
}

// Regression for denoland/deno#20643: the same source-map fix must also
// shift line numbers across SWC's parameter-property transform (which
// expands `constructor(public x: T)` into an explicit assignment in the
// constructor body, growing the class body by several lines). Without the
// fix, the `throw` after the class body reports a line that's many lines
// past the original.
#[test]
async fn jupyter_execute_error_source_map_remaps_after_class_transform()
-> Result<()> {
  let (_ctx, client, _process) = setup().await;
  let code = "class Point {\n\
              \x20\x20constructor(public x: number, public y: number, public z: number) {}\n\
              }\n\
              new Point(1, 2, 3);\n\
              throw new Error(\"fail\");\n";
  // The throw is on line 5 of the user's TypeScript.
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": true,
        "stop_on_error": false,
        "code": code,
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  let status = reply
    .content
    .get("status")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  assert_eq!(
    status, "error",
    "expected status=error: {:?}",
    reply.content
  );
  let traceback = reply
    .content
    .get("traceback")
    .and_then(|v| v.as_array())
    .cloned()
    .unwrap_or_default();
  let joined = traceback
    .iter()
    .filter_map(|v| v.as_str())
    .collect::<Vec<_>>()
    .join("\n");
  assert!(
    joined.contains(":5:"),
    "traceback should point at user line 5 (the throw): {joined:?}",
  );

  Ok(())
}

#[test]
async fn jupyter_stdin_disabled_returns_null() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  // allow_stdin: false should short-circuit prompt() to null without
  // sending anything on the stdin channel.
  client
    .send(
      Shell,
      "execute_request",
      json!({
        "silent": false,
        "store_history": true,
        "user_expressions": {},
        "allow_stdin": false,
        "stop_on_error": false,
        "code": "Deno.jupyter.broadcast(\"stream\", { name: \"stdout\", text: String(prompt(\"x?\")) })",
      }),
    )
    .await?;

  let reply = client.recv(Shell).await?;
  assert_eq!(reply.header.msg_type, "execute_reply");
  assert_json_subset(reply.content, json!({ "status": "ok" }));

  // Find the stream message and confirm the prompt() call returned null.
  let mut found = false;
  for _ in 0..6 {
    let msg = client.recv(IoPub).await?;
    if msg.header.msg_type == "stream"
      && msg.content.get("text").and_then(|v| v.as_str()) == Some("null")
    {
      found = true;
      break;
    }
  }
  assert!(found, "expected stream message with text \"null\"");

  Ok(())
}
