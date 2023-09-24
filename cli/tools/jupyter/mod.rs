// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::tools::repl;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::op2;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::Op;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use tokio::sync::Mutex;

mod install;
mod jupyter_msg;
mod server;

use jupyter_msg::Connection;
use jupyter_msg::JupyterMessage;

pub async fn kernel(
  flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  if !flags.unstable {
    eprintln!(
      "Unstable subcommand 'deno jupyter'. The --unstable flag must be provided."
    );
    std::process::exit(70);
  }

  if !jupyter_flags.install && !jupyter_flags.kernel {
    install::status()?;
    return Ok(());
  }

  if jupyter_flags.install {
    install::install()?;
    return Ok(());
  }

  let connection_filepath = jupyter_flags.conn_file.unwrap();

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

  let conn_file =
    std::fs::read_to_string(&connection_filepath).with_context(|| {
      format!("Couldn't read connection file: {:?}", connection_filepath)
    })?;
  let spec: ConnectionSpec =
    serde_json::from_str(&conn_file).with_context(|| {
      format!(
        "Connection file is not a valid JSON: {:?}",
        connection_filepath
      )
    })?;

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

  server::JupyterServer::start(spec, stdio_rx, repl_session).await?;

  Ok(())
}

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_send_io,
  ],
  options = {
    sender: mpsc::UnboundedSender<server::StdioMsg>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print::DECL,
    _ => op,
  },
  state = |state, options| {
    state.put(options.sender);
  },
);

#[op2(async)]
pub async fn op_jupyter_send_io(
  state: Rc<RefCell<OpState>>,
  #[serde] content: serde_json::Value,
) -> Result<(), AnyError> {
  let (iopub_socket, last_execution_request) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<Connection<zeromq::PubSocket>>>>()
        .clone(),
      s.borrow::<Rc<RefCell<Option<JupyterMessage>>>>().clone(),
    )
  };

  if let Some(last_request) = last_execution_request.borrow().clone() {
    last_request
      .new_message("display_data")
      .with_content(content)
      .send(&mut *iopub_socket.lock().await)
      .await?;
  }

  Ok(())
}

#[op2(fast)]
pub fn op_print(
  state: &mut OpState,
  #[string] msg: &str,
  is_err: bool,
) -> Result<(), AnyError> {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<server::StdioMsg>>();

  if is_err {
    if let Err(err) =
      sender.unbounded_send(server::StdioMsg::Stderr(msg.into()))
    {
      eprintln!("Failed to send stderr message: {}", err);
    }
    return Ok(());
  }

  if let Err(err) = sender.unbounded_send(server::StdioMsg::Stdout(msg.into()))
  {
    eprintln!("Failed to send stdout message: {}", err);
  }
  Ok(())
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
}
