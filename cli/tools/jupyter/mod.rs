// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::tools::repl;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::op;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::Op;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use ring::hmac;

mod comm;
mod install;
mod message_types;

use comm::DealerComm;
use comm::HbComm;
use comm::PubComm;
pub use install::install;
use message_types::*;

pub async fn kernel(
  flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  let Some(connection_filepath) = jupyter_flags.conn_file else {
    return Err(generic_error("Missing --conn flag"));
  };

  // This env var might be set by notebook
  if std::env::var("DEBUG").is_ok() {
    logger::init(Some(log::Level::Debug));
  }

  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let main_module =
    resolve_url_or_path("./$deno$jupyter.ts", cli_options.initial_cwd())
      .unwrap();
  // TODO(bartlomieju): should we run with all permissions?
  let permissions = PermissionsContainer::new(Permissions::allow_all());
  let npm_resolver = factory.npm_resolver().await?.clone();
  let resolver = factory.resolver().await?.clone();
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let (stdio_tx, stdio_rx) = mpsc::unbounded();

  let mut worker = worker_factory
    .create_custom_worker(
      main_module.clone(),
      permissions,
      vec![deno_jupyter::init_ops(stdio_tx)],
      Default::default(),
    )
    .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let repl_session =
    repl::ReplSession::initialize(cli_options, npm_resolver, resolver, worker)
      .await?;

  let mut kernel =
    Kernel::new(&connection_filepath, stdio_rx, repl_session).await?;
  println!("[DENO] kernel created: {:#?}", kernel.identity);

  println!("running kernel...");
  // TODO(bartlomieju): handle the result
  let _r = kernel.run().await;
  println!("done running kernel.");

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KernelState {
  Busy,
  Idle,

  // TODO(bartlomieju):
  #[allow(unused)]
  Starting,
}

struct Kernel {
  metadata: KernelMetadata,
  // TODO(bartlomieju):
  #[allow(unused)]
  conn_spec: ConnectionSpec,
  state: KernelState,
  iopub_comm: PubComm,
  shell_comm: DealerComm,
  control_comm: DealerComm,
  stdin_comm: DealerComm,
  hb_comm: HbComm,
  identity: String,
  execution_count: u32,
  repl_session: repl::ReplSession,
  stdio_rx: mpsc::UnboundedReceiver<WorkerCommMsg>,
  last_comm_ctx: Option<CommContext>,
}

#[derive(Copy, Clone, Debug)]
enum HandlerType {
  Shell,
  Control,
  Stdin,
}

pub enum WorkerCommMsg {
  Stderr(String),
  Stdout(String),
}

deno_core::extension!(deno_jupyter,
  options = {
    sender: mpsc::UnboundedSender<WorkerCommMsg>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print::DECL,
    _ => op,
  },
  state = |state, options| {
    state.put(options.sender);
  },
);

#[op]
pub fn op_print(
  state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<WorkerCommMsg>>();

  // TODO(bartlomieju): should these results be handled somehow?
  if is_err {
    let _r = sender.unbounded_send(WorkerCommMsg::Stderr(msg));
  } else {
    let _r = sender.unbounded_send(WorkerCommMsg::Stdout(msg));
  }
  Ok(())
}

impl Kernel {
  async fn new(
    connection_filepath: &Path,
    stdio_rx: mpsc::UnboundedReceiver<WorkerCommMsg>,
    repl_session: repl::ReplSession,
  ) -> Result<Self, AnyError> {
    let conn_file =
      std::fs::read_to_string(connection_filepath).with_context(|| {
        format!("Couldn't read connection file: {:?}", connection_filepath)
      })?;
    let spec: ConnectionSpec =
      serde_json::from_str(&conn_file).with_context(|| {
        format!(
          "Connection file is not a valid JSON: {:?}",
          connection_filepath
        )
      })?;

    println!("[DENO] parsed conn file: {:#?}", spec);

    let identity = uuid::Uuid::new_v4().to_string();
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, spec.key.as_ref());

    let kernel = Self {
      metadata: KernelMetadata::default(),
      state: KernelState::Idle,
      iopub_comm: PubComm::new(&spec, &identity, &hmac_key),
      shell_comm: DealerComm::create_shell(&spec, &identity, &hmac_key),
      control_comm: DealerComm::create_control(&spec, &identity, &hmac_key),
      stdin_comm: DealerComm::create_stdin(&spec, &identity, &hmac_key),
      hb_comm: HbComm::new(&spec),
      identity,
      execution_count: 0,
      repl_session,
      stdio_rx,
      last_comm_ctx: None,
      conn_spec: spec,
    };

    Ok(kernel)
  }

  async fn run(&mut self) -> Result<(), AnyError> {
    println!("Connecting to iopub");
    self.iopub_comm.connect().await?;
    println!("Connected to iopub");
    println!("Connecting to shell");
    self.shell_comm.connect().await?;
    println!("Connected to shell");
    println!("Connecting to control");
    self.control_comm.connect().await?;
    println!("Connected to control");
    println!("Connecting to stdin");
    self.stdin_comm.connect().await?;
    println!("Connected to stdin");
    println!("Connecting to heartbeat");
    self.hb_comm.connect().await?;
    println!("Connected to heartbeat");

    let mut poll_worker = true;
    loop {
      tokio::select! {
        shell_msg_result = self.shell_comm.recv() => {
          self.handler(HandlerType::Shell, shell_msg_result).await;
          poll_worker = true;
        },
        control_msg_result = self.control_comm.recv() => {
          self.handler(HandlerType::Control, control_msg_result).await;
          poll_worker = true;
        },
        stdin_msg_result = self.stdin_comm.recv() => {
          self.handler(HandlerType::Stdin, stdin_msg_result).await;
          poll_worker = true;
        },
        maybe_stdio_proxy_msg = self.stdio_rx.next() => {
          if let Some(stdio_proxy_msg) = maybe_stdio_proxy_msg {
            // TODO(bartlomieju): should the result be handled?
            let _ = self.worker_comm_handler(stdio_proxy_msg).await;
          }
        },
        heartbeat_result = self.hb_comm.heartbeat() => {
          if let Err(e) = heartbeat_result {
            println!("[heartbeat] error: {}", e);
          }
        },
        _ = self.repl_session.run_event_loop(), if poll_worker => {
          poll_worker = false;
        }
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

    let comm_ctx = CommContext { message: req_msg };
    self.last_comm_ctx = Some(comm_ctx.clone());

    println!("[DENO] set_state busy {:#?}", handler_type);
    self.set_state(&comm_ctx, KernelState::Busy).await;

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

    println!("[DENO] set_state idle {:#?}", handler_type);
    self.set_state(&comm_ctx, KernelState::Idle).await;
  }

  async fn worker_comm_handler(
    &mut self,
    worker_msg: WorkerCommMsg,
  ) -> Result<(), AnyError> {
    let comm_ctx = match self.last_comm_ctx.clone() {
      Some(cc) => cc,
      None => {
        return Err(anyhow!(
          "Received stdio message, but there is no last CommContext"
        ));
      }
    };

    match worker_msg {
      WorkerCommMsg::Stdout(s) => {
        self
          .send_stdio(&comm_ctx, StdioType::Stdout, s.as_ref())
          .await?;
      }
      WorkerCommMsg::Stderr(s) => {
        self
          .send_stdio(&comm_ctx, StdioType::Stderr, s.as_ref())
          .await?;
      }
    };

    Ok(())
  }

  async fn shell_handler(
    &mut self,
    comm_ctx: &CommContext,
  ) -> Result<(), AnyError> {
    let msg_type = comm_ctx.message.header.msg_type.as_ref();
    let result = match msg_type {
      "kernel_info_request" => self.kernel_info_reply(comm_ctx).await,
      "execute_request" => self.execute_request(comm_ctx).await,
      // "inspect_request" => self.inspect_request(comm_ctx).await,
      // "complete_request" => self.complete_request(comm_ctx).await,
      // "history_request" => self.history_request(comm_ctx).await,
      // "is_complete_request" => self.is_complete_request(comm_ctx).await,
      // "comm_info_request" => self.comm_info_request(comm_ctx).await,
      _ => {
        println!("[shell] no handler for {}", msg_type);
        Ok(())
      }
    };

    if let Err(e) = result {
      println!("[shell] error handling {}: {}", msg_type, e);
    } else {
      println!("[shell] ok {}", msg_type);
    }

    Ok(())
  }

  fn control_handler(&self, comm_ctx: &CommContext) -> Result<(), AnyError> {
    eprintln!("Unimplemented control_handler {:#?}", comm_ctx);
    Ok(())
  }

  fn stdin_handler(&self, comm_ctx: &CommContext) -> Result<(), AnyError> {
    eprintln!("Unimplemented stdin_handler {:#?}", comm_ctx);
    Ok(())
  }

  async fn set_state(&mut self, comm_ctx: &CommContext, state: KernelState) {
    if self.state == state {
      println!("[DENO] set_state sets the same state: {:#?}", state);
      return;
    }

    self.state = state;

    let s = match state {
      KernelState::Busy => "busy",
      KernelState::Idle => "idle",
      KernelState::Starting => "starting",
    };

    let msg = SideEffectMessage::new(
      comm_ctx,
      "status",
      ReplyMetadata::Empty,
      ReplyContent::Status(KernelStatusContent {
        execution_state: s.to_string(),
      }),
    );

    if let Err(e) = self.iopub_comm.send(msg).await {
      println!("[IoPub] Error setting state: {}, reason: {}", s, e);
    }
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
      },
      help_links: vec![KernelHelpLink {
        text: self.metadata.help_text.clone(),
        url: self.metadata.help_url.clone(),
      }],
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

    let evaluate_response = self
      .repl_session
      .evaluate_line_with_object_wrapping(&exec_request_content.code)
      .await?;

    let repl::cdp::EvaluateResponse {
      result,
      exception_details,
    } = evaluate_response.value;

    let exec_result = if let Some(exception_details) = exception_details {
      let name = if let Some(exception) = exception_details.exception {
        if let Some(description) = exception.description {
          description
        } else if let Some(value) = exception.value {
          value.to_string()
        } else {
          "undefined".to_string()
        }
      } else {
        "Unknown exception".to_string()
      };
      let e = ExecError {
        err_name: name,
        err_value: "".to_string(),
        stack_trace: vec![],
      };

      println!("sending exec reply error {}", self.execution_count);
      let msg = ReplyMessage::new(
        comm_ctx,
        "execute_reply",
        ReplyMetadata::Empty,
        ReplyContent::ExecuteError(ExecuteErrorContent {
          execution_count: self.execution_count,
          status: "error".to_string(),
          payload: vec![],
          user_expressions: json!({}),
          // TODO(apowers313) implement error messages
          ename: e.err_name.clone(),
          evalue: e.err_value.clone(),
          traceback: e.stack_trace.clone(),
        }),
      );
      self.shell_comm.send(msg).await?;
      let msg = SideEffectMessage::new(
        comm_ctx,
        "error",
        ReplyMetadata::Empty,
        ReplyContent::Error(ErrorContent {
          ename: e.err_name.clone(),
          evalue: e.err_value.clone(),
          traceback: e.stack_trace.clone(),
        }),
      );
      self.iopub_comm.send(msg).await?;
    } else {
      let output = self.repl_session.get_eval_value(&result).await?;
      println!("sending exec result {}", self.execution_count);
      let msg = ReplyMessage::new(
        comm_ctx,
        "execute_reply",
        ReplyMetadata::Empty,
        ReplyContent::ExecuteReply(ExecuteReplyContent {
          status: "ok".to_string(),
          execution_count: self.execution_count,
          // NOTE(bartlomieju): these two fields are always empty
          payload: vec![],
          user_expressions: json!({}),
        }),
      );
      self.shell_comm.send(msg).await?;

      let msg = SideEffectMessage::new(
        comm_ctx,
        "execute_result",
        ReplyMetadata::Empty,
        ReplyContent::ExecuteResult(ExecuteResultContent {
          execution_count: self.execution_count,
          data: json!({
            "text/plain": output,
          }),
          // data: json!("<not implemented>"),
          metadata: json!({}),
        }),
      );
      self.iopub_comm.send(msg).await?;
    };

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
  Ok(String),
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
      help_text: "Visit Deno manual".to_string(),
      help_url: "https://deno.land/manual".to_string(),
      implementation_name: "Deno kernel".to_string(),
      kernel_version: crate::version::deno().to_string(),
      language_version: crate::version::TYPESCRIPT.to_string(),
      language: "typescript".to_string(),
      mime: "text/x.typescript".to_string(),
      protocol_version: "5.3".to_string(),
    }
  }
}

#[derive(Debug, Deserialize)]
pub struct ConnectionSpec {
  ip: String,
  transport: String,
  control_port: u32,
  shell_port: u32,
  stdin_port: u32,
  hb_port: u32,
  iopub_port: u32,
  key: String,

  #[allow(unused)]
  signature_scheme: String,
}

#[derive(Debug)]
enum StdioType {
  Stdout,
  Stderr,
}
