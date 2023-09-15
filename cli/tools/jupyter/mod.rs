// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::tools::repl;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::op;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::Op;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;

mod install;
mod jupyter_msg;
mod server;

pub async fn kernel(
  flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
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

#[op]
pub fn op_print(
  state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<server::StdioMsg>>();

  // TODO(bartlomieju): should these results be handled somehow?
  if is_err {
    let _r = sender.unbounded_send(server::StdioMsg::Stderr(msg));
  } else {
    let _r = sender.unbounded_send(server::StdioMsg::Stdout(msg));
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
