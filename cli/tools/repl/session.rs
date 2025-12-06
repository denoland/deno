// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use deno_ast::ImportsNotUsedAsValues;
use deno_ast::JsxAutomaticOptions;
use deno_ast::JsxClassicOptions;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnosticsError;
use deno_ast::ParsedSource;
use deno_ast::SourcePos;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_ast::diagnostics::Diagnostic;
use deno_ast::swc::ast as swc_ast;
use deno_ast::swc::atoms::Wtf8Atom;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::ecma_visit::noop_visit_type;
use deno_core::LocalInspectorSession;
use deno_core::PollEventLoopOptions;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::channel::mpsc::unbounded;
use deno_core::parking_lot::Mutex as SyncMutex;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::Position;
use deno_graph::PositionRange;
use deno_graph::analysis::SpecifierWithRange;
use deno_lib::util::result::any_and_jserrorbox_downcast_ref;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_runtime::worker::MainWorker;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use once_cell::sync::Lazy;
use regex::Match;
use regex::Regex;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

use crate::args::CliOptions;
use crate::cdp;
use crate::cdp::RemoteObjectId;
use crate::colors;
use crate::lsp::ReplLanguageServer;
use crate::npm::CliNpmInstaller;
use crate::resolver::CliResolver;
use crate::tools::test::TestEventReceiver;
use crate::tools::test::TestEventTracker;
use crate::tools::test::TestFailureFormatOptions;
use crate::tools::test::report_tests;
use crate::tools::test::reporters::PrettyTestReporter;
use crate::tools::test::reporters::TestReporter;
use crate::tools::test::run_tests_for_worker;
use crate::tools::test::worker_has_tests;

fn comment_source_to_position_range(
  comment_start: SourcePos,
  m: &Match,
  text_info: &SourceTextInfo,
  is_jsx_import_source: bool,
) -> PositionRange {
  // the comment text starts after the double slash or slash star, so add 2
  let comment_start = comment_start + 2;
  // -1 and +1 to include the quotes, but not for jsx import sources because
  // they don't have quotes
  let padding = if is_jsx_import_source { 0 } else { 1 };
  PositionRange {
    start: Position::from_source_pos(
      comment_start + m.start() - padding,
      text_info,
    ),
    end: Position::from_source_pos(
      comment_start + m.end() + padding,
      text_info,
    ),
  }
}

fn get_prelude() -> String {
  r#"(() => {
  const repl_internal = {
    String,
    lastEvalResult: undefined,
    lastThrownError: undefined,
    inspectArgs: Deno[Deno.internal].inspectArgs,
    noColor: Deno.noColor,
    get closed() {
      try {
        return typeof globalThis.closed === 'undefined' ? false : globalThis.closed;
      } catch {
        return false;
      }
    }
  };

  Object.defineProperty(globalThis, "_", {
    configurable: true,
    get: () => repl_internal.lastEvalResult,
    set: (value) => {
     Object.defineProperty(globalThis, "_", {
       value: value,
       writable: true,
       enumerable: true,
       configurable: true,
     });
     console.log("Last evaluation result is no longer saved to _.");
    },
  });

  Object.defineProperty(globalThis, "_error", {
    configurable: true,
    get: () => repl_internal.lastThrownError,
    set: (value) => {
     Object.defineProperty(globalThis, "_error", {
       value: value,
       writable: true,
       enumerable: true,
       configurable: true,
     });

     console.log("Last thrown error is no longer saved to _error.");
    },
  });

  globalThis.clear = console.clear.bind(console);

  return repl_internal;
})()"#.to_string()
}

pub enum EvaluationOutput {
  Value(String),
  Error(String),
}

impl std::fmt::Display for EvaluationOutput {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      EvaluationOutput::Value(value) => f.write_str(value),
      EvaluationOutput::Error(value) => f.write_str(value),
    }
  }
}

pub fn result_to_evaluation_output(
  r: Result<EvaluationOutput, AnyError>,
) -> EvaluationOutput {
  match r {
    Ok(value) => value,
    Err(err) => {
      EvaluationOutput::Error(format!("{} {:#}", colors::red("error:"), err))
    }
  }
}

#[derive(Debug)]
pub struct TsEvaluateResponse {
  pub ts_code: String,
  pub value: cdp::EvaluateResponse,
}

