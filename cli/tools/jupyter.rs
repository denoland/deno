// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(unused)]

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use data_encoding::HEXLOWER;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use ring::hmac;
use std::collections::HashMap;
use std::env::current_exe;
use std::time::Duration;
use tempfile::TempDir;
use tokio::join;
use tokio::time::sleep;
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

  let mut kernel = Kernel::new(conn_file_path.to_str().unwrap());
  println!("[DENO] kernel created: {:#?}", kernel.session_id);

  println!("running kernel...");
  kernel.run().await;
  println!("done running kernel.");

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
  session_id: String,
  execution_count: u32,
}

enum HandlerType {
  Shell,
  Control,
  Stdin,
}

impl Kernel {
  fn new(conn_file_path: &str) -> Self {
    let conn_file = match std::fs::read_to_string(conn_file_path)
      .context("Failed to read connection file")
    {
      Ok(cf) => cf,
      Err(_) => {
        println!("Couldn't read connection file: {}", conn_file_path);
        std::process::exit(1);
      }
    };
    let conn_spec: ConnectionSpec = match serde_json::from_str(&conn_file)
      .context("Failed to parse connection file")
    {
      Ok(cs) => cs,
      Err(_) => {
        println!("Connection file isn't proper JSON: {}", conn_file_path);
        std::process::exit(1);
      }
    };
    println!("[DENO] parsed conn file: {:#?}", conn_spec);

    let execution_count = 0;
    let session_id = uuid::Uuid::new_v4().to_string();
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, conn_spec.key.as_ref());
    let metadata = KernelMetadata::default();
    let iopub_comm = PubComm::new(
      create_conn_str(
        &conn_spec.transport,
        &conn_spec.ip,
        conn_spec.iopub_port,
      ),
      session_id.clone(),
      hmac_key.clone(),
    );
    let shell_comm = DealerComm::new(
      "shell",
      create_conn_str(
        &conn_spec.transport,
        &conn_spec.ip,
        conn_spec.shell_port,
      ),
      session_id.clone(),
      hmac_key.clone(),
    );
    let control_comm = DealerComm::new(
      "control",
      create_conn_str(
        &conn_spec.transport,
        &conn_spec.ip,
        conn_spec.control_port,
      ),
      session_id.clone(),
      hmac_key.clone(),
    );
    let stdin_comm = DealerComm::new(
      "stdin",
      create_conn_str(
        &conn_spec.transport,
        &conn_spec.ip,
        conn_spec.stdin_port,
      ),
      session_id.clone(),
      hmac_key.clone(),
    );

