// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use test_util::assertions::assert_json_subset;
use test_util::DenoChild;
use test_util::TestContext;
use test_util::TestContextBuilder;

use chrono::DateTime;
use chrono::Utc;
use deno_core::anyhow::Result;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::time::timeout;
use uuid::Uuid;
use zeromq::SocketRecv;
use zeromq::SocketSend;
use zeromq::ZmqMessage;

/// Jupyter connection file format
#[derive(Serialize)]
struct ConnectionSpec {
  // key used for HMAC signature, if empty, hmac is not used
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
    format!("{}://{}:{}", self.transport, self.ip, port)
  }
}

/// Gets an unused port from the OS, and returns the port number and a
/// `TcpListener` bound to that port. You can keep the listener alive
/// to prevent another process from binding to the port.
fn pick_unused_port() -> (u16, std::net::TcpListener) {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
  (listener.local_addr().unwrap().port(), listener)
}

impl ConnectionSpec {
  fn new() -> (Self, Vec<std::net::TcpListener>) {
    let mut listeners = Vec::new();
    let (hb_port, listener) = pick_unused_port();
    listeners.push(listener);
    let (control_port, listener) = pick_unused_port();
    listeners.push(listener);
    let (shell_port, listener) = pick_unused_port();
    listeners.push(listener);
    let (stdin_port, listener) = pick_unused_port();
    listeners.push(listener);
    let (iopub_port, listener) = pick_unused_port();
    listeners.push(listener);

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
  routing_prefix: Vec<String>,
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
      routing_prefix: vec![Uuid::new_v4().to_string()],
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
  fn to_raw(&self) -> ZmqMessage {
    let mut parts = Vec::new();
    parts.extend(
      self
        .routing_prefix
        .iter()
        .map(|uuid| uuid.as_bytes().to_vec().into()),
    );
    parts.push(Bytes::from_static(DELIMITER));
    parts.push(self.signature.clone().into());
    parts.push(serde_json::to_vec(&self.header).unwrap().into());
    parts.push(self.parent_header.to_string().into());
    parts.push(self.metadata.to_string().into());
    parts.push(self.content.to_string().into());
    parts.extend(self.buffers.clone());
    ZmqMessage::try_from(parts).unwrap()
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

  fn from_raw(msg: ZmqMessage) -> Self {
    let parts = msg.into_vec();
    let delimiter = parts.iter().position(|part| part == DELIMITER).unwrap();
    let routing_prefix = parts[..delimiter]
      .iter()
      .map(|part: &Bytes| String::from_utf8_lossy(part.as_ref()).to_string())
      .collect();
    let signature = String::from_utf8(parts[delimiter + 1].to_vec())
      .expect("Failed to parse signature");
    let header: MsgHeader = serde_json::from_slice(&parts[delimiter + 2])
      .expect("Failed to parse header");
    let parent_header: Value =
      serde_json::from_slice(&parts[delimiter + 3]).unwrap();
    let metadata: Value =
      serde_json::from_slice(&parts[delimiter + 4]).unwrap();
    let content: Value = serde_json::from_slice(&parts[delimiter + 5]).unwrap();
    let buffers = parts[delimiter + 6..].to_vec();
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
}

async fn connect_socket<S: zeromq::Socket>(
  spec: &ConnectionSpec,
  port: u16,
) -> S {
  let addr = spec.endpoint(port);
  let mut socket = S::new();
  match timeout(Duration::from_millis(5000), socket.connect(&addr)).await {
    Ok(Ok(_)) => socket,
    Ok(Err(e)) => {
      panic!("Failed to connect to {addr}: {e}");
    }
    Err(e) => {
      panic!("Timed out connecting to {addr}: {e}");
    }
  }
}

#[derive(Clone)]
struct JupyterClient {
  recv_timeout: Duration,
  session: Uuid,
  heartbeat: Arc<Mutex<zeromq::ReqSocket>>,
  control: Arc<Mutex<zeromq::DealerSocket>>,
  shell: Arc<Mutex<zeromq::DealerSocket>>,
  io_pub: Arc<Mutex<zeromq::SubSocket>>,
  stdin: Arc<Mutex<zeromq::RouterSocket>>,
}

#[derive(Debug, Clone, Copy)]
enum JupyterChannel {
  Control,
  Shell,
  #[allow(dead_code)]
  Stdin,
  IoPub,
}

use JupyterChannel::*;

impl JupyterClient {
  async fn new(spec: &ConnectionSpec) -> Self {
    Self::new_with_timeout(spec, Duration::from_secs(10)).await
  }

  async fn new_with_timeout(spec: &ConnectionSpec, timeout: Duration) -> Self {
    let (heartbeat, control, shell, io_pub, stdin) = tokio::join!(
      connect_socket::<zeromq::ReqSocket>(spec, spec.hb_port),
      connect_socket::<zeromq::DealerSocket>(spec, spec.control_port),
      connect_socket::<zeromq::DealerSocket>(spec, spec.shell_port),
      connect_socket::<zeromq::SubSocket>(spec, spec.iopub_port),
      connect_socket::<zeromq::RouterSocket>(spec, spec.stdin_port),
    );

    Self {
      session: Uuid::new_v4(),
      heartbeat: Arc::new(Mutex::new(heartbeat)),
      control: Arc::new(Mutex::new(control)),
      shell: Arc::new(Mutex::new(shell)),
      io_pub: Arc::new(Mutex::new(io_pub)),
      stdin: Arc::new(Mutex::new(stdin)),
      recv_timeout: timeout,
    }
  }

  async fn io_subscribe(&self, topic: &str) -> Result<()> {
    Ok(self.io_pub.lock().await.subscribe(topic).await?)
  }

  async fn recv_with_timeout<S: SocketRecv>(
    &self,
    s: &mut S,
  ) -> Result<JupyterMsg> {
    let msg = timeout(self.recv_timeout, s.recv()).await??;
    Ok(JupyterMsg::from_raw(msg))
  }

  async fn send_msg(
    &self,
    channel: JupyterChannel,
    msg: JupyterMsg,
  ) -> Result<JupyterMsg> {
    let raw = msg.to_raw();
    match channel {
      Control => self.control.lock().await.send(raw).await?,
      Shell => self.shell.lock().await.send(raw).await?,
      Stdin => self.stdin.lock().await.send(raw).await?,
      IoPub => panic!("Cannot send over IOPub"),
    }
    Ok(msg)
  }

  async fn send(
    &self,
    channel: JupyterChannel,
    msg_type: &str,
    content: Value,
  ) -> Result<JupyterMsg> {
    let msg = JupyterMsg::new(self.session, msg_type, content);
    self.send_msg(channel, msg).await
  }

  async fn recv(&self, channel: JupyterChannel) -> Result<JupyterMsg> {
    Ok(match channel {
      Control => {
        self
          .recv_with_timeout(&mut *self.control.lock().await)
          .await?
      }
      Shell => {
        self
          .recv_with_timeout(&mut *self.shell.lock().await)
          .await?
      }
      Stdin => {
        self
          .recv_with_timeout(&mut *self.stdin.lock().await)
          .await?
      }
      IoPub => {
        self
          .recv_with_timeout(&mut *self.io_pub.lock().await)
          .await?
      }
    })
  }

  async fn send_heartbeat(&self, bytes: impl AsRef<[u8]>) -> Result<()> {
    Ok(
      self
        .heartbeat
        .lock()
        .await
        .send(ZmqMessage::from(bytes.as_ref().to_vec()))
        .await?,
    )
  }

  async fn recv_heartbeat(&self) -> Result<Bytes> {
    Ok(
      timeout(self.recv_timeout, self.heartbeat.lock().await.recv())
        .await??
        .into_vec()[0]
        .clone(),
    )
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

// Wrapper around the Jupyter server process that
// ensures the process is killed when dropped.
struct JupyterServerProcess(Option<DenoChild>);

impl JupyterServerProcess {
  // Wait for the process to exit, or kill it after the given duration.
  //
  // Ideally we could use this at the end of each test, but the server
  // doesn't seem to exit in a reasonable amount of time after getting
  // a shutdown request.
  #[allow(dead_code)]
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
      // already exited
      return;
    }
    proc.kill().unwrap();
  }
}

async fn server_ready_on(addr: &str) -> bool {
  matches!(
    timeout(
      Duration::from_millis(1000),
      tokio::net::TcpStream::connect(addr.trim_start_matches("tcp://")),
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
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let (mut conn, mut listeners) = ConnectionSpec::new();
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

  // drop the listeners so the server can listen on the ports
  drop(listeners);

  // try to start the server, retrying up to 5 times
  // (this can happen due to TOCTOU errors with selecting unused TCP ports)
  let mut process = start_process(&conn_file);

  'outer: for i in 0..10 {
    // try to see if the server is healthy
    for _ in 0..10 {
      // server still running?
      if process.try_wait().unwrap().is_none() {
        // listening on all ports?
        if server_ready(&conn).await {
          // server is ready to go
          break 'outer;
        }
      } else {
        // server exited, try again
        break;
      }
      tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // pick new ports and try again
    (conn, listeners) = ConnectionSpec::new();
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
  // make sure server is ready to receive messages
  client.send_heartbeat(b"ping").await.unwrap();
  let _ = client.recv_heartbeat().await.unwrap();

  (context, client, process)
}

#[tokio::test]
async fn jupyter_heartbeat_echoes() -> Result<()> {
  let (_ctx, client, _process) = setup().await;
  client.send_heartbeat(b"ping").await?;
  let msg = client.recv_heartbeat().await?;
  assert_eq!(msg, Bytes::from_static(b"pong"));

  Ok(())
}

#[tokio::test]
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
        "nbconvert_exporter": "script"
      },
    }),
  );

  Ok(())
}

#[tokio::test]
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
          // may timeout if we missed some messages
          eprintln!("Timed out waiting for messages");
        }
        panic!("Error: {:#?}", e);
      }
    }
  }

  let execution_idle = msgs
    .iter()
    .find(|msg| {
      if let Some(state) = msg.content.get("execution_state") {
        state == "idle"
      } else {
        false
      }
    })
    .expect("execution_state idle not found");
  assert_eq!(execution_idle.parent_header, request.header.to_json());
  assert_json_subset(
    execution_idle.content.clone(),
    json!({
      "execution_state": "idle",
    }),
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
      "text": "asdf\n", // the trailing newline is added by console.log
    }),
  );

  Ok(())
}

#[tokio::test]
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

#[tokio::test]
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
    json!({
      "status": "ok",
      "execution_count": 0,
    }),
  );

  for _ in 0..3 {
    let resp = reqwest::get("http://localhost:10234").await.unwrap();
    let text: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(text, json!({ "hello": "world" }));
  }

  Ok(())
}