pub struct ReplSession {
  internal_object_id: Option<RemoteObjectId>,
  npm_installer: Option<Arc<CliNpmInstaller>>,
  resolver: Arc<CliResolver>,
  // NB: `session` and `state` must come before Worker, so that relevant V8 objects
  // are dropped before the isolate is dropped with `worker`.
  session: LocalInspectorSession,
  state: ReplSessionState,
  pub worker: MainWorker,
  pub context_id: u64,
  pub language_server: ReplLanguageServer,
  pub notifications: Arc<Mutex<UnboundedReceiver<Value>>>,
  referrer: ModuleSpecifier,
  main_module: ModuleSpecifier,
  test_reporter_factory: Box<dyn Fn() -> Box<dyn TestReporter>>,
  /// This is only optional because it's temporarily taken when evaluating.
  test_event_receiver: Option<TestEventReceiver>,
  jsx: deno_ast::JsxRuntime,
  decorators: deno_ast::DecoratorsTranspileOption,
}

// TODO: duplicated in `cli/tools/run/hmr.rs`
#[derive(Debug)]
enum InspectorMessageState {
  Ready(serde_json::Value),
  WaitingFor(oneshot::Sender<serde_json::Value>),
}

#[derive(Debug)]
pub struct ReplSessionInner {
  messages: HashMap<i32, InspectorMessageState>,
  notification_tx: UnboundedSender<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct ReplSessionState(Arc<SyncMutex<ReplSessionInner>>);

impl ReplSessionState {
  pub fn new(notification_tx: UnboundedSender<serde_json::Value>) -> Self {
    Self(Arc::new(SyncMutex::new(ReplSessionInner {
      messages: HashMap::new(),
      notification_tx,
    })))
  }

  fn callback(&self, msg: deno_core::InspectorMsg) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      if let Ok(value) = serde_json::from_str(&msg.content) {
        let _ = self.0.lock().notification_tx.unbounded_send(value);
      }
      return;
    };

    let message: serde_json::Value = match serde_json::from_str(&msg.content) {
      Ok(v) => v,
      Err(error) => match error.classify() {
        serde_json::error::Category::Syntax => serde_json::json!({
          "id": msg_id,
          "result": {
            "result": {
              "type": "error",
              "description": "Unterminated string literal",
              "value": "Unterminated string literal",
            },
            "exceptionDetails": {
              "exceptionId": 0,
              "text": "Unterminated string literal",
              "lineNumber": 0,
              "columnNumber": 0
            },
          },
        }),
        _ => panic!("Could not parse inspector message"),
      },
    };

