// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(unused)]

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use crate::tools::repl::EvaluationOutput;
use crate::tools::repl::ReplSession;
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
use deno_runtime::worker::MainWorker;
use ring::hmac;
use std::collections::HashMap;
use std::env::current_exe;
use std::time::Duration;
use tempfile::TempDir;
use tokio::join;
use tokio::time::sleep;
use zeromq::prelude::*;
use zeromq::ZmqMessage;

mod comm;
mod install;
mod message_types;

use comm::create_zmq_reply;
use comm::DealerComm;
use comm::PubComm;
pub use install::install;
use message_types::*;

pub async fn kernel(
  _flags: Flags,
  jupyter_flags: JupyterFlags,
  worker: MainWorker,
) -> Result<(), AnyError> {
  if jupyter_flags.conn_file.is_none() {
    return Err(generic_error("Missing --conn flag"));
  }

  let conn_file_path = jupyter_flags.conn_file.unwrap();

  let repl_session = ReplSession::initialize(worker).await?;
  let mut kernel = Kernel::new(conn_file_path.to_str().unwrap(), repl_session);
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
  repl_session: ReplSession,
}

enum HandlerType {
  Shell,
  Control,
  Stdin,
}

impl Kernel {
  fn new(conn_file_path: &str, repl_session: ReplSession) -> Self {
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
      hmac_key,
    );

    let hb_conn_str =
      create_conn_str(&conn_spec.transport, &conn_spec.ip, conn_spec.hb_port);

    let kernel: Kernel = Self {
      metadata,
      conn_spec,
      state: KernelState::Idle,
      iopub_comm,
      shell_comm,
      control_comm,
      stdin_comm,
      session_id,
      execution_count,
      repl_session,
    };

    kernel
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
      comm_ctx,
      "status",
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
      "kernel_info_reply",
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
    self.execution_count += 1;

    let exec_request_content = match &comm_ctx.message.content {
      RequestContent::Execute(c) => c,
      _ => return Err(anyhow!("malformed execution content")),
    };

    let input_msg = SideEffectMessage::new(
      comm_ctx,
      "execute_input",
      ReplyMetadata::Empty,
      ReplyContent::ExecuteInput(ExecuteInputContent {
        code: exec_request_content.code.clone(),
        execution_count: self.execution_count,
      }),
    );
    self.iopub_comm.send(input_msg).await?;

    let output = self
      .repl_session
      .evaluate_line_and_get_output(&exec_request_content.code)
      .await?;
    let result = match output {
      EvaluationOutput::Value(value_str) => ExecResult::OkString(value_str),
      EvaluationOutput::Error(value_str) => ExecResult::Error(ExecError {
        err_name: "<TODO>".to_string(),
        err_value: value_str,
        stack_trace: vec![],
      }),
    };
    self.exec_done(comm_ctx, result).await?;

    Ok(())
  }

  async fn exec_done(
    &mut self,
    comm_ctx: &CommContext,
    result: ExecResult,
  ) -> Result<(), AnyError> {
    match &result {
      ExecResult::OkString(v) => {
        println!("sending exec result");
        let msg = ReplyMessage::new(
          comm_ctx,
          "execute_reply",
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

    let msg = SideEffectMessage::new(
      comm_ctx,
      "execute_result",
      ReplyMetadata::Empty,
      ReplyContent::ExecuteResult(ExecuteResultContent {
        execution_count: self.execution_count,
        data: Some(json!({
            "text/plain": match result {
                ExecResult::OkString(v) => v,
                ExecResult::Error(v) => v.err_value,
            }
        })),
        metadata: None,
      }),
    );
    self.iopub_comm.send(msg).await?;

    Ok(())
  }

  async fn send_stdio(
    &mut self,
    comm_ctx: &CommContext,
    t: StdioType,
    text: &str,
  ) -> Result<(), AnyError> {
    let content = StreamContent {
      name: match t {
        StdioType::Stdout => "stdout".to_string(),
        StdioType::Stderr => "stderr".to_string(),
      },
      text: text.to_string(),
    };

    let msg = SideEffectMessage::new(
      comm_ctx,
      "stream",
      ReplyMetadata::Empty,
      ReplyContent::Stream(content),
    );

    self.iopub_comm.send(msg).await?;

    Ok(())
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
      kernel_version: crate::version::deno(),
      language_version: crate::version::TYPESCRIPT.to_string(),
      language: "typescript".to_string(),
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

fn create_conn_str(transport: &str, ip: &str, port: u32) -> String {
  format!("{}://{}:{}", transport, ip, port)
}

enum StdioType {
  Stdout,
  Stderr,
}
