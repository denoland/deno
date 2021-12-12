// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use std::env::current_exe;
use tempfile::TempDir;
use tokio::join;
use zeromq::prelude::*;
use zeromq::ZmqMessage;

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
  let conn_file_path = jupyter_flags.conn_file.unwrap();

  let conn_file = std::fs::read_to_string(conn_file_path)
    .context("Failed to read connection file")?;
  let conn_spec: ConnectionSpec = serde_json::from_str(&conn_file)
    .context("Failed to parse connection file")?;
  eprintln!("[DENO] parsed conn file: {:#?}", conn_spec);

  let metadata = KernelMetadata::default();
  let iopub_comm = PubComm::new(
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.iopub_port),
    metadata.session_id.clone(),
    conn_spec.key.clone(),
  );
  let shell_comm = DealerComm::new(
    "shell",
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.shell_port),
    metadata.session_id.clone(),
    conn_spec.key.clone(),
  );
  let control_comm = DealerComm::new(
    "control",
    create_conn_str(
      &conn_spec.transport,
      &conn_spec.ip,
      conn_spec.control_port,
    ),
    metadata.session_id.clone(),
    conn_spec.key.clone(),
  );
  let stdin_comm = DealerComm::new(
    "stdin",
    create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.stdin_port),
    metadata.session_id.clone(),
    conn_spec.key.clone(),
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
          eprintln!("shell got packet: {:#?}", shell_msg);
        },
        control_msg = self.control_comm.recv() => {
          eprintln!("control got packet: {:#?}", control_msg);
        },
        stdin_msg = self.stdin_comm.recv() => {
          eprintln!("stdin got packet: {:#?}", stdin_msg);
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

#[derive(Debug, Deserialize)]
struct MessageHeader {
  msg_id: String,
  session: String,
  username: String,
  date: String,
  msg_type: String,
  version: String,
}

struct Message {
  is_reply: bool,
  r#type: String,
  header: MessageHeader,
  parent_header: Option<MessageHeader>,
  content: Value,
  session_id: String,
}

impl Message {
  fn new() -> Self {
    todo!()
  }

  fn calc_hmac(&self, hmac: String) -> String {
    todo!()
  }

  fn serialize(&self, hmac: String) -> Vec<u8> {
    todo!()
  }

  fn hmac_verify(&self, expected_signature: Vec<u8>, hmac: String) {
    todo!()
  }

  fn from_data(data: Vec<u8>, hmac: String) -> Result<Self, AnyError> {
    todo!()
  }
}

struct PubComm {
  conn_str: String,
  session_id: String,
  hmac_key: String,
  socket: zeromq::PubSocket,
}

impl PubComm {
  pub fn new(conn_str: String, session_id: String, hmac_key: String) -> Self {
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
    let msg_str = msg.serialize(self.hmac_key.clone());
    self.socket.send(msg_str.into()).await?;
    Ok(())
  }
}

struct DealerComm {
  name: String,
  conn_str: String,
  session_id: String,
  hmac_key: String,
  socket: zeromq::DealerSocket,
}

impl DealerComm {
  pub fn new(
    name: &str,
    conn_str: String,
    session_id: String,
    hmac_key: String,
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
    let msg_str = msg.serialize(self.hmac_key.clone());
    self.socket.send(msg_str.into()).await?;
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

fn parse_zmq_packet(data: &ZmqMessage) -> Result<(), AnyError> {
  let _delim = data.get(0);
  let _hmac = data.get(1);
  let header = data.get(2).unwrap();
  let _parent_header = data.get(3);
  let _metadata = data.get(4);
  let _content = data.get(5);

  println!("header:");
  dbg!(header);
  let header_str = std::str::from_utf8(&header).unwrap();
  let header_value: MessageHeader = serde_json::from_str(header_str).unwrap();
  println!("header_value");
  dbg!(&header_value);
  // validate_header(&header_value)?;

  Ok(())
}