    let hb_conn_str =
      create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.hb_port);

    let s: Kernel = Self {
      metadata,
      conn_spec,
      state: KernelState::Idle,
      iopub_comm,
      shell_comm,
      control_comm,
      stdin_comm,
      session_id,
      execution_count,
    };

    s
  }

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

    // TODO(apowers313): run heartbeat
    // create_zmq_reply("hb", &hb_conn_str)

    loop {
      tokio::select! {
        shell_msg = self.shell_comm.recv() => {
          // println!("shell got packet: {:#?}", shell_msg);
          self.handler(HandlerType::Shell, shell_msg).await;
        },
        control_msg = self.control_comm.recv() => {
          // println!("control got packet: {:#?}", control_msg);
          self.handler(HandlerType::Control, control_msg);
        },
        stdin_msg = self.stdin_comm.recv() => {
          // println!("stdin got packet: {:#?}", stdin_msg);
          self.handler(HandlerType::Stdin, stdin_msg);
        },
      }
    }
  }

  async fn handler(
    &mut self,
    handler_type: HandlerType,
    recv_result: Result<RequestMessage, AnyError>,
  ) {
    let req_msg = match recv_result {
      Ok(m) => m,
      Err(e) => {
        println!("error receiving msg: {}", e);
        return;
      }
    };

    let comm_ctx = CommContext {
      session_id: self.session_id.clone(),
      message: req_msg,
    };

    match self.set_state(&comm_ctx, KernelState::Busy).await {
      Ok(_) => {}
      Err(e) => {
        println!("error setting busy state: {}", e);
        return;
      }
    };

    let major_version = &comm_ctx.message.header.version.to_string()[0..1];
    let res = match (handler_type, major_version) {
      // TODO(apowers313) implement new and old Jupyter protocols here
      (HandlerType::Shell, "5") => self.shell_handler(&comm_ctx).await,
      (HandlerType::Control, "5") => self.control_handler(&comm_ctx),
      (HandlerType::Stdin, "5") => self.stdin_handler(&comm_ctx),
      _ => Err(anyhow!(
        "No handler for message: '{}' v{}",
        comm_ctx.message.header.msg_type,
        major_version
      )),
    };

    match res {
      Ok(_) => {}
      Err(e) => {
        println!(
          "Error handling packet '{}': {}",
          comm_ctx.message.header.msg_type, e
        );
      }
    };

    match self.set_state(&comm_ctx, KernelState::Idle).await {
      Ok(_) => {}
      Err(e) => {
        println!("error setting idle state: {}", e);
        return;
      }
    };
  }

  async fn shell_handler(
    &mut self,
    comm_ctx: &CommContext,
  ) -> Result<(), AnyError> {
    match comm_ctx.message.header.msg_type.as_ref() {
      "kernel_info_request" => self.kernel_info_reply(comm_ctx).await?,
      "execute_request" => self.execute_request(comm_ctx).await?,
      _ => {
        return Err(anyhow!(
          "no handler for msg_id: '{}'",
          comm_ctx.message.header.msg_id
        ))
      }
    };

    Ok(())
  }

  fn control_handler(&self, comm_ctx: &CommContext) -> Result<(), AnyError> {
    todo!()
  }

  fn stdin_handler(&self, comm_ctx: &CommContext) -> Result<(), AnyError> {
    todo!()
  }

  async fn set_state(
    &mut self,
    comm_ctx: &CommContext,
    state: KernelState,
  ) -> Result<(), AnyError> {
    if self.state == state {
      return Ok(());
    }

    self.state = state;

    let now = std::time::SystemTime::now();
    let now: chrono::DateTime<chrono::Utc> = now.into();
    let now = now.to_rfc3339();

    let s = match state {
      KernelState::Busy => "busy".to_string(),
      KernelState::Idle => "idle".to_string(),
      KernelState::Starting => "starting".to_string(),
    };

    let msg = SideEffectMessage::new(
      &comm_ctx,
      "status".to_string(),
      ReplyMetadata::Empty,
      ReplyContent::Status(KernelStatusContent { execution_state: s }),
    );

    self.iopub_comm.send(msg).await
  }

  async fn kernel_info_reply(
    &mut self,
    comm_ctx: &CommContext,
  ) -> Result<(), AnyError> {
    let content = KernelInfoReplyContent {
      status: String::from("ok"),
      protocol_version: self.metadata.protocol_version.clone(),
      implementation_version: self.metadata.kernel_version.clone(),
      implementation: self.metadata.implementation_name.clone(),
      language_info: KernelLanguageInfo {
        name: self.metadata.language.clone(),
        version: self.metadata.language_version.clone(),
        mimetype: self.metadata.mime.clone(),
        file_extension: self.metadata.file_ext.clone(),
        // TODO: "None" gets translated to "null"
        // codemirror_mode: None,
        // nbconvert_exporter: None,
        // pygments_lexer: None,
      },
      help_links: vec![], // TODO(apowers313) dig up help links
      banner: self.metadata.banner.clone(),
      debugger: false,
    };

    let reply = ReplyMessage::new(
      comm_ctx,
      "kernel_info_reply".to_string(),
      ReplyMetadata::Empty,
      ReplyContent::KernelInfo(content),
    );

    self.shell_comm.send(reply).await?;

    Ok(())
  }

  async fn execute_request(
    &mut self,
    comm_ctx: &CommContext,
  ) -> Result<(), AnyError> {
    self.execution_count = self.execution_count + 1;

    let exec_request_content = match &comm_ctx.message.content {
      RequestContent::Execute(c) => c,
      _ => return Err(anyhow!("malformed execution content")),
    };

    let input_msg = SideEffectMessage::new(
      comm_ctx,
      "execute_input".to_string(),
      ReplyMetadata::Empty,
      ReplyContent::ExecuteInput(ExecuteInputContent {
        code: exec_request_content.code.clone(),
        execution_count: self.execution_count,
      }),
    );
    self.iopub_comm.send(input_msg).await?;

    // TODO(apowers313) it executes code... just not the code you requested :)
    // hook in the real REPL request to execute code here
    let result = self.fake_task(&comm_ctx, "foo".to_string()).await?;

    self.exec_done(&comm_ctx, result).await?;

    Ok(())
  }

  async fn exec_done(
    &mut self,
    comm_ctx: &CommContext,
    result: ExecResult,
  ) -> Result<(), AnyError> {
    match result {
      ExecResult::OkString(v) => {
        println!("sending exec result");
        let msg = ReplyMessage::new(
          comm_ctx,
          "execute_reply".to_string(),
          ReplyMetadata::Empty,
          ReplyContent::ExecuteReply(ExecuteReplyContent {
            status: "ok".to_string(),
            execution_count: self.execution_count,
            // TODO: "None" gets translated to "null" by serde_json
            // payload: None,
            // user_expressions: None,
          }),
        );
        self.shell_comm.send(msg).await?;
      }
      ExecResult::Error(e) => {
        println!("Not implemented: sending exec ERROR");
      }
    };

    // TODO(apowers313) send(ExecuteResult)

    Ok(())
  }

  async fn send_stdio(
    &mut self,
    comm_ctx: &CommContext,
    t: StdioType,
    text: &String,
  ) -> Result<(), AnyError> {
    let content = StreamContent {
      name: match t {
        StdioType::Stdout => "stdout".to_string(),
        StdioType::Stderr => "stderr".to_string(),
      },
      text: text.clone(),
    };

    let msg = SideEffectMessage::new(
      &comm_ctx,
      "stream".to_string(),
      ReplyMetadata::Empty,
      ReplyContent::Stream(content),
    );

    self.iopub_comm.send(msg).await?;

    Ok(())
  }

  async fn fake_task(
    &mut self,
    comm_ctx: &CommContext,
    arg: String,
  ) -> Result<ExecResult, AnyError> {
    for i in 0..6 {
      sleep(Duration::from_millis(500)).await;
      println!("ping! {}", &arg);
      self
        .send_stdio(comm_ctx, StdioType::Stdout, &format!("ping! {}\n", i))
        .await?;
    }

    // TODO(apowers313) result should be any valid JavaScript value
    Ok(ExecResult::OkString("fake result".to_string()))
  }
}

