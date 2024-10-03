// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io;
use std::io::Write;

use std::sync::Arc;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::ReplFlags;
use crate::cdp;
use crate::colors;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::unsync::spawn_blocking;
use deno_runtime::WorkerExecutionMode;
use rustyline::error::ReadlineError;

mod channel;
mod editor;
mod session;

use channel::rustyline_channel;
use channel::RustylineSyncMessage;
use channel::RustylineSyncMessageHandler;
use channel::RustylineSyncResponse;
use editor::EditorHelper;
use editor::ReplEditor;
pub use session::EvaluationOutput;
pub use session::ReplSession;
pub use session::TsEvaluateResponse;
pub use session::REPL_INTERNALS_NAME;

use super::test::create_single_test_event_channel;

struct Repl {
  session: ReplSession,
  editor: ReplEditor,
  message_handler: RustylineSyncMessageHandler,
}

#[allow(clippy::print_stdout)]
impl Repl {
  async fn run(&mut self) -> Result<(), AnyError> {
    loop {
      let line = read_line_and_poll(
        &mut self.session,
        &mut self.message_handler,
        self.editor.clone(),
      )
      .await;
      match line {
        Ok(line) => {
          self.editor.set_should_exit_on_interrupt(false);
          self.editor.update_history(line.clone());
          let output = self.session.evaluate_line_and_get_output(&line).await;

          // We check for close and break here instead of making it a loop condition to get
          // consistent behavior in when the user evaluates a call to close().
          if self.session.closing().await? {
            break;
          }

          println!("{}", output);
        }
        Err(ReadlineError::Interrupted) => {
          if self.editor.should_exit_on_interrupt() {
            break;
          }
          self.editor.set_should_exit_on_interrupt(true);
          println!("press ctrl+c again to exit");
          continue;
        }
        Err(ReadlineError::Eof) => {
          break;
        }
        Err(err) => {
          println!("Error: {:?}", err);
          break;
        }
      }
    }

    Ok(())
  }
}

#[allow(clippy::print_stdout)]
async fn read_line_and_poll(
  repl_session: &mut ReplSession,
  message_handler: &mut RustylineSyncMessageHandler,
  editor: ReplEditor,
) -> Result<String, ReadlineError> {
  let mut line_fut = spawn_blocking(move || editor.readline());
  let mut poll_worker = true;
  let notifications_rc = repl_session.notifications.clone();
  let mut notifications = notifications_rc.lock().await;

  loop {
    tokio::select! {
      result = &mut line_fut => {
        return result.unwrap();
      }
      result = message_handler.recv() => {
        match result {
          Some(RustylineSyncMessage::PostMessage { method, params }) => {
            let result = repl_session
              .post_message_with_event_loop(&method, params)
              .await;
            message_handler.send(RustylineSyncResponse::PostMessage(result)).unwrap();
          },
          Some(RustylineSyncMessage::LspCompletions {
            line_text,
            position,
          }) => {
            let result = repl_session.language_server.completions(&line_text, position).await;
            message_handler.send(RustylineSyncResponse::LspCompletions(result)).unwrap();
          }
          None => {}, // channel closed
        }

        poll_worker = true;
      }
      message = notifications.next() => {
        if let Some(message) = message {
          let notification: cdp::Notification = serde_json::from_value(message).unwrap();
          if notification.method == "Runtime.exceptionThrown" {
            let exception_thrown: cdp::ExceptionThrown = serde_json::from_value(notification.params).unwrap();
            let (message, description) = exception_thrown.exception_details.get_message_and_description();
            println!("{} {}", message, description);
          }
        }
      }
      _ = repl_session.run_event_loop(), if poll_worker => {
        poll_worker = false;
      }
    }
  }
}

async fn read_eval_file(
  cli_options: &CliOptions,
  file_fetcher: &FileFetcher,
  eval_file: &str,
) -> Result<Arc<str>, AnyError> {
  let specifier =
    deno_core::resolve_url_or_path(eval_file, cli_options.initial_cwd())?;

  let file = file_fetcher.fetch_bypass_permissions(&specifier).await?;

  Ok(file.into_text_decoded()?.source)
}

#[allow(clippy::print_stdout)]
pub async fn run(
  flags: Arc<Flags>,
  repl_flags: ReplFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let main_module = cli_options.resolve_main_module()?;
  let permissions = factory.root_permissions_container()?;
  let npm_resolver = factory.npm_resolver().await?.clone();
  let resolver = factory.resolver().await?.clone();
  let file_fetcher = factory.file_fetcher()?;
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let history_file_path = factory
    .deno_dir()
    .ok()
    .and_then(|dir| dir.repl_history_file_path());
  let (worker, test_event_receiver) = create_single_test_event_channel();
  let test_event_sender = worker.sender;
  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Repl,
      main_module.clone(),
      permissions.clone(),
      vec![crate::ops::testing::deno_test::init_ops(test_event_sender)],
      Default::default(),
    )
    .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let session = ReplSession::initialize(
    cli_options,
    npm_resolver,
    resolver,
    worker,
    main_module.clone(),
    test_event_receiver,
  )
  .await?;
  let rustyline_channel = rustyline_channel();

  let helper = EditorHelper {
    context_id: session.context_id,
    sync_sender: rustyline_channel.0,
  };

  let editor = ReplEditor::new(helper, history_file_path)?;

  let mut repl = Repl {
    session,
    editor,
    message_handler: rustyline_channel.1,
  };

  if let Some(eval_files) = repl_flags.eval_files {
    for eval_file in eval_files {
      match read_eval_file(cli_options, file_fetcher, &eval_file).await {
        Ok(eval_source) => {
          let output = repl
            .session
            .evaluate_line_and_get_output(&eval_source)
            .await;
          // only output errors
          if let EvaluationOutput::Error(error_text) = output {
            println!("Error in --eval-file file \"{eval_file}\": {error_text}");
          }
        }
        Err(e) => {
          println!("Error in --eval-file file \"{eval_file}\": {e}");
        }
      }
    }
  }

  if let Some(eval) = repl_flags.eval {
    let output = repl.session.evaluate_line_and_get_output(&eval).await;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("Error in --eval flag: {error_text}");
    }
  }

  // Doing this manually, instead of using `log::info!` because these messages
  // are supposed to go to stdout, not stderr.
  // Using writeln, because println panics in certain cases
  // (eg: broken pipes - https://github.com/denoland/deno/issues/21861)
  if !cli_options.is_quiet() {
    let mut handle = io::stdout().lock();

    writeln!(handle, "Deno {}", crate::version::DENO_VERSION_INFO.deno)?;
    writeln!(handle, "exit using ctrl+d, ctrl+c, or close()")?;

    if repl_flags.is_default_command {
      writeln!(
        handle,
        "{}",
        colors::yellow("REPL is running with all permissions allowed.")
      )?;
      writeln!(
        handle,
        "To specify permissions, run `deno repl` with allow flags."
      )?;
    }
  }

  repl.run().await?;

  Ok(repl.session.worker.exit_code())
}
