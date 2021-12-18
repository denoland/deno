// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(unused)]

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use data_encoding::HEXLOWER;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use ring::hmac;
use std::env::current_exe;
use tempfile::TempDir;
use tokio::join;
use zeromq::prelude::*;
use zeromq::ZmqMessage;

const DELIMITER: &str = "<IDS|MSG>";

pub fn install() -> Result<(), AnyError> {
  let temp_dir = TempDir::new().unwrap();
  let kernel_json_path = temp_dir.path().join("kernel.json");

  // TODO(bartlomieju): add remaining fields as per
  // https://jupyter-client.readthedocs.io/en/stable/kernels.html#kernel-specs
  // FIXME(bartlomieju): replace `current_exe`
  let json_data = json!({
      "argv": [current_exe().unwrap().to_string_lossy(), "jupyter", "--conn", "{connection_file}"],
      "display_name": "Deno (Rust)",
      "language": "typescript",
  });

  let f = std::fs::File::create(kernel_json_path)?;
  serde_json::to_writer_pretty(f, &json_data)?;

  let child_result = std::process::Command::new("jupyter")
    .args([
      "kernelspec",
      "install",
      "--name",
      "rusty_deno",
      &temp_dir.path().to_string_lossy(),
    ])
    .spawn();

  // TODO(bartlomieju): copy icons the the kernelspec directory

  if let Ok(mut child) = child_result {
    let wait_result = child.wait();
    match wait_result {
      Ok(status) => {
        if !status.success() {
          eprintln!("Failed to install kernelspec, try again.");
        }
      }
      Err(err) => {
        eprintln!("Failed to install kernelspec: {}", err);
      }
    }
  }

  let _ = std::fs::remove_dir(temp_dir);
  println!("Deno kernelspec installed successfully.");
  Ok(())
}

pub async fn kernel(
  _flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  if jupyter_flags.conn_file.is_none() {
    return Err(generic_error("Missing --conn flag"));
  }

  let conn_file_path = jupyter_flags.conn_file.unwrap();

  let conn_file = std::fs::read_to_string(conn_file_path)
    .context("Failed to read connection file")?;
  let conn_spec: ConnectionSpec = serde_json::from_str(&conn_file)
    .context("Failed to parse connection file")?;
  eprintln!("[DENO] parsed conn file: {:#?}", conn_spec);

  let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, conn_spec.key.as_ref());
  let metadata = KernelMetadata::default();
  let iopub_comm = PubComm::new(
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.iopub_port),
    metadata.session_id.clone(),
    hmac_key.clone(),
  );
  let shell_comm = DealerComm::new(
    "shell",
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.shell_port),
    metadata.session_id.clone(),
    hmac_key.clone(),
  );
  let control_comm = DealerComm::new(
    "control",
    create_conn_str(
      &conn_spec.transport,
      &conn_spec.ip,
      conn_spec.control_port,
    ),
    metadata.session_id.clone(),
    hmac_key.clone(),
  );
  let stdin_comm = DealerComm::new(
    "stdin",
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.stdin_port),
    metadata.session_id.clone(),
    hmac_key.clone(),
  );

  let hb_conn_str =
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.hb_port);

  let mut kernel = Kernel {
    metadata,
    conn_spec,
    state: KernelState::Idle,
    iopub_comm,
    shell_comm,
    control_comm,
    stdin_comm,
  };

  eprintln!("[DENO] kernel created: {:#?}", kernel.metadata.session_id);

  let (_first, _second) =
    join!(kernel.run(), create_zmq_reply("hb", &hb_conn_str),);

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KernelState {
  Busy,
  Idle,
  Starting,
}

impl std::fmt::Display for KernelState {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Busy => write!(f, "busy"),
      Self::Idle => write!(f, "idle"),
      Self::Starting => write!(f, "starting"),
    }
  }
}

struct Kernel {
  metadata: KernelMetadata,
  conn_spec: ConnectionSpec,
  state: KernelState,
  iopub_comm: PubComm,
  shell_comm: DealerComm,
  control_comm: DealerComm,
  stdin_comm: DealerComm,
}