    let mut state = self.0.lock();
    let Some(message_state) = state.messages.remove(&msg_id) else {
      state
        .messages
        .insert(msg_id, InspectorMessageState::Ready(message));
      return;
    };
    let InspectorMessageState::WaitingFor(sender) = message_state else {
      return;
    };
    let _ = sender.send(message);
  }

  async fn wait_for_response(&self, msg_id: i32) -> serde_json::Value {
    if let Some(message_state) = self.0.lock().messages.remove(&msg_id) {
      let InspectorMessageState::Ready(mut value) = message_state else {
        unreachable!();
      };
      return value["result"].take();
    }

    let (tx, rx) = oneshot::channel();
    self
      .0
      .lock()
      .messages
      .insert(msg_id, InspectorMessageState::WaitingFor(tx));
    let mut value = rx.await.unwrap();
    value["result"].take()
  }
}

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);
fn next_msg_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl ReplSession {
  #[allow(clippy::too_many_arguments)]
  pub async fn initialize(
    cli_options: &CliOptions,
    npm_installer: Option<Arc<CliNpmInstaller>>,
    resolver: Arc<CliResolver>,
    compiler_options_resolver: &CompilerOptionsResolver,
    mut worker: MainWorker,
    main_module: ModuleSpecifier,
    test_event_receiver: TestEventReceiver,
  ) -> Result<Self, AnyError> {
    let language_server = ReplLanguageServer::new_initialized().await?;

    let (notification_tx, mut notification_rx) = unbounded();
    let repl_session_state = ReplSessionState::new(notification_tx);
    let state = repl_session_state.clone();
    let callback =
      Box::new(move |message| repl_session_state.callback(message));
    let mut session = worker.create_inspector_session(callback);

    session.post_message::<()>(next_msg_id(), "Runtime.enable", None);

    // Enabling the runtime domain will always send trigger one executionContextCreated for each
    // context the inspector knows about so we grab the execution context from that since
    // our inspector does not support a default context (0 is an invalid context id).
    let context_id: u64;

    loop {
      let notification = notification_rx.next().await.unwrap();
      let notification =
        serde_json::from_value::<cdp::Notification>(notification)?;
      if notification.method == "Runtime.executionContextCreated" {
        let execution_context_created = serde_json::from_value::<
          cdp::ExecutionContextCreated,
        >(notification.params)?;
        assert!(
          execution_context_created
            .context
            .aux_data
            .get("isDefault")
            .unwrap()
            .as_bool()
            .unwrap()
        );
        context_id = execution_context_created.context.id;
        break;
      }
    }
    assert_ne!(context_id, 0);

    let referrer =
      deno_core::resolve_path("./$deno$repl.mts", cli_options.initial_cwd())
        .unwrap();

    let cwd_url =
      Url::from_directory_path(cli_options.initial_cwd()).map_err(|_| {
        anyhow!(
          "Unable to construct URL from the path of cwd: {}",
          cli_options.initial_cwd().to_string_lossy(),
        )
      })?;
    let transpile_options = &compiler_options_resolver
      .for_specifier(&cwd_url)
      .transpile_options()?
      .transpile;
    let mut repl_session = ReplSession {
      internal_object_id: None,
      npm_installer,
      resolver,
      worker,
      session,
      state,
      context_id,
      language_server,
      referrer,
      notifications: Arc::new(Mutex::new(notification_rx)),
      test_reporter_factory: Box::new(move || {
        Box::new(PrettyTestReporter::new(
          false,
          true,
          false,
          true,
          cwd_url.clone(),
          TestFailureFormatOptions::default(),
        ))
      }),
      main_module,
      test_event_receiver: Some(test_event_receiver),
      jsx: transpile_options.jsx.clone().unwrap_or_default(),
      decorators: transpile_options.decorators.clone(),
    };

    // inject prelude
    let evaluated = repl_session.evaluate_expression(&get_prelude()).await?;
    repl_session.internal_object_id = evaluated.result.object_id;

    Ok(repl_session)
  }

  pub fn set_test_reporter_factory(
    &mut self,
    f: Box<dyn Fn() -> Box<dyn TestReporter>>,
  ) {
    self.test_reporter_factory = f;
  }

  pub async fn closing(&mut self) -> Result<bool, AnyError> {
    let result = self
      .call_function_on_repl_internal_obj(
        r#"function () { return this.closed; }"#.to_string(),
        &[],
      )
      .await?
      .result;
    let closed = result
      .value
      .ok_or_else(|| anyhow!(result.description.unwrap()))?
      .as_bool()
      .unwrap();

    Ok(closed)
  }

  pub async fn post_message_with_event_loop<T: serde::Serialize>(
    &mut self,
    method: &str,
    params: Option<T>,
  ) -> Value {
    let msg_id = next_msg_id();
    self.session.post_message(msg_id, method, params);
    let fut = self
      .state
      .wait_for_response(msg_id)
      .map(Ok::<_, ()>)
      .boxed_local();

    self
      .worker
      .js_runtime
      .with_event_loop_future(
        fut,
        PollEventLoopOptions {
          // NOTE(bartlomieju): this is an important bit; we don't want to pump V8
          // message loop here, so that GC won't run. Otherwise, the resulting
          // object might be GC'ed before we have a chance to inspect it.
          pump_v8_message_loop: false,
          ..Default::default()
        },
      )
      .await
      .unwrap()
  }

  pub async fn run_event_loop(&mut self) -> Result<(), CoreError> {
    self.worker.run_event_loop(true).await
  }

  pub async fn evaluate_line_and_get_output(
    &mut self,
    line: &str,
  ) -> EvaluationOutput {
    fn format_diagnostic(diagnostic: &deno_ast::ParseDiagnostic) -> String {
      let display_position = diagnostic.display_position();
      format!(
        "{}: {} at {}:{}",
        colors::red("parse error"),
        diagnostic.message(),
        display_position.line_number,
        display_position.column_number,
      )
    }

    async fn inner(
      session: &mut ReplSession,
      line: &str,
    ) -> Result<EvaluationOutput, AnyError> {
      match session.evaluate_line_with_object_wrapping(line).await {
        Ok(evaluate_response) => {
          let cdp::EvaluateResponse {
            result,
            exception_details,
          } = evaluate_response.value;

          Ok(if let Some(exception_details) = exception_details {
            session.set_last_thrown_error(&result).await?;
            let description = match exception_details.exception {
              Some(exception) => {
                if let Some(description) = exception.description {
                  description
                } else if let Some(value) = exception.value {
                  value.to_string()
                } else {
                  "undefined".to_string()
                }
              }
              None => "Unknown exception".to_string(),
            };
            EvaluationOutput::Error(format!(
              "{} {}",
              exception_details.text, description
            ))
          } else {
            session
              .language_server
              .commit_text(&evaluate_response.ts_code)
              .await;

            session.set_last_eval_result(&result).await?;
            let value = session.get_eval_value(&result).await?;
            EvaluationOutput::Value(value)
          })
        }
        Err(err) => {
          // handle a parsing diagnostic
          match any_and_jserrorbox_downcast_ref::<deno_ast::ParseDiagnostic>(
            &err,
          ) {
            Some(diagnostic) => {
              Ok(EvaluationOutput::Error(format_diagnostic(diagnostic)))
            }
            None => {
              match any_and_jserrorbox_downcast_ref::<ParseDiagnosticsError>(
                &err,
              ) {
                Some(diagnostics) => Ok(EvaluationOutput::Error(
                  diagnostics
                    .0
                    .iter()
                    .map(format_diagnostic)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                )),
                None => Err(err),
              }
            }
          }
        }
      }
    }

    let result = inner(self, line).await;
    result_to_evaluation_output(result)
  }

  pub async fn evaluate_line_with_object_wrapping(
    &mut self,
    line: &str,
  ) -> Result<TsEvaluateResponse, AnyError> {
    // Expressions like { "foo": "bar" } are interpreted as block expressions at the
    // statement level rather than an object literal so we interpret it as an expression statement
    // to match the behavior found in a typical prompt including browser developer tools.
    let wrapped_line = if line.trim_start().starts_with('{')
      && !line.trim_end().ends_with(';')
    {
      format!("({})", &line)
    } else {
      line.to_string()
    };

    let evaluate_response = self.evaluate_ts_expression(&wrapped_line).await;

    // If that fails, we retry it without wrapping in parens letting the error bubble up to the
    // user if it is still an error.
    let result = if wrapped_line != line
      && (evaluate_response.is_err()
        || evaluate_response
          .as_ref()
          .unwrap()
          .value
          .exception_details
          .is_some())
    {
      self.evaluate_ts_expression(line).await
    } else {
      evaluate_response
    };

    if worker_has_tests(&mut self.worker) {
      let report_tests_handle = spawn(report_tests(
        self.test_event_receiver.take().unwrap(),
        (self.test_reporter_factory)(),
      ));
      let event_tracker =
        TestEventTracker::new(self.worker.js_runtime.op_state());
      run_tests_for_worker(
        &mut self.worker,
        &self.main_module,
        &Default::default(),
        &Default::default(),
        &event_tracker,
      )
      .await
      .unwrap();
      event_tracker.force_end_report().unwrap();
      self.test_event_receiver = Some(report_tests_handle.await.unwrap().1);
    }

    result
  }

  pub async fn set_last_thrown_error(
    &mut self,
    error: &cdp::RemoteObject,
  ) -> Result<(), AnyError> {
    self
      .post_message_with_event_loop(
        "Runtime.callFunctionOn",
        Some(cdp::CallFunctionOnArgs {
          function_declaration:
            r#"function (object) { this.lastThrownError = object; }"#
              .to_string(),
          object_id: self.internal_object_id.clone(),
          arguments: Some(vec![error.into()]),
          silent: None,
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          execution_context_id: None,
          object_group: None,
          throw_on_side_effect: None,
        }),
      )
      .await;
    Ok(())
  }

  pub async fn set_last_eval_result(
    &mut self,
    evaluate_result: &cdp::RemoteObject,
  ) -> Result<(), AnyError> {
    self
      .post_message_with_event_loop(
        "Runtime.callFunctionOn",
        Some(cdp::CallFunctionOnArgs {
          function_declaration: r#"function (object) { this.lastEvalResult = object; }"#.to_string(),
          object_id: self.internal_object_id.clone(),
          arguments: Some(vec![evaluate_result.into()]),
          silent: None,
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          execution_context_id: None,
          object_group: None,
          throw_on_side_effect: None,
        }),
      )
      .await;
    Ok(())
  }

  pub async fn call_function_on_args(
    &mut self,
    function_declaration: String,
    args: &[cdp::RemoteObject],
  ) -> Result<cdp::CallFunctionOnResponse, AnyError> {
    let arguments: Option<Vec<cdp::CallArgument>> = if args.is_empty() {
      None
    } else {
      Some(args.iter().map(|a| a.into()).collect())
    };

    let inspect_response = self
      .post_message_with_event_loop(
        "Runtime.callFunctionOn",
        Some(cdp::CallFunctionOnArgs {
          function_declaration,
          object_id: None,
          arguments,
          silent: None,
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          execution_context_id: Some(self.context_id),
          object_group: None,
          throw_on_side_effect: None,
        }),
      )
      .await;

    let response: cdp::CallFunctionOnResponse =
      serde_json::from_value(inspect_response)?;
    Ok(response)
  }

  pub async fn call_function_on_repl_internal_obj(
    &mut self,
    function_declaration: String,
    args: &[cdp::RemoteObject],
  ) -> Result<cdp::CallFunctionOnResponse, AnyError> {
    let arguments: Option<Vec<cdp::CallArgument>> = if args.is_empty() {
      None
    } else {
      Some(args.iter().map(|a| a.into()).collect())
    };

    let inspect_response = self
      .post_message_with_event_loop(
        "Runtime.callFunctionOn",
        Some(cdp::CallFunctionOnArgs {
          function_declaration,
          object_id: self.internal_object_id.clone(),
          arguments,
          silent: None,
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          execution_context_id: None,
          object_group: None,
          throw_on_side_effect: None,
        }),
      )
      .await;

    let response: cdp::CallFunctionOnResponse =
      serde_json::from_value(inspect_response)?;
    Ok(response)
  }

  pub async fn get_eval_value(
    &mut self,
    evaluate_result: &cdp::RemoteObject,
  ) -> Result<String, AnyError> {
    // TODO(caspervonb) we should investigate using previews here but to keep things
    // consistent with the previous implementation we just get the preview result from
    // Deno.inspectArgs.
    let response = self
      .call_function_on_repl_internal_obj(
        r#"function (object) {
          try {
            return this.inspectArgs(["%o", object], { colors: !this.noColor });
          } catch (err) {
            return this.inspectArgs(["%o", err]);
          }
        }"#
          .to_string(),
        std::slice::from_ref(evaluate_result),
      )
      .await?;
    let s = response
      .result
      .value
      .map(|v| v.as_str().unwrap().to_string())
      .or(response.result.description)
      .ok_or_else(|| anyhow!("failed to evaluate expression"))?;

    Ok(s)
  }

  async fn evaluate_ts_expression(
    &mut self,
    expression: &str,
  ) -> Result<TsEvaluateResponse, AnyError> {
    let parsed_source =
      match parse_source_as(expression.to_string(), deno_ast::MediaType::Tsx) {
        Ok(parsed) => parsed,
        Err(err) => {
          match parse_source_as(
            expression.to_string(),
            deno_ast::MediaType::TypeScript,
          ) {
            Ok(parsed) => parsed,
            _ => {
              return Err(err);
            }
          }
        }
      };

    self
      .check_for_npm_or_node_imports(&parsed_source.program())
      .await?;

    self.analyze_and_handle_jsx(&parsed_source);

    let transpiled_src = parsed_source
      .transpile(
        &deno_ast::TranspileOptions {
          decorators: self.decorators.clone(),
          imports_not_used_as_values: ImportsNotUsedAsValues::Preserve,
          jsx: Some(self.jsx.clone()),
          var_decl_imports: true,
          verbatim_module_syntax: false,
        },
        &deno_ast::TranspileModuleOptions {
          module_kind: Some(ModuleKind::Esm),
        },
        &deno_ast::EmitOptions {
          source_map: deno_ast::SourceMapOption::None,
          source_map_base: None,
          source_map_file: None,
          inline_sources: false,
          remove_comments: false,
        },
      )?
      .into_source()
      .text;

    let value = self
      .evaluate_expression(&format!("'use strict'; void 0;{transpiled_src}"))
      .await?;

    Ok(TsEvaluateResponse {
      ts_code: expression.to_string(),
      value,
    })
  }

  fn analyze_and_handle_jsx(&mut self, parsed_source: &ParsedSource) {
    let Some(analyzed_pragmas) = analyze_jsx_pragmas(parsed_source) else {
      return;
    };

    if !analyzed_pragmas.has_any() {
      return;
    }

    if let Some(jsx) = analyzed_pragmas.jsx {
      match &mut self.jsx {
        deno_ast::JsxRuntime::Classic(jsx_classic_options) => {
          jsx_classic_options.factory = jsx.text;
        }
        deno_ast::JsxRuntime::Automatic(_)
        | deno_ast::JsxRuntime::Precompile(_) => {
          self.jsx = deno_ast::JsxRuntime::Classic(JsxClassicOptions {
            factory: jsx.text,
            ..Default::default()
          });
        }
      }
    }
    if let Some(jsx_frag) = analyzed_pragmas.jsx_fragment {
      match &mut self.jsx {
        deno_ast::JsxRuntime::Classic(jsx_classic_options) => {
          jsx_classic_options.fragment_factory = jsx_frag.text;
        }
        deno_ast::JsxRuntime::Automatic(_)
        | deno_ast::JsxRuntime::Precompile(_) => {
          self.jsx = deno_ast::JsxRuntime::Classic(JsxClassicOptions {
            fragment_factory: jsx_frag.text,
            ..Default::default()
          });
        }
      }
    }
    if let Some(jsx_import_source) = analyzed_pragmas.jsx_import_source {
      match &mut self.jsx {
        deno_ast::JsxRuntime::Classic(_) => {
          self.jsx = deno_ast::JsxRuntime::Automatic(JsxAutomaticOptions {
            import_source: Some(jsx_import_source.text),
            development: false,
          });
        }
        deno_ast::JsxRuntime::Automatic(automatic)
        | deno_ast::JsxRuntime::Precompile(deno_ast::JsxPrecompileOptions {
          automatic,
          ..
        }) => {
          automatic.import_source = Some(jsx_import_source.text);
        }
      }
    }
  }

  async fn check_for_npm_or_node_imports(
    &mut self,
    program: &swc_ast::Program,
  ) -> Result<(), AnyError> {
    let Some(npm_installer) = &self.npm_installer else {
      return Ok(());
    };

    let mut collector = ImportCollector::new();
    program.visit_with(&mut collector);

    let resolved_imports = collector
      .imports
      .iter()
      .flat_map(|i| {
        let specifier = i.to_string_lossy();
        self
          .resolver
          .resolve(
            &specifier,
            &self.referrer,
            deno_graph::Position::zeroed(),
            ResolutionMode::Import,
            NodeResolutionKind::Execution,
          )
          .ok()
          .or_else(|| ModuleSpecifier::parse(&specifier).ok())
      })
      .collect::<Vec<_>>();

    let npm_imports = resolved_imports
      .iter()
      .flat_map(|url| NpmPackageReqReference::from_specifier(url).ok())
      .map(|r| r.into_inner().req)
      .collect::<Vec<_>>();
    if !npm_imports.is_empty() {
      npm_installer
        .add_and_cache_package_reqs(&npm_imports)
        .await?;
    }
    Ok(())
  }

  async fn evaluate_expression(
    &mut self,
    expression: &str,
  ) -> Result<cdp::EvaluateResponse, JsErrorBox> {
    let res = self
      .post_message_with_event_loop(
        "Runtime.evaluate",
        Some(cdp::EvaluateArgs {
          expression: expression.to_string(),
          object_group: None,
          include_command_line_api: None,
          silent: None,
          context_id: Some(self.context_id),
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          throw_on_side_effect: None,
          timeout: None,
          disable_breaks: None,
          repl_mode: Some(true),
          allow_unsafe_eval_blocked_by_csp: None,
          unique_context_id: None,
        }),
      )
      .await;
    serde_json::from_value(res).map_err(JsErrorBox::from_err)
  }
}

