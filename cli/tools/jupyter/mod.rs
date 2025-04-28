// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::WorkerExecutionMode;
use deno_terminal::colors;
use jupyter_runtime::jupyter::ConnectionInfo;
use jupyter_runtime::messaging::StreamContent;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::cdp;
use crate::lsp::ReplCompletionItem;
use crate::ops;
use crate::tools::repl;
use crate::tools::test::create_single_test_event_channel;
use crate::tools::test::reporters::PrettyTestReporter;
use crate::tools::test::TestEventWorkerSender;
use crate::tools::test::TestFailureFormatOptions;
use crate::CliFactory;

mod install;
pub mod server;

pub async fn kernel(
  flags: Arc<Flags>,
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

  let factory = CliFactory::from_flags(flags);
  let registry_provider =
    Arc::new(factory.lockfile_npm_package_info_provider()?);
  let cli_options = factory.cli_options()?;
  let main_module =
    resolve_url_or_path("./$deno$jupyter.mts", cli_options.initial_cwd())
      .unwrap();
  // TODO(bartlomieju): should we run with all permissions?
  let permissions =
    PermissionsContainer::allow_all(factory.permission_desc_parser()?.clone());
  let npm_installer = factory.npm_installer_if_managed().await?.cloned();
  let tsconfig_resolver = factory.tsconfig_resolver()?;
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
    npm_installer,
    resolver,
    tsconfig_resolver,
    worker,
    main_module,
    test_event_receiver,
    registry_provider,
  )
  .await?;
  struct TestWriter(UnboundedSender<StreamContent>);
  impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      self
        .0
        .send(StreamContent::stdout(&String::from_utf8_lossy(buf)))
        .ok();
      Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
      Ok(())
    }
  }
  let cwd_url =
    Url::from_directory_path(cli_options.initial_cwd()).map_err(|_| {
      anyhow!(
        "Unable to construct URL from the path of cwd: {}",
        cli_options.initial_cwd().to_string_lossy(),
      )
    })?;
  repl_session.set_test_reporter_factory(Box::new(move || {
    Box::new(
      PrettyTestReporter::new(
        false,
        true,
        false,
        true,
        cwd_url.clone(),
        TestFailureFormatOptions::default(),
      )
      .with_writer(Box::new(TestWriter(stdio_tx.clone()))),
    )
  }));

  let (tx1, rx1) = mpsc::unbounded_channel();
  let (tx2, rx2) = mpsc::unbounded_channel();
  let (startup_data_tx, startup_data_rx) =
    oneshot::channel::<server::StartupData>();

  let mut repl_session_proxy = JupyterReplSession {
    repl_session,
    rx: rx1,
    tx: tx2,
  };
  let repl_session_proxy_channels = JupyterReplProxy { tx: tx1, rx: rx2 };

  let join_handle = std::thread::spawn(move || {
    let fut = server::JupyterServer::start(
      spec,
      stdio_rx,
      repl_session_proxy_channels,
      startup_data_tx,
    )
    .boxed_local();
    deno_runtime::tokio_util::create_and_run_current_thread(fut)
  });

  let Ok(startup_data) = startup_data_rx.await else {
    bail!("Failed to acquire startup data");
  };
  {
    let op_state_rc =
      repl_session_proxy.repl_session.worker.js_runtime.op_state();
    let mut op_state = op_state_rc.borrow_mut();
    op_state.put(startup_data.iopub_connection.clone());
    op_state.put(startup_data.last_execution_request.clone());
    op_state.put(startup_data.stdin_connection_proxy.clone());
  }

  repl_session_proxy.start().await;
  let server_result = join_handle.join();
  match server_result {
    Ok(result) => {
      result?;
    }
    Err(e) => {
      bail!("Jupyter kernel error: {:?}", e);
    }
  };

  Ok(())
}

pub enum JupyterReplRequest {
  LspCompletions {
    line_text: String,
    position: usize,
  },
  JsGetProperties {
    object_id: String,
  },
  JsEvaluate {
    expr: String,
  },
  JsGlobalLexicalScopeNames,
  JsEvaluateLineWithObjectWrapping {
    line: String,
  },
  JsCallFunctionOnArgs {
    function_declaration: String,
    args: Vec<cdp::RemoteObject>,
  },
  JsCallFunctionOn {
    arg0: cdp::CallArgument,
    arg1: cdp::CallArgument,
  },
}