impl Kernel {
  async fn run(&mut self) -> Result<(), AnyError> {
    let (iopub_res, shell_res, control_res, stdin_res) = join!(
      self.iopub_comm.connect(),
      self.shell_comm.connect(),
      self.control_comm.connect(),
      self.stdin_comm.connect(),
    );
    iopub_res?;
    shell_res?;
    control_res?;
    stdin_res?;

    loop {
      tokio::select! {
        shell_msg = self.shell_comm.recv() => {
          eprintln!("shell got packet: {:#?}", Message::from_zmq_message(shell_msg?, &self.shell_comm.hmac_key)?);
        },
        control_msg = self.control_comm.recv() => {
          eprintln!("control got packet: {:#?}", Message::from_zmq_message(control_msg?, &self.control_comm.hmac_key)?);
        },
        stdin_msg = self.stdin_comm.recv() => {
          eprintln!("stdin got packet: {:#?}", Message::from_zmq_message(stdin_msg?, &self.stdin_comm.hmac_key)?);
        },
      }
    }
  }

  async fn set_state(&mut self, state: KernelState, ctx: CommContext) {
    if self.state == state {
      return;
    }

    self.state = state;

    let now = std::time::SystemTime::now();
    let now: chrono::DateTime<chrono::Utc> = now.into();
    let now = now.to_rfc3339();

    let msg = Message {
      is_reply: true,
      r#type: "status".to_string(),
      header: MessageHeader {
        msg_id: uuid::Uuid::new_v4().to_string(),
        session: ctx.session_id.clone(),
        // FIXME:
        username: "<TODO>".to_string(),
        date: now.to_string(),
        msg_type: "status".to_string(),
        // TODO: this should be taken from a global,
        version: "5.3".to_string(),
      },
      parent_header: Some(ctx.message.header),
      session_id: ctx.session_id,
      metadata: json!({}),
      content: json!({
        "execution_state": state.to_string(),
      }),
    };
    // ignore any error when announcing changes
    let _ = self.iopub_comm.send(msg).await;
  }

  // TODO(bartlomieju): feels like this info should be a separate struct
  // instead of KernelMetadata
  fn get_kernel_info(&self) -> Value {
    json!({
      "status": "ok",
      "protocol_version": self.metadata.protocol_version,
      "implementation_version": self.metadata.kernel_version,
      "implementation": self.metadata.implementation_name,
      "language_info": {
        "name": self.metadata.language,
        "version": self.metadata.language_version,
        "mime": self.metadata.mime,
        "file_extension": self.metadata.file_ext,
      },
      "help_links": [{
        "text": self.metadata.help_text,
        "url": self.metadata.help_url
      }],
      "banner": self.metadata.banner,
      "debugger": false
    })
  }

  fn get_comm_info() -> Value {
    json!({
      "status": "ok",
      "comms": {}
    })
  }
}

#[derive(Debug)]
struct KernelMetadata {
  banner: String,
  file_ext: String,
  help_text: String,
  help_url: String,
  implementation_name: String,
  kernel_version: String,
  language_version: String,
  language: String,
  mime: String,
  protocol_version: String,
  session_id: String,
}

impl Default for KernelMetadata {
  fn default() -> Self {
    Self {
      banner: "Welcome to Deno kernel".to_string(),
      file_ext: ".ts".to_string(),
      help_text: "<TODO>".to_string(),
      help_url: "https://github.com/denoland/deno".to_string(),
      implementation_name: "Deno kernel".to_string(),
      // FIXME:
      kernel_version: "0.0.1".to_string(),
      // FIXME:
      language_version: "1.16.4".to_string(),
      language: "typescript".to_string(),
      // FIXME:
      mime: "text/x.typescript".to_string(),
      protocol_version: "5.3".to_string(),
      session_id: uuid::Uuid::new_v4().to_string(),
    }
  }
}

#[derive(Debug, Deserialize)]
struct ConnectionSpec {
  ip: String,
  transport: String,
  control_port: u32,
  shell_port: u32,
  stdin_port: u32,
  hb_port: u32,
  iopub_port: u32,
  signature_scheme: String,
  key: String,
}

