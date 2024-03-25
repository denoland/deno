use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use test_util::DenoChild;
use test_util::TestContextBuilder;

use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::time::timeout;
use uuid::Uuid;
use zeromq::SocketRecv;
use zeromq::SocketSend;
use zeromq::ZmqMessage;

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
    format!("{}://{}:{}", self.transport, self.ip, port)
  }
}

impl Default for ConnectionSpec {
  fn default() -> Self {
    Self {
      key: "".into(),
      signature_scheme: "hmac-sha256".into(),
      transport: "tcp".into(),
      ip: "127.0.0.1".into(),
      hb_port: 9000,
      control_port: 9001,
      shell_port: 9002,
      stdin_port: 9003,
      iopub_port: 9004,
      kernel_name: "deno".into(),
    }
  }
}

const DELIMITER: &[u8] = b"<IDS|MSG>";

#[derive(Debug, Clone)]
struct JupyterMsg {
  routing_prefix: Vec<Uuid>,
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
      routing_prefix: vec![Uuid::new_v4()],
      signature: "".into(),
      header: MsgHeader {
        msg_type: "kernel_info_request".into(),
        version: "5.2".into(),
        ..Default::default()
      },
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
  date: String,
  username: String,
  msg_type: String,
  version: String,
}

impl Default for MsgHeader {
  fn default() -> Self {
    Self {
      msg_id: Uuid::new_v4(),
      session: Uuid::new_v4(),
      date: "2021-09-01T00:00:00.000Z".into(),
      username: "test".into(),
      msg_type: "kernel_info_request".into(),
      version: "5.2".into(),
    }
  }
}

impl JupyterMsg {
  fn to_raw(&self) -> ZmqMessage {
    println!("to_raw: {self:?}");
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

  fn new(msg_type: impl AsRef<str>, content: Value) -> Self {
    Self {
      header: MsgHeader {
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
      .map(|part| Uuid::from_bytes(part.as_ref().try_into().unwrap()))
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
  socket.connect(&addr).await.unwrap();
  socket
}

async fn bind_socket<S: zeromq::Socket>(spec: &ConnectionSpec, port: u16) -> S {
  let addr = spec.endpoint(port);
  let mut socket = S::new();
  socket.bind(&addr).await.unwrap();
  socket
}

#[derive(Clone)]
struct JupyterClient {
  recv_timeout: Duration,
  heartbeat: Arc<Mutex<zeromq::ReqSocket>>,
  control: Arc<Mutex<zeromq::DealerSocket>>,
  shell: Arc<Mutex<zeromq::DealerSocket>>,
  io_pub: Arc<Mutex<zeromq::SubSocket>>,
  stdin: Arc<Mutex<zeromq::RouterSocket>>,
}

impl JupyterClient {
  async fn new(spec: &ConnectionSpec) -> Self {
    Self::new_with_timeout(spec, Duration::from_secs(5)).await
  }

  async fn new_with_timeout(spec: &ConnectionSpec, timeout: Duration) -> Self {
    let (heartbeat, control, shell, io_pub, stdin) = tokio::join!(
      connect_socket::<zeromq::ReqSocket>(spec, spec.hb_port),
      connect_socket::<zeromq::DealerSocket>(spec, spec.control_port),
      connect_socket::<zeromq::DealerSocket>(spec, spec.shell_port),
      connect_socket::<zeromq::SubSocket>(spec, spec.iopub_port),
      connect_socket::<zeromq::RouterSocket>(spec, spec.stdin_port),
    );
    fn wrap<T>(t: T) -> Arc<Mutex<T>> {
      Arc::new(Mutex::new(t))
    }

    Self {
      heartbeat: wrap(heartbeat),
      control: wrap(control),
      shell: wrap(shell),
      io_pub: wrap(io_pub),
      stdin: wrap(stdin),
      recv_timeout: timeout,
    }
  }

  async fn send_control(&self, msg_type: &str, content: Value) {
    let msg = JupyterMsg::new(msg_type, content);
    let raw = msg.to_raw();
    println!("sending control {}", raw.len());
    self.control.lock().send(msg.to_raw()).await.unwrap();
  }

  async fn recv_control(&self) -> JupyterMsg {
    JupyterMsg::from_raw(self.control.lock().recv().await.unwrap())
  }

  async fn recv<S: SocketRecv>(&self, s: &mut S) -> JupyterMsg {
    let msg = timeout(self.recv_timeout, s.recv()).await.unwrap().unwrap();
    JupyterMsg::from_raw(msg)
  }

  async fn send_shell(&self, msg_type: &str, content: Value) {
    let msg = JupyterMsg::new(msg_type, content);
    self.shell.lock().send(msg.to_raw()).await.unwrap();
  }

  async fn recv_shell(&self) -> JupyterMsg {
    self.recv(&mut *self.shell.lock()).await
  }

  async fn send_stdin(&self, msg_type: &str, content: Value) {
    let msg = JupyterMsg::new(msg_type, content);
    self.stdin.lock().send(msg.to_raw()).await.unwrap();
  }

  async fn recv_stdin(&self) -> JupyterMsg {
    self.recv(&mut *self.stdin.lock()).await
  }

  async fn recv_io_pub(&self) -> JupyterMsg {
    self.recv(&mut *self.io_pub.lock()).await
  }

  async fn send_heartbeat(&self) {
    self
      .heartbeat
      .lock()
      .send(ZmqMessage::from(b"ping".to_vec()))
      .await
      .unwrap();
  }

  async fn recv_heartbeat(&self) -> Bytes {
    timeout(self.recv_timeout, self.heartbeat.lock().recv())
      .await
      .unwrap()
      .unwrap()
      .into_vec()[0]
      .clone()
  }
}

async fn wait_or_kill(mut process: DenoChild, wait: Duration) -> Output {
  let start = std::time::Instant::now();
  while start.elapsed() < wait {
    if let Some(_) = process.try_wait().unwrap() {
      return process.wait_with_output().unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  process.kill().unwrap();
  return process.wait_with_output().unwrap();
}

#[tokio::test]
async fn jupyter_smoke_test() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let conn = ConnectionSpec::default();
  let conn_file = context.temp_dir().path().join("connection.json");
  conn_file.write_json(&conn);

  let mut process = context
    .new_command()
    .args_vec(vec![
      "jupyter",
      "--kernel",
      "--conn",
      conn_file.to_string().as_str(),
    ])
    .split_output()
    .spawn()
    .unwrap();

  println!("Making client");
  let client = JupyterClient::new(&conn).await;
  println!("Starting kernel info req");

  client.send_control("kernel_info_request", json!({})).await;
  println!("Waiting for kernel info reply");
  let msg = client.recv_control().await;
  assert_eq!(msg.header.msg_type, "kernel_info_reply");
  println!("Kernel info reply: {:#?}", msg.content);

  client
    .send_control("shutdown_request", json!({"restart": false}))
    .await;

  println!("Waiting for process to exit");

  let _status = process.try_wait().unwrap();

  // process.kill().unwrap();
  let output = wait_or_kill(process, Duration::from_secs(5)).await;
  println!(
    "jupyter output: {}\n\t{}\n\t{}",
    output.status,
    String::from_utf8(output.stdout).unwrap(),
    String::from_utf8(output.stderr).unwrap()
  );
}