/// Walk an AST and get all import specifiers for analysis if any of them is
/// an npm specifier.
struct ImportCollector {
  pub imports: Vec<Wtf8Atom>,
}

impl ImportCollector {
  pub fn new() -> Self {
    Self { imports: vec![] }
  }
}

impl Visit for ImportCollector {
  noop_visit_type!();

  fn visit_call_expr(&mut self, call_expr: &swc_ast::CallExpr) {
    if !matches!(call_expr.callee, swc_ast::Callee::Import(_)) {
      return;
    }

    if !call_expr.args.is_empty() {
      let arg = &call_expr.args[0];
      if let swc_ast::Expr::Lit(swc_ast::Lit::Str(str_lit)) = &*arg.expr {
        self.imports.push(str_lit.value.clone());
      }
    }
  }

  fn visit_module_decl(&mut self, module_decl: &swc_ast::ModuleDecl) {
    use deno_ast::swc::ast::*;

    match module_decl {
      ModuleDecl::Import(import_decl) => {
        if import_decl.type_only {
          return;
        }

        self.imports.push(import_decl.src.value.clone());
      }
      ModuleDecl::ExportAll(export_all) => {
        self.imports.push(export_all.src.value.clone());
      }
      ModuleDecl::ExportNamed(export_named) => {
        if let Some(src) = &export_named.src {
          self.imports.push(src.value.clone());
        }
      }
      _ => {}
    }
  }
}