struct CommContext {
  message: Message,
  hmac: String,
  session_id: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct MessageHeader {
  msg_id: String,
  session: String,
  username: String,
  date: String,
  msg_type: String,
  version: String,
}

#[derive(Debug)]
struct Message {
  is_reply: bool,
  r#type: String,
  header: MessageHeader,
  parent_header: Option<MessageHeader>,
  metadata: Value,
  content: Value,
  session_id: String,
}

impl Message {
  fn new() -> Self {
    todo!()
  }

  fn serialize(&self, hmac_key: &hmac::Key) -> ZmqMessage {
    let header = serde_json::to_string(&self.header).unwrap();
    let parent_header = if let Some(p_header) = self.parent_header.as_ref() {
      serde_json::to_string(p_header).unwrap()
    } else {
      serde_json::to_string(&json!({})).unwrap()
    };
    let metadata = serde_json::to_string(&self.metadata).unwrap();
    let content = serde_json::to_string(&self.content).unwrap();

    let hmac =
      hmac_sign(hmac_key, &header, &parent_header, &metadata, &content);

    let mut zmq_msg = ZmqMessage::from(DELIMITER);
    zmq_msg.push_back(hmac.into());
    zmq_msg.push_back(header.into());
    zmq_msg.push_back(parent_header.into());
    zmq_msg.push_back(metadata.into());
    zmq_msg.push_back(content.into());
    zmq_msg
  }

