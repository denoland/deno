use std::future::Future;
use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use test_util::DenoChild;
use test_util::TestContext;
use test_util::TestContextBuilder;

use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::timeout;
use uuid::Uuid;
use zeromq::SocketRecv;
use zeromq::SocketSend;
use zeromq::ZmqMessage;

// for the `utc_now` function
include!("../../cli/util/time.rs");

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

fn pick_unused_port() -> u16 {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
  listener.local_addr().unwrap().port()
}

impl Default for ConnectionSpec {
  fn default() -> Self {
    Self {
      key: "".into(),
      signature_scheme: "hmac-sha256".into(),
      transport: "tcp".into(),
      ip: "127.0.0.1".into(),
      hb_port: pick_unused_port(),
      control_port: pick_unused_port(),
      shell_port: pick_unused_port(),
      stdin_port: pick_unused_port(),
      iopub_port: pick_unused_port(),
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
      date: utc_now().to_rfc3339(),
      username: "test".into(),
      msg_type: "kernel_info_request".into(),
      version: "5.3".into(),
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
  Stdin,
  IoPub,
}

use JupyterChannel::*;

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

  async fn recv_with_timeout<S: SocketRecv>(&self, s: &mut S) -> JupyterMsg {
    let msg = timeout(self.recv_timeout, s.recv()).await.unwrap().unwrap();
    JupyterMsg::from_raw(msg)
  }

  async fn send_msg(&self, channel: JupyterChannel, msg: JupyterMsg) {
    let raw = msg.to_raw();
    match channel {
      Control => self.control.lock().send(raw).await.unwrap(),
      Shell => self.shell.lock().send(raw).await.unwrap(),
      Stdin => self.stdin.lock().send(raw).await.unwrap(),
      IoPub => panic!("Cannot send over IOPub"),
    }
  }

  async fn send(
    &self,
    channel: JupyterChannel,
    msg_type: &str,
    content: Value,
  ) {
    let msg = JupyterMsg::new(self.session, msg_type, content);
    self.send_msg(channel, msg).await;
  }

  async fn recv(&self, channel: JupyterChannel) -> JupyterMsg {
    match channel {
      Control => self.recv_with_timeout(&mut *self.control.lock()).await,
      Shell => self.recv_with_timeout(&mut *self.shell.lock()).await,
      Stdin => self.recv_with_timeout(&mut *self.stdin.lock()).await,
      IoPub => self.recv_with_timeout(&mut *self.io_pub.lock()).await,
    }
  }

  async fn send_heartbeat(&self, bytes: impl AsRef<[u8]>) {
    self
      .heartbeat
      .lock()
      .send(ZmqMessage::from(bytes.as_ref().to_vec()))
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

async fn setup() -> (TestContext, JupyterClient, JupyterServerProcess) {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let conn = ConnectionSpec::default();
  let conn_file = context.temp_dir().path().join("connection.json");
  conn_file.write_json(&conn);

  let process = context
    .new_command()
    .stderr_piped()
    .stdout_piped()
    .args_vec(vec![
      "jupyter",
      "--kernel",
      "--conn",
      conn_file.to_string().as_str(),
    ])
    .spawn()
    .unwrap();

  let client = JupyterClient::new(&conn).await;

  (context, client, JupyterServerProcess(Some(process)))
}

struct JupyterServerProcess(Option<DenoChild>);

impl JupyterServerProcess {
  async fn wait_or_kill(mut self, wait: Duration) -> Output {
    wait_or_kill(self.0.take().unwrap(), wait).await
  }
}

impl Drop for JupyterServerProcess {
  fn drop(&mut self) {
    let Some(mut proc) = self.0.take() else {
      return;
    };
    if let Some(_) = proc.try_wait().unwrap() {
      return;
    }
    proc.kill().unwrap();
  }
}

#[tokio::test]
async fn jupyter_heartbeat_echoes() {
  let (_ctx, client, _process) = setup().await;
  client.send_heartbeat(b"ping").await;
  let msg = client.recv_heartbeat().await;
  assert_eq!(msg, Bytes::from_static(b"ping"));
}

#[tokio::test]
async fn jupyter_smoke_test() {
  let (_ctx, client, process) = setup().await;
  client.send(Control, "kernel_info_request", json!({})).await;
  let msg = client.recv(Control).await;
  assert_eq!(msg.header.msg_type, "kernel_info_reply");
  println!("Kernel info reply: {:#?}", msg.content);

  client
    .send(Control, "shutdown_request", json!({"restart": false}))
    .await;

  println!("Waiting for process to exit");

  let output = process.wait_or_kill(Duration::from_secs(10)).await;
  println!(
    "jupyter output: {}\n\t{}\n\t{}",
    output.status,
    String::from_utf8(output.stdout).unwrap(),
    String::from_utf8(output.stderr).unwrap()
  );
}