enum ExecResult {
  OkString(String),
  // TODO(apowers313)
  // OkValue(Value),
  // OkDisplayData(DisplayDataContent),
  Error(ExecError),
}

struct ExecError {
  err_name: String,
  err_value: String,
  stack_trace: Vec<String>,
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

#[derive(Debug)]
struct CommContext {
  message: RequestMessage,
  session_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MessageHeader {
  msg_id: String,
  session: String,
  username: String,
  // TODO(apowers313) -- date as an Option is to address a Jupyter bug
  // see also: https://github.com/jupyter/notebook/issues/6257
  date: Option<String>,
  msg_type: String,
  version: String,
}

impl MessageHeader {
  fn new(msg_type: String, session_id: String) -> Self {
    let now = std::time::SystemTime::now();
    let now: chrono::DateTime<chrono::Utc> = now.into();
    let now = now.to_rfc3339();

    Self {
      msg_id: uuid::Uuid::new_v4().to_string(),
      session: session_id.clone(),
      // FIXME:
      username: "<TODO>".to_string(),
      date: Some(now.to_string()),
      msg_type,
      // TODO: this should be taken from a global,
      version: "5.3".to_string(),
    }
  }
}

#[derive(Debug)]
struct RequestMessage {
  header: MessageHeader,
  parent_header: Option<()>,
  metadata: RequestMetadata,
  content: RequestContent,
}

impl RequestMessage {
  fn new(
    header: MessageHeader,
    metadata: RequestMetadata,
    content: RequestContent,
  ) -> Self {
    Self {
      header,
      parent_header: None,
      metadata,
      content,
    }
  }
}

impl TryFrom<ZmqMessage> for RequestMessage {
  type Error = AnyError;