  fn from_zmq_message(
    zmq_msg: ZmqMessage,
    hmac_key: &hmac::Key,
  ) -> Result<Self, AnyError> {
    // TODO(bartomieju): can these unwraps be better handled?
    let expected_signature_bytes = zmq_msg.get(1).unwrap();
    let header_bytes = zmq_msg.get(2).unwrap();
    let parent_header_bytes = zmq_msg.get(3).unwrap();
    let metadata_bytes = zmq_msg.get(4).unwrap();
    let content_bytes = zmq_msg.get(5).unwrap();

    hmac_verify(
      hmac_key,
      expected_signature_bytes,
      header_bytes,
      parent_header_bytes,
      metadata_bytes,
      content_bytes,
    )?;

    // eprintln!("parent_Header {:?}", String::from_utf8(parent_header_bytes.to_vec()).unwrap());
    // TODO(bartomieju): can these unwraps be better handled?
    let header: MessageHeader = serde_json::from_slice(header_bytes).unwrap();
    // let parent_header: MessageHeader =
    //   serde_json::from_slice(parent_header_bytes).unwrap();
    let metadata: Value = serde_json::from_slice(metadata_bytes).unwrap();
    let content: Value = serde_json::from_slice(content_bytes).unwrap();

    let msg = Message {
      is_reply: false,
      r#type: header.msg_type.clone(),
      header,
      parent_header: None,
      metadata,
      content,
      // FIXME:
      session_id: "".to_string(),
    };

    Ok(msg)
  }
}

struct PubComm {
  conn_str: String,
  session_id: String,
  hmac_key: hmac::Key,
  socket: zeromq::PubSocket,
}

impl PubComm {
  pub fn new(
    conn_str: String,
    session_id: String,
    hmac_key: hmac::Key,
  ) -> Self {
    println!("iopub connection: {}", conn_str);
    Self {
      conn_str,
      session_id,
      hmac_key,
      socket: zeromq::PubSocket::new(),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn send(&mut self, msg: Message) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

struct DealerComm {
  name: String,
  conn_str: String,
  session_id: String,
  hmac_key: hmac::Key,
  socket: zeromq::DealerSocket,
}

impl DealerComm {
  pub fn new(
    name: &str,
    conn_str: String,
    session_id: String,
    hmac_key: hmac::Key,
  ) -> Self {
    println!("dealer '{}' connection: {}", name, conn_str);
    Self {
      name: name.to_string(),
      conn_str,
      session_id,
      hmac_key,
      socket: zeromq::DealerSocket::new(),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn recv(&mut self) -> Result<ZmqMessage, AnyError> {
    let msg = self.socket.recv().await?;
    Ok(msg)
  }

  pub async fn send(&mut self, msg: Message) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

async fn create_zmq_reply(name: &str, conn_str: &str) -> Result<(), AnyError> {
  println!("reply '{}' connection string: {}", name, conn_str);

  let mut sock = zeromq::RepSocket::new(); // TODO(apowers313) exact same as dealer, refactor
  sock.monitor();
  sock.bind(conn_str).await?;

  loop {
    let msg = sock.recv().await?;
    dbg!(&msg);
    println!("{} got packet!", name);
  }
}

fn create_conn_str(transport: &str, ip: &str, port: u32) -> String {
  format!("{}://{}:{}", transport, ip, port)
}

fn hmac_sign(
  key: &hmac::Key,
  header: &str,
  parent_header: &str,
  metadata: &str,
  content: &str,
) -> String {
  let mut hmac_ctx = hmac::Context::with_key(key);
  hmac_ctx.update(header.as_bytes());
  hmac_ctx.update(parent_header.as_bytes());
  hmac_ctx.update(metadata.as_bytes());
  hmac_ctx.update(content.as_bytes());
  let tag = hmac_ctx.sign();
  let sig = HEXLOWER.encode(tag.as_ref());
  sig
}

fn hmac_verify(
  key: &hmac::Key,
  expected_signature: &[u8],
  header: &[u8],
  parent_header: &[u8],
  metadata: &[u8],
  content: &[u8],
) -> Result<(), AnyError> {
  let mut msg = Vec::<u8>::new();
  msg.extend(header);
  msg.extend(parent_header);
  msg.extend(metadata);
  msg.extend(content);
  let sig = HEXLOWER.decode(expected_signature)?;
  hmac::verify(key, msg.as_ref(), sig.as_ref())?;
  Ok(())
}

#[test]
fn hmac_verify_test() {
  let key_value = "1f5cec86-8eaa942eef7f5a35b51ddcf5";
  let key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());

  let expected_signature =
    "43a5c45062e0b6bcc59c727f90165ad1d2eb02e1c5317aa25c2c2049d96d3b6a"
      .as_bytes()
      .to_vec();
  let header = "{\"msg_id\":\"c0fd20872c1b4d1c87e7fc814b75c93e_0\",\"msg_type\":\"kernel_info_request\",\"username\":\"ampower\",\"session\":\"c0fd20872c1b4d1c87e7fc814b75c93e\",\"date\":\"2021-12-10T06:20:40.259695Z\",\"version\":\"5.3\"}".as_bytes().to_vec();
  let parent_header = "{}".as_bytes().to_vec();
  let metadata = "{}".as_bytes().to_vec();
  let content = "{}".as_bytes().to_vec();

  let result = hmac_verify(
    &key,
    &expected_signature,
    &header,
    &parent_header,
    &metadata,
    &content,
  );

  assert!(result.is_ok(), "signature validation failed");
}

#[test]
fn hmac_sign_test() {
  let key_value = "1f5cec86-8eaa942eef7f5a35b51ddcf5";
  let key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());
  let header = "{\"msg_id\":\"c0fd20872c1b4d1c87e7fc814b75c93e_0\",\"msg_type\":\"kernel_info_request\",\"username\":\"ampower\",\"session\":\"c0fd20872c1b4d1c87e7fc814b75c93e\",\"date\":\"2021-12-10T06:20:40.259695Z\",\"version\":\"5.3\"}";
  let parent_header = "{}";
  let metadata = "{}";
  let content = "{}";
  let sig = hmac_sign(&key, header, parent_header, metadata, content);
  assert_eq!(
    sig,
    "43a5c45062e0b6bcc59c727f90165ad1d2eb02e1c5317aa25c2c2049d96d3b6a"
  );
}

// /* *****************
//  * SHELL MESSAGES
//  * *****************/
// Shell Request Message Types
// "execute_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
// "inspect_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

// Shell Reply Message Types
// "execute_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
// "inspect_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
struct ExecuteRequestContent {
  code: String,
  silent: bool,
  store_history: bool,
  user_expressions: Value,
  allow_stdin: bool,
  stop_on_error: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
struct ExecuteReplyContent {
  status: String,
  execution_count: u32,
  payload: Option<Vec<String>>,
  user_expressions: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
struct InspectRequestContent {
  code: String,
  cursor_pos: u32,
  detail_level: u8, // 0 = Low, 1 = High
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
struct InspectReplyContent {
  status: String,
  found: bool,
  data: Option<Value>,
  metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
struct CompleteRequestContent {
  code: String,
  cursor_pos: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
struct CompleteReplyContent {
  status: String,
  matches: Option<Value>,
  cursor_start: u32,
  cursor_end: u32,
  metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
struct HistoryRequestContent {
  output: bool,
  raw: bool,
  hist_access_type: String, // "range" | "tail" | "search"
  session: u32,
  start: u32,
  stop: u32,
  n: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
struct HistoryReplyContent {
  status: String,
  history: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
struct CodeCompleteRequestContent {
  code: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
struct CodeCompleteReplyContent {
  status: String, // "complete" | "incomplete" | "invalid" | "unknown"
  indent: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
struct CommInfoRequestContent {
  target_name: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
struct CommInfoReplyContent {
  status: String,
  comms: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
// struct KernelInfoRequest {} // empty

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
struct KernelInfoReply {
  status: String,
  protocol_version: String,
  implementation: String,
  implementation_version: String,
  language_info: KernelLanguageInfo,
  banner: String,
  debugger: bool,
  help_links: Vec<KernelHelpLink>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
struct KernelLanguageInfo {
  name: String,
  version: String,
  mimetype: String,
  file_extension: String,
  pygments_lexer: String,
  codemirror_mode: Option<Value>,
  nbconvert_exporter: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
struct KernelHelpLink {
  text: String,
  url: String,
}

/* *****************
 * CONTROL MESSAGES
 * *****************/

// Control Request Message Types
// "shutdown_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

// Control Reply Message Types
// "shutdown_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
struct ShutdownRequestContent {
  restart: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
struct ShutdownReplyContent {
  status: String,
  restart: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// struct InterruptRequestContent {} // empty

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
struct InterruptReplyContent {
  status: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// struct DebugRequestContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// struct DebugReplyContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

/* *****************
 * IOPUB MESSAGES
 * *****************/

// Io Pub Message Types
// "stream" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
// "display_data" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
// "update_display_data" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// "execute_input" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
// "execute_result" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
// "error" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
// "status" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
// "clear_output" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
// "debug_event" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// "comm_open" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
// "comm_msg" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
// "comm_close" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#tearing-down-comms

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
struct ErrorStatusContent {
  status: String, // "error"
  ename: String,
  evalue: String,
  traceback: Vec<String>,
  execution_count: Option<u32>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
struct StatusContent {
  status: String, // "ok" | "abort"
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
struct StreamContent {
  name: String, // "stdout" | "stderr"
  text: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
struct DisplayDataContent {
  data: Value,
  metadata: Option<Value>,
  transient: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// struct UpdateDisplayDataContent {} // same as DisplayDataContent

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
struct ExecuteInputContent {
  code: String,
  execution_count: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
struct ExecuteResultContent {
  execution_count: u32,
  data: Option<Value>,
  metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
struct ErrorContent {
  payload: Option<Vec<String>>,
  user_expressions: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
struct KernelStatusContent {
  execution_state: String, // "busy" | "idle" | "starting"
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
struct ClearOutputContent {
  wait: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// struct DebugEventContent {} // see Event message from the Debug Adapter Protocol (DAP)

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
struct CommOpenMessage {
  comm_id: uuid::Uuid,
  target_name: String,
  data: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
struct CommMsgMessage {
  comm_id: uuid::Uuid,
  data: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
struct CommCloseMessage {
  comm_id: uuid::Uuid,
  data: Option<Value>,
}

/* *****************
 * STDIN MESSAGES
 * *****************/
// Stdin Request Message Types
// "input_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

// Stdin Reply Message Types
// "input_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
struct InputRequestContent {
  prompt: String,
  password: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
struct InputReplyContent {
  value: String,
}