fn parse_source_as(
  source: String,
  media_type: deno_ast::MediaType,
) -> Result<deno_ast::ParsedSource, AnyError> {
  let specifier = if media_type == deno_ast::MediaType::Tsx {
    ModuleSpecifier::parse("file:///repl.tsx").unwrap()
  } else {
    ModuleSpecifier::parse("file:///repl.ts").unwrap()
  };

  let parsed = deno_ast::parse_module(deno_ast::ParseParams {
    specifier,
    text: source.into(),
    media_type,
    capture_tokens: true,
    maybe_syntax: None,
    scope_analysis: false,
  })?;

  Ok(parsed)
}

// TODO(bartlomieju): remove these and use regexes from `deno_graph`
/// Matches the `@jsxImportSource` pragma.
static JSX_IMPORT_SOURCE_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(?i)^[\s*]*@jsxImportSource\s+(\S+)").unwrap());
/// Matches the `@jsx` pragma.
static JSX_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(?i)^[\s*]*@jsx\s+(\S+)").unwrap());
/// Matches the `@jsxFrag` pragma.
static JSX_FRAG_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(?i)^[\s*]*@jsxFrag\s+(\S+)").unwrap());

#[derive(Default, Debug)]
struct AnalyzedJsxPragmas {
  /// Information about `@jsxImportSource` pragma.
  jsx_import_source: Option<SpecifierWithRange>,