  fn try_from(zmq_msg: ZmqMessage) -> Result<Self, AnyError> {
    // TODO(apowers313) make all unwraps recoverable errors
    let header_bytes = zmq_msg.get(2).unwrap();
    let metadata_bytes = zmq_msg.get(4).unwrap();
    let content_bytes = zmq_msg.get(5).unwrap();

    let header: MessageHeader = serde_json::from_slice(header_bytes).unwrap();

    // TODO(apowers313) refactor to an unwrap function to handles unwrapping based on different protocol versions
    let mc = match header.msg_type.as_ref() {
      "kernel_info_request" => (RequestMetadata::Empty, RequestContent::Empty),
      "execute_request" => (
        RequestMetadata::Empty,
        RequestContent::Execute(serde_json::from_slice(content_bytes).unwrap()),
      ),
      _ => (RequestMetadata::Empty, RequestContent::Empty),
    };

    let rm = RequestMessage::new(header, mc.0, mc.1);
    println!("<== RECEIVING: {:#?}", rm);

    Ok(rm)
  }
}

struct ReplyMessage {
  header: MessageHeader,
  parent_header: MessageHeader,
  metadata: ReplyMetadata,
  content: ReplyContent,
}

impl ReplyMessage {
  fn new(
    comm_ctx: &CommContext,
    msg_type: String,
    metadata: ReplyMetadata,
    content: ReplyContent,
  ) -> Self {
    Self {
      header: MessageHeader::new(msg_type, comm_ctx.session_id.clone()),
      parent_header: comm_ctx.message.header.clone(),
      metadata,
      content,
    }
  }

