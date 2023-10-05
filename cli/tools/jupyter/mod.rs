// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::ops;
use crate::tools::jupyter::server::StdioMsg;
use crate::tools::repl;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use tokio::sync::mpsc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedSender;

use super::test::reporters::PrettyTestReporter;
use super::test::TestEvent;
use super::test::TestEventSender;

mod install;
pub(crate) mod jupyter_msg;
pub(crate) mod server;

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
  let (stdio_tx, stdio_rx) = mpsc::unbounded_channel();

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
  let (test_event_sender, test_event_receiver) =
    unbounded_channel::<TestEvent>();
  let test_event_sender = TestEventSender::new(test_event_sender);
  let stdout = StdioPipe::File(test_event_sender.stdout());
  let stderr = StdioPipe::File(test_event_sender.stderr());
  let mut worker = worker_factory
    .create_custom_worker(
      main_module.clone(),
      permissions,
      vec![
        ops::jupyter::deno_jupyter::init_ops(stdio_tx.clone()),
        ops::testing::deno_test::init_ops(test_event_sender.clone()),
      ],
      // FIXME(nayeemrmn): Test output capturing currently doesn't work.
      Stdio {
        stdin: StdioPipe::Inherit,
        stdout,
        stderr,
      },
    )
    .await?;
  worker.setup_repl().await?;
  worker.execute_script_static(
    located_script_name!(),
    "Deno[Deno.internal].enableJupyter();",
  )?;
  let worker = worker.into_main_worker();
  let mut repl_session = repl::ReplSession::initialize(
    cli_options,
    npm_resolver,
    resolver,
    worker,
    main_module,
    test_event_sender,
    test_event_receiver,
  )
  .await?;
  struct TestWriter(UnboundedSender<StdioMsg>);
  impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      self
        .0
        .send(StdioMsg::Stdout(String::from_utf8_lossy(buf).into_owned()))
        .ok();
      Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
      Ok(())
    }
  }
  repl_session.set_test_reporter_factory(Box::new(move || {
    Box::new(
      PrettyTestReporter::new(false, true, false, true)
        .with_writer(Box::new(TestWriter(stdio_tx.clone()))),
    )
  }));

  server::JupyterServer::start(spec, stdio_rx, repl_session).await?;

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