  /// Matches the `@jsx` pragma.
  jsx: Option<SpecifierWithRange>,

  /// Matches the `@jsxFrag` pragma.
  jsx_fragment: Option<SpecifierWithRange>,
}

impl AnalyzedJsxPragmas {
  fn has_any(&self) -> bool {
    self.jsx_import_source.is_some()
      || self.jsx.is_some()
      || self.jsx_fragment.is_some()
  }
}

/// Analyze provided source and return information about carious pragmas
/// used to configure the JSX transforms.
fn analyze_jsx_pragmas(
  parsed_source: &ParsedSource,
) -> Option<AnalyzedJsxPragmas> {
  if !matches!(
    parsed_source.media_type(),
    deno_ast::MediaType::Jsx | deno_ast::MediaType::Tsx
  ) {
    return None;
  }

  let mut analyzed_pragmas = AnalyzedJsxPragmas::default();

  for c in parsed_source.get_leading_comments()?.iter() {
    if c.kind != CommentKind::Block {
      continue; // invalid
    }

    if let Some(captures) = JSX_IMPORT_SOURCE_RE.captures(&c.text)
      && let Some(m) = captures.get(1)
    {
      analyzed_pragmas.jsx_import_source = Some(SpecifierWithRange {
        text: m.as_str().to_string(),
        range: comment_source_to_position_range(
          c.start(),
          &m,
          parsed_source.text_info_lazy(),
          true,
        ),
      });
    }

    if let Some(captures) = JSX_RE.captures(&c.text)
      && let Some(m) = captures.get(1)
    {
      analyzed_pragmas.jsx = Some(SpecifierWithRange {
        text: m.as_str().to_string(),
        range: comment_source_to_position_range(
          c.start(),
          &m,
          parsed_source.text_info_lazy(),
          false,
        ),
      });
    }

    if let Some(captures) = JSX_FRAG_RE.captures(&c.text)
      && let Some(m) = captures.get(1)
    {
      analyzed_pragmas.jsx_fragment = Some(SpecifierWithRange {
        text: m.as_str().to_string(),
        range: comment_source_to_position_range(
          c.start(),
          &m,
          parsed_source.text_info_lazy(),
          false,
        ),
      });
    }
  }

  Some(analyzed_pragmas)
}
