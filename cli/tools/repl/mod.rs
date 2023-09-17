// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::ReplFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::unsync::spawn_blocking;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use rustyline::error::ReadlineError;

pub(crate) mod cdp;
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
pub use session::REPL_INTERNALS_NAME;

#[allow(clippy::await_holding_refcell_ref)]
async fn read_line_and_poll(
  repl_session: &mut ReplSession,
  message_handler: &mut RustylineSyncMessageHandler,
  editor: ReplEditor,
) -> Result<String, ReadlineError> {
  let mut line_fut = spawn_blocking(move || editor.readline());
  let mut poll_worker = true;
  let notifications_rc = repl_session.notifications.clone();
  let mut notifications = notifications_rc.borrow_mut();

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
          let method = message.get("method").unwrap().as_str().unwrap();
          if method == "Runtime.exceptionThrown" {
            let params = message.get("params").unwrap().as_object().unwrap();
            let exception_details = params.get("exceptionDetails").unwrap().as_object().unwrap();
            let text = exception_details.get("text").unwrap().as_str().unwrap();
            let exception = exception_details.get("exception").unwrap().as_object().unwrap();
            let description = exception.get("description").and_then(|d| d.as_str()).unwrap_or("undefined");
            println!("{text} {description}");
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
) -> Result<String, AnyError> {
  let specifier =
    deno_core::resolve_url_or_path(eval_file, cli_options.initial_cwd())?;

  let file = file_fetcher
    .fetch(&specifier, PermissionsContainer::allow_all())
    .await?;

  Ok((*file.source).to_string())
}

pub async fn run(flags: Flags, repl_flags: ReplFlags) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let main_module = cli_options.resolve_main_module()?;
  let permissions = PermissionsContainer::new(Permissions::from_options(
    &cli_options.permissions_options(),
  )?);
  let npm_resolver = factory.npm_resolver().await?.clone();
  let resolver = factory.resolver().await?.clone();
  let file_fetcher = factory.file_fetcher()?;
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let history_file_path = factory
    .deno_dir()
    .ok()
    .and_then(|dir| dir.repl_history_file_path());

  let mut worker = worker_factory
    .create_main_worker(main_module, permissions)
    .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let mut repl_session =
    ReplSession::initialize(cli_options, npm_resolver, resolver, worker)
      .await?;
  let mut rustyline_channel = rustyline_channel();

  let helper = EditorHelper {
    context_id: repl_session.context_id,
    sync_sender: rustyline_channel.0,
  };

  let editor = ReplEditor::new(helper, history_file_path)?;

  if let Some(eval_files) = repl_flags.eval_files {
    for eval_file in eval_files {
      match read_eval_file(cli_options, file_fetcher, &eval_file).await {
        Ok(eval_source) => {
          let output = repl_session
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
    let output = repl_session.evaluate_line_and_get_output(&eval).await;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("Error in --eval flag: {error_text}");
    }
  }

  // Doing this manually, instead of using `log::info!` because these messages
  // are supposed to go to stdout, not stderr.
  if !cli_options.is_quiet() {
    println!("Deno {}", crate::version::deno());
    println!("exit using ctrl+d, ctrl+c, or close()");
    if repl_flags.is_default_command {
      println!(
        "{}",
        colors::yellow("REPL is running with all permissions allowed.")
      );
      println!("To specify permissions, run `deno repl` with allow flags.")
    }
  }

  loop {
    let line = read_line_and_poll(
      &mut repl_session,
      &mut rustyline_channel.1,
      editor.clone(),
    )
    .await;
    match line {
      Ok(line) => {
        editor.set_should_exit_on_interrupt(false);
        editor.update_history(line.clone());
        let output = repl_session.evaluate_line_and_get_output(&line).await;

        // We check for close and break here instead of making it a loop condition to get
        // consistent behavior in when the user evaluates a call to close().
        if repl_session.closing().await? {
          break;
        }

        println!("{output}");
      }
      Err(ReadlineError::Interrupted) => {
        if editor.should_exit_on_interrupt() {
          break;
        }
        editor.set_should_exit_on_interrupt(true);
        println!("press ctrl+c again to exit");
        continue;
      }
      Err(ReadlineError::Eof) => {
        break;
      }
      Err(err) => {
        println!("Error: {err:?}");
        break;
      }
    }
  }

  Ok(repl_session.worker.exit_code())
}