  fn serialize(&self, hmac_key: &hmac::Key) -> ZmqMessage {
    // TODO(apowers313) convert unwrap() to recoverable error
    let header = serde_json::to_string(&self.header).unwrap();
    let parent_header = serde_json::to_string(&self.parent_header).unwrap();
    let metadata = serde_json::to_string(&self.metadata).unwrap();
    let metadata = match &self.metadata {
      ReplyMetadata::Empty => serde_json::to_string(&json!({})).unwrap(),
    };
    let content = match &self.content {
      ReplyContent::Empty => serde_json::to_string(&json!({})).unwrap(),
      // reply messages
      ReplyContent::KernelInfo(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteReply(c) => serde_json::to_string(&c).unwrap(),
      // side effects
      ReplyContent::Status(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::Stream(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteInput(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteResult(c) => serde_json::to_string(&c).unwrap(),
    };

    let hmac =
      hmac_sign(hmac_key, &header, &parent_header, &metadata, &content);

    let mut zmq_msg = ZmqMessage::from(DELIMITER);
    zmq_msg.push_back(hmac.into());
    zmq_msg.push_back(header.into());
    zmq_msg.push_back(parent_header.into());
    zmq_msg.push_back(metadata.into());
    zmq_msg.push_back(content.into());
    println!("==> SENDING ZMQ MSG: {:#?}", zmq_msg);
    zmq_msg
  }
}

// side effects messages sent on IOPub look lik ReplyMessages (for now?)
type SideEffectMessage = ReplyMessage;

struct PubComm {
  conn_str: String,
  session_id: String,
  hmac_key: hmac::Key,
  socket: zeromq::PubSocket,
}

// TODO(apowers313) connect and send look like traits shared with DealerComm
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

  pub async fn send(&mut self, msg: SideEffectMessage) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    println!(">>> ZMQ SENDING: {:#?}", zmq_msg);
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

  pub async fn recv(&mut self) -> Result<RequestMessage, AnyError> {
    let zmq_msg = self.socket.recv().await?;
    println!("<<< ZMQ RECEIVING: {:#?}", zmq_msg);

    hmac_verify(
      &self.hmac_key,
      zmq_msg.get(1).unwrap(),
      zmq_msg.get(2).unwrap(),
      zmq_msg.get(3).unwrap(),
      zmq_msg.get(4).unwrap(),
      zmq_msg.get(5).unwrap(),
    )?;

    let jup_msg = RequestMessage::try_from(zmq_msg)?;

    Ok(jup_msg)
  }

  pub async fn send(&mut self, msg: ReplyMessage) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    println!(">>> ZMQ SENDING: {:#?}", zmq_msg);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

// TODO(apowers313) this is the heartbeat loop now
async fn create_zmq_reply(name: &str, conn_str: &str) -> Result<(), AnyError> {
  println!("reply '{}' connection string: {}", name, conn_str);

  let mut sock = zeromq::RepSocket::new(); // TODO(apowers313) exact same as dealer, refactor
  sock.monitor();
  sock.bind(conn_str).await?;

  loop {
    let msg = sock.recv().await?;
    println!("*** '{}' got packet!", name);
  }
}

fn create_conn_str(transport: &str, ip: &str, port: u32) -> String {
  format!("{}://{}:{}", transport, ip, port)
}

fn parse_zmq_packet(data: &ZmqMessage) -> Result<(), AnyError> {
  let _delim = data.get(0);
  let _hmac = data.get(1);
  let header = data.get(2).unwrap();
  let _parent_header = data.get(3);
  let _metadata = data.get(4);
  let _content = data.get(5);

  let header_str = std::str::from_utf8(header).unwrap();
  let header_value: MessageHeader = serde_json::from_str(header_str).unwrap();

  Ok(())
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

#[derive(Debug, Serialize, Deserialize)]
enum RequestContent {
  Empty,
  Execute(ExecuteRequestContent),
}

#[derive(Debug, Serialize, Deserialize)]
enum ReplyContent {
  Empty,
  // Reply Messages
  KernelInfo(KernelInfoReplyContent),
  ExecuteReply(ExecuteReplyContent),
  // Side Effects
  Status(KernelStatusContent),
  Stream(StreamContent),
  ExecuteInput(ExecuteInputContent),
  ExecuteResult(ExecuteResultContent),
}

#[derive(Debug, Serialize, Deserialize)]
enum RequestMetadata {
  Empty,
  Unknown(Value),
}

#[derive(Debug, Serialize, Deserialize)]
enum ReplyMetadata {
  Empty,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
#[derive(Debug, Serialize, Deserialize)]
struct ExecuteRequestContent {
  code: String,
  silent: bool,
  store_history: bool,
  user_expressions: Value,
  allow_stdin: bool,
  stop_on_error: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
#[derive(Debug, Serialize, Deserialize)]
struct ExecuteReplyContent {
  status: String,
  execution_count: u32,
  // TODO: "None" gets translated to "null"
  // payload: Option<Vec<String>>,
  // user_expressions: Option<Value>,
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
#[derive(Debug, Serialize, Deserialize)]
struct KernelInfoReplyContent {
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
#[derive(Debug, Serialize, Deserialize)]
struct KernelLanguageInfo {
  name: String,
  version: String,
  mimetype: String,
  file_extension: String,
  // TODO: "None" gets translated to "null"
  // pygments_lexer: Option<String>,
  // codemirror_mode: Option<Value>,
  // nbconvert_exporter: Option<String>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Debug, Serialize, Deserialize)]
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
// #[derive(Debug, Serialize, Deserialize)]
// struct StatusContent {
//   status: String, // "ok" | "abort"
// }

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
#[derive(Debug, Serialize, Deserialize)]
struct StreamContent {
  name: String, // "stdout" | "stderr"
  text: String,
}

enum StdioType {
  Stdout,
  Stderr,
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
#[derive(Debug, Serialize, Deserialize)]
struct ExecuteInputContent {
  code: String,
  execution_count: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
