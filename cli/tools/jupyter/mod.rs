// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::ops;
use crate::tools::repl;
use crate::tools::test::create_single_test_event_channel;
use crate::tools::test::reporters::PrettyTestReporter;
use crate::tools::test::TestEventWorkerSender;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::WorkerExecutionMode;
use deno_terminal::colors;

use jupyter_runtime::jupyter::ConnectionInfo;
use jupyter_runtime::messaging::StreamContent;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;

mod install;
pub mod server;

pub async fn kernel(
  flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  log::info!(
    "{} \"deno jupyter\" is unstable and might change in the future.",
    colors::yellow("Warning"),
  );

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

  let factory = CliFactory::from_flags(flags)?;
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
  let spec: ConnectionInfo =
    serde_json::from_str(&conn_file).with_context(|| {
      format!(
        "Connection file is not a valid JSON: {:?}",
        connection_filepath
      )
    })?;
  let (worker, test_event_receiver) = create_single_test_event_channel();
  let TestEventWorkerSender {
    sender: test_event_sender,
    stdout,
    stderr,
  } = worker;

  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Jupyter,
      main_module.clone(),
      permissions,
      vec![
        ops::jupyter::deno_jupyter::init_ops(stdio_tx.clone()),
        ops::testing::deno_test::init_ops(test_event_sender),
      ],
      // FIXME(nayeemrmn): Test output capturing currently doesn't work.
      Stdio {
        stdin: StdioPipe::inherit(),
        stdout: StdioPipe::file(stdout),
        stderr: StdioPipe::file(stderr),
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
    test_event_receiver,
  )
  .await?;
  struct TestWriter(UnboundedSender<StreamContent>);
  impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      self
        .0
        .send(StreamContent::stdout(
          String::from_utf8_lossy(buf).into_owned(),
        ))
        .ok();
      Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
      Ok(())
    }
  }
  let cwd_url =
    Url::from_directory_path(cli_options.initial_cwd()).map_err(|_| {
      generic_error(format!(
        "Unable to construct URL from the path of cwd: {}",
        cli_options.initial_cwd().to_string_lossy(),
      ))
    })?;
  repl_session.set_test_reporter_factory(Box::new(move || {
    Box::new(
      PrettyTestReporter::new(false, true, false, true, cwd_url.clone())
        .with_writer(Box::new(TestWriter(stdio_tx.clone()))),
    )
  }));

  server::JupyterServer::start(spec, stdio_rx, repl_session).await?;

  Ok(())
}