pub enum JupyterReplResponse {
  LspCompletions(Vec<ReplCompletionItem>),
  JsGetProperties(Option<cdp::GetPropertiesResponse>),
  JsEvaluate(Option<cdp::EvaluateResponse>),
  JsGlobalLexicalScopeNames(cdp::GlobalLexicalScopeNamesResponse),
  JsEvaluateLineWithObjectWrapping(Result<repl::TsEvaluateResponse, AnyError>),
  JsCallFunctionOnArgs(Result<cdp::CallFunctionOnResponse, AnyError>),
  JsCallFunctionOn(Option<cdp::CallFunctionOnResponse>),
}

pub struct JupyterReplProxy {
  tx: mpsc::UnboundedSender<JupyterReplRequest>,
  rx: mpsc::UnboundedReceiver<JupyterReplResponse>,
}

impl JupyterReplProxy {
  pub async fn lsp_completions(
    &mut self,
    line_text: String,
    position: usize,
  ) -> Vec<ReplCompletionItem> {
    let _ = self.tx.send(JupyterReplRequest::LspCompletions {
      line_text,
      position,
    });
    let Some(JupyterReplResponse::LspCompletions(resp)) = self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  pub async fn get_properties(
    &mut self,
    object_id: String,
  ) -> Option<cdp::GetPropertiesResponse> {
    let _ = self
      .tx
      .send(JupyterReplRequest::JsGetProperties { object_id });
    let Some(JupyterReplResponse::JsGetProperties(resp)) = self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  pub async fn evaluate(
    &mut self,
    expr: String,
  ) -> Option<cdp::EvaluateResponse> {
    let _ = self.tx.send(JupyterReplRequest::JsEvaluate { expr });
    let Some(JupyterReplResponse::JsEvaluate(resp)) = self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  pub async fn global_lexical_scope_names(
    &mut self,
  ) -> cdp::GlobalLexicalScopeNamesResponse {
    let _ = self.tx.send(JupyterReplRequest::JsGlobalLexicalScopeNames);
    let Some(JupyterReplResponse::JsGlobalLexicalScopeNames(resp)) =
      self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  pub async fn evaluate_line_with_object_wrapping(
    &mut self,
    line: String,
  ) -> Result<repl::TsEvaluateResponse, AnyError> {
    let _ = self
      .tx
      .send(JupyterReplRequest::JsEvaluateLineWithObjectWrapping { line });
    let Some(JupyterReplResponse::JsEvaluateLineWithObjectWrapping(resp)) =
      self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  pub async fn call_function_on_args(
    &mut self,
    function_declaration: String,
    args: Vec<cdp::RemoteObject>,
  ) -> Result<cdp::CallFunctionOnResponse, AnyError> {
    let _ = self.tx.send(JupyterReplRequest::JsCallFunctionOnArgs {
      function_declaration,
      args,
    });
    let Some(JupyterReplResponse::JsCallFunctionOnArgs(resp)) =
      self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }

  // TODO(bartlomieju): rename to "broadcast_result"?
  pub async fn call_function_on(
    &mut self,
    arg0: cdp::CallArgument,
    arg1: cdp::CallArgument,
  ) -> Option<cdp::CallFunctionOnResponse> {
    let _ = self
      .tx
      .send(JupyterReplRequest::JsCallFunctionOn { arg0, arg1 });
    let Some(JupyterReplResponse::JsCallFunctionOn(resp)) =
      self.rx.recv().await
    else {
      unreachable!()
    };
    resp
  }
}

pub struct JupyterReplSession {
  repl_session: repl::ReplSession,
  rx: mpsc::UnboundedReceiver<JupyterReplRequest>,
  tx: mpsc::UnboundedSender<JupyterReplResponse>,
}

impl JupyterReplSession {
  pub async fn start(&mut self) {
    let mut poll_worker = true;
    loop {
      tokio::select! {
        biased;

        maybe_message = self.rx.recv() => {
          let Some(msg) = maybe_message else {
            break;
          };
          if self.handle_message(msg).await.is_err() {
            break;
          }
          poll_worker = true;
        },
        _ = self.repl_session.run_event_loop(), if poll_worker => {
          poll_worker = false;
        }
      }
    }
  }

  async fn handle_message(
    &mut self,
    msg: JupyterReplRequest,
  ) -> Result<(), AnyError> {
    let resp = match msg {
      JupyterReplRequest::LspCompletions {
        line_text,
        position,
      } => JupyterReplResponse::LspCompletions(
        self
          .lsp_completions(&line_text, position, CancellationToken::new())
          .await,
      ),
      JupyterReplRequest::JsGetProperties { object_id } => {
        JupyterReplResponse::JsGetProperties(
          self.get_properties(object_id).await,
        )
      }
      JupyterReplRequest::JsEvaluate { expr } => {
        JupyterReplResponse::JsEvaluate(self.evaluate(expr).await)
      }
      JupyterReplRequest::JsGlobalLexicalScopeNames => {
        JupyterReplResponse::JsGlobalLexicalScopeNames(
          self.global_lexical_scope_names().await,
        )
      }
      JupyterReplRequest::JsEvaluateLineWithObjectWrapping { line } => {
        JupyterReplResponse::JsEvaluateLineWithObjectWrapping(
          self.evaluate_line_with_object_wrapping(&line).await,
        )
      }
      JupyterReplRequest::JsCallFunctionOnArgs {
        function_declaration,
        args,
      } => JupyterReplResponse::JsCallFunctionOnArgs(
        self
          .call_function_on_args(function_declaration, &args)
          .await,
      ),
      JupyterReplRequest::JsCallFunctionOn { arg0, arg1 } => {
        JupyterReplResponse::JsCallFunctionOn(
          self.call_function_on(arg0, arg1).await,
        )
      }
    };

    self.tx.send(resp).map_err(|e| e.into())
  }

  pub async fn lsp_completions(
    &mut self,
    line_text: &str,
    position: usize,
    token: CancellationToken,
  ) -> Vec<ReplCompletionItem> {
    self
      .repl_session
      .language_server
      .completions(line_text, position, token)
      .await
  }

  pub async fn get_properties(
    &mut self,
    object_id: String,
  ) -> Option<cdp::GetPropertiesResponse> {
    let get_properties_response = self
      .repl_session
      .post_message_with_event_loop(
        "Runtime.getProperties",
        Some(cdp::GetPropertiesArgs {
          object_id,
          own_properties: None,
          accessor_properties_only: None,
          generate_preview: None,
          non_indexed_properties_only: Some(true),
        }),
      )
      .await
      .ok()?;
    serde_json::from_value(get_properties_response).ok()
  }

  pub async fn evaluate(
    &mut self,
    expr: String,
  ) -> Option<cdp::EvaluateResponse> {
    let evaluate_response: serde_json::Value = self
      .repl_session
      .post_message_with_event_loop(
        "Runtime.evaluate",
        Some(cdp::EvaluateArgs {
          expression: expr,
          object_group: None,
          include_command_line_api: None,
          silent: None,
          context_id: Some(self.repl_session.context_id),
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          throw_on_side_effect: Some(true),
          timeout: Some(200),
          disable_breaks: None,
          repl_mode: None,
          allow_unsafe_eval_blocked_by_csp: None,
          unique_context_id: None,
        }),
      )
      .await
      .ok()?;
    serde_json::from_value(evaluate_response).ok()
  }

  pub async fn global_lexical_scope_names(
    &mut self,
  ) -> cdp::GlobalLexicalScopeNamesResponse {
    let evaluate_response = self
      .repl_session
      .post_message_with_event_loop(
        "Runtime.globalLexicalScopeNames",
        Some(cdp::GlobalLexicalScopeNamesArgs {
          execution_context_id: Some(self.repl_session.context_id),
        }),
      )
      .await
      .unwrap();
    serde_json::from_value(evaluate_response).unwrap()
  }

  pub async fn evaluate_line_with_object_wrapping(
    &mut self,
    line: &str,
  ) -> Result<repl::TsEvaluateResponse, AnyError> {
    self
      .repl_session
      .evaluate_line_with_object_wrapping(line)
      .await
  }

  pub async fn call_function_on_args(
    &mut self,
    function_declaration: String,
    args: &[cdp::RemoteObject],
  ) -> Result<cdp::CallFunctionOnResponse, AnyError> {
    self
      .repl_session
      .call_function_on_args(function_declaration, args)
      .await
  }

  // TODO(bartlomieju): rename to "broadcast_result"?
  pub async fn call_function_on(
    &mut self,
    arg0: cdp::CallArgument,
    arg1: cdp::CallArgument,
  ) -> Option<cdp::CallFunctionOnResponse> {
    let response = self.repl_session
    .post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(json!({
        "functionDeclaration": r#"async function (execution_count, result) {
          await Deno[Deno.internal].jupyter.broadcastResult(execution_count, result);
    }"#,
        "arguments": [arg0, arg1],
        "executionContextId": self.repl_session.context_id,
        "awaitPromise": true,
      })),
    )
    .await.ok()?;
    serde_json::from_value(response).ok()
  }
}
