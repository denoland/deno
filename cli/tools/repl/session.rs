// Copyright 2018-2026 the Deno authors. MIT license.

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
  pub notifications: Arc<Mutex<UnboundedReceiver<Value>>>,
  referrer: ModuleSpecifier,
  main_module: ModuleSpecifier,
  test_reporter_factory: Box<dyn Fn() -> Box<dyn TestReporter>>,
  /// This is only optional because it's temporarily taken when evaluating.
  test_event_receiver: Option<TestEventReceiver>,
  jsx: deno_ast::JsxRuntime,
  decorators: deno_ast::DecoratorsTranspileOption,
  /// When set, the next `evaluate_ts_expression` call produces a source map
  /// for the transpiled code and stores it in `source_maps` under the key
  /// V8 uses for evaluated scripts (`<anonymous>`). Jupyter cells set this
  /// before each evaluate so the stack trace line/column numbers can be
  /// mapped back to the user's TypeScript via `apply_source_map_to_stack`.
  pub track_source_map_for_next: bool,
  source_maps: HashMap<String, Arc<deno_core::sourcemap::SourceMap>>,
  /// Cached cell source per `source_maps` key. Populated alongside
  /// `source_maps` so `apply_source_map_to_stack` can echo the user's
  /// original source line for each remapped frame — Python/IPython-style,
  /// since Jupyter cells don't show line numbers by default.
  cell_sources: HashMap<String, Arc<str>>,
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
    let rx = {
      let mut state = self.0.lock();
      if let Some(message_state) = state.messages.remove(&msg_id) {
        let InspectorMessageState::Ready(value) = message_state else {
          unreachable!();
        };
        return Self::extract_result(value);
      }
      let (tx, rx) = oneshot::channel();
      state
        .messages
        .insert(msg_id, InspectorMessageState::WaitingFor(tx));
      rx
    };

    let value = rx.await.unwrap();
    Self::extract_result(value)
  }

  #[allow(clippy::print_stderr, reason = "diagnostic for flaky CDP responses")]
  fn extract_result(mut value: serde_json::Value) -> serde_json::Value {
    let result = value["result"].take();
    if result.is_null() {
      eprintln!(
        "CDP response has null result. Full response: {}",
        serde_json::to_string(&value).unwrap_or_default()
      );
    }
    result
  }
}

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);
fn next_msg_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl ReplSession {
  #[allow(clippy::too_many_arguments, reason = "construction")]
  pub async fn initialize(
    cli_options: &CliOptions,
    npm_installer: Option<Arc<CliNpmInstaller>>,
    resolver: Arc<CliResolver>,
    compiler_options_resolver: &CompilerOptionsResolver,
    mut worker: MainWorker,
    main_module: ModuleSpecifier,
    test_event_receiver: TestEventReceiver,
  ) -> Result<Self, AnyError> {
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
      track_source_map_for_next: false,
      source_maps: HashMap::new(),
      cell_sources: HashMap::new(),
    };

    // inject prelude
    let evaluated = repl_session.evaluate_expression(&get_prelude()).await?;
    repl_session.internal_object_id = evaluated.result.object_id;

    Ok(repl_session)
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

    // Under Explicit microtask policy, V8's REPL-mode evaluation creates
    // a promise whose resolution callback is a queued microtask. Without
    // this checkpoint, GC can collect the weakly-held promise before the
    // microtask drains, causing a "Promise was collected" CDP error.
    self
      .worker
      .js_runtime
      .v8_isolate()
      .perform_microtask_checkpoint();

    let fut = self
      .state
      .wait_for_response(msg_id)
      .map(Ok::<_, ()>)
      .boxed_local();

    self
      .worker
      .js_runtime
      .with_event_loop_future(fut, Default::default())
      .await
      .unwrap()
  }

  pub async fn run_event_loop(&mut self) -> Result<(), CoreError> {
    // Pump inspector sessions ahead of the rest of the event-loop tick and
    // drain microtasks before any other V8 work runs. Under V8's Explicit
    // microtask policy, REPL-mode Runtime.evaluate (replMode: true) wraps
    // the expression in an async IIFE and tracks the result promise via a
    // weak handle. If GC runs after the inspector dispatch but before the
    // resolution microtask drains, the weakly-held promise is collected and
    // the CDP client gets a `-32000 "Promise was collected"` error. Doing
    // the dispatch + drain pair here closes that window for external
    // debuggers attached via `deno repl --inspect`.
    // (post_message_with_event_loop above handles the same race for the
    // in-process REPL session.)
    std::future::poll_fn(|cx| {
      self
        .worker
        .js_runtime
        .inspector()
        .poll_sessions_from_event_loop(cx);
      self
        .worker
        .js_runtime
        .v8_isolate()
        .perform_microtask_checkpoint();
      let poll_result = self.worker.js_runtime.poll_event_loop(
        cx,
        deno_core::PollEventLoopOptions {
          wait_for_inspector: true,
        },
      );
      // Flush inspector sessions again after the event-loop tick. A timer (or
      // other async) callback that ran during `poll_event_loop` may have
      // produced inspector notifications (e.g. `Runtime.exceptionThrown` from
      // an uncaught error). These are queued on the session's outbound channel
      // and are only delivered to the REPL's notification channel when the
      // sessions are pumped. Without this second pump they would linger until
      // the next evaluation, so an uncaught exception thrown from a timeout
      // would not be printed until the user evaluated another expression (see
      // https://github.com/denoland/deno/issues/21622). Delivering them here
      // wakes the `notifications` stream in the REPL read loop right away.
      self
        .worker
        .js_runtime
        .inspector()
        .poll_sessions_from_event_loop(cx);
      poll_result
    })
    .await
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
    let tsx_result =
      parse_source_as(expression.to_string(), deno_ast::MediaType::Tsx);
    // Prefer a clean `.tsx` parse. Fall back to parsing as TypeScript when the
    // `.tsx` parse fails outright, or when it only recovered by inserting an
    // `Invalid` placeholder node (e.g. a TypeScript type assertion like
    // `<string>x` is misread as JSX in `.tsx`).
    let needs_ts_fallback = match &tsx_result {
      Ok(parsed) => {
        deno_resolver::emit::invalid_syntax_parse_diagnostics(parsed).is_some()
      }
      Err(_) => true,
    };
    let parsed_source = match tsx_result {
      Ok(parsed) if !needs_ts_fallback => parsed,
      tsx_result => match parse_source_as(
        expression.to_string(),
        deno_ast::MediaType::TypeScript,
      ) {
        Ok(parsed) => parsed,
        Err(ts_err) => match tsx_result {
          // Both parses have errors; report the recovered `.tsx` diagnostics
          // via the check below.
          Ok(parsed) => parsed,
          Err(_) => return Err(ts_err),
        },
      },
    };

    // If `swc` recovered from a syntax error by inserting an `Invalid`
    // placeholder node, surface the precise parse diagnostic instead of
    // transpiling to `<invalid>` and letting V8 report a misleading
    // `Unexpected token '<'`. See denoland/deno#19457.
    if let Some(diagnostics) =
      deno_resolver::emit::invalid_syntax_parse_diagnostics(&parsed_source)
    {
      return Err(diagnostics.into());
    }

    self
      .check_for_npm_or_node_imports(&parsed_source.program())
      .await?;

    self.analyze_and_handle_jsx(&parsed_source);

    let want_source_map = self.track_source_map_for_next;
    let original_source: Option<Arc<str>> = if want_source_map {
      Some(Arc::from(parsed_source.text().as_ref()))
    } else {
      None
    };
    let emitted = parsed_source
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
          source_map: if want_source_map {
            deno_ast::SourceMapOption::Separate
          } else {
            deno_ast::SourceMapOption::None
          },
          source_map_base: None,
          source_map_file: None,
          inline_sources: false,
          remove_comments: false,
        },
      )?
      .into_source();
    let transpiled_src = emitted.text;

    if want_source_map
      && let Some(source_map) = emitted.source_map.as_ref()
      && let Ok(parsed_map) =
        deno_core::sourcemap::SourceMap::from_slice(source_map.as_bytes())
    {
      // Keyed by V8's reported file name (always `<anonymous>` for
      // `Runtime.evaluate`). Each cell's source map replaces the previous
      // one, so frames coming from earlier cells will get remapped against
      // the current cell's map. That's a known limitation — V8 doesn't
      // surface per-script identifiers in `err.stack` for evaluate-style
      // inputs, so we can't tell apart frames from prior cells.
      self
        .source_maps
        .insert("<anonymous>".to_string(), Arc::new(parsed_map));
      if let Some(original) = original_source {
        self
          .cell_sources
          .insert("<anonymous>".to_string(), original);
      }
    }

    // V8's `Runtime.evaluate` (with replMode) reports stack frames as
    // `<anonymous>:LINE:COL` and does not honour `//# sourceURL=` magic
    // comments injected into the evaluated script (the inspector treats
    // the script as anonymous regardless). So instead of trying to
    // surface a stable URL through V8, we always register the current
    // cell's source map under the `<anonymous>` key and apply it to any
    // `<anonymous>` frames in the stack. This matches V8's reported
    // script name and handles the common case where the thrown exception
    // originates in the cell that's currently executing. Frames from
    // earlier cells (function definitions that live across cells) still
    // get remapped through the latest source map — that's not strictly
    // correct, but it's far better than reporting the transpiled JS
    // line numbers, and lining them up correctly would require V8 to
    // surface per-script identifiers in `err.stack`.
    let script = format!("'use strict'; void 0;{transpiled_src}");

    let value = self.evaluate_expression(&script).await?;

    Ok(TsEvaluateResponse { value })
  }

  /// Walks a JavaScript stack trace string and rewrites the position of any
  /// frame whose URL we have a registered source map for. Lines that don't
  /// match the `at FILE:LINE:COL` / `at NAME (FILE:LINE:COL)` shapes are
  /// passed through unchanged. When a cached cell source is available for
  /// the frame's file, the user's original source line is echoed beneath
  /// the frame (Python/IPython-style) — Jupyter cells don't show line
  /// numbers by default, so just printing a remapped line/column number
  /// would leave users counting lines in their cell.
  pub fn apply_source_map_to_stack(&mut self, stack: &str) -> String {
    if self.source_maps.is_empty() {
      return stack.to_string();
    }

    static STACK_FRAME_RE: Lazy<Regex> = Lazy::new(|| {
      // Two capture shapes:
      //   `    at FILE:LINE:COL`
      //   `    at NAME (FILE:LINE:COL)`
      // The file portion can itself contain colons (eg. `<jupyter:cell:1>`),
      // so we lazy-match it and let the anchored `:LINE:COL[)]$` tail force
      // backtracking onto the correct boundary.
      Regex::new(r"^(?P<prefix>\s*at (?:.+? \()?)(?P<file>.+?):(?P<line>\d+):(?P<col>\d+)(?P<suffix>\)?)\s*$").unwrap()
    });

    // Cap echoed source lines so a pathological 50KB minified line can't
    // explode the traceback we hand back to Jupyter.
    const MAX_SNIPPET_LEN: usize = 200;

    let mut out = String::with_capacity(stack.len());
    let mut last_snippet_line: Option<(String, u32)> = None;
    for (idx, line) in stack.lines().enumerate() {
      if idx > 0 {
        out.push('\n');
      }
      let Some(caps) = STACK_FRAME_RE.captures(line) else {
        out.push_str(line);
        continue;
      };
      let file = caps.name("file").unwrap().as_str();
      let Some(source_map) = self.source_maps.get(file).cloned() else {
        out.push_str(line);
        continue;
      };
      let line_num: u32 = match caps["line"].parse() {
        Ok(n) => n,
        Err(_) => {
          out.push_str(line);
          continue;
        }
      };
      let col_num: u32 = match caps["col"].parse() {
        Ok(n) => n,
        Err(_) => {
          out.push_str(line);
          continue;
        }
      };
      // SourceMap::lookup_token expects 0-based positions; the lookup token
      // returns 0-based source positions which we convert back to 1-based.
      let lookup_line = line_num.saturating_sub(1);
      let lookup_col = col_num.saturating_sub(1);
      let Some(token) = source_map.lookup_token(lookup_line, lookup_col) else {
        out.push_str(line);
        continue;
      };
      let new_line = token.get_src_line() + 1;
      let new_col = token.get_src_col() + 1;
      // Don't risk silently swapping the URL out: keeping the V8-reported
      // file name avoids visually surprising callers who only have a single
      // cell anyway. Only rewrite the line and column.
      let prefix = caps.name("prefix").unwrap().as_str();
      out.push_str(prefix);
      out.push_str(file);
      out.push(':');
      out.push_str(&new_line.to_string());
      out.push(':');
      out.push_str(&new_col.to_string());
      out.push_str(caps.name("suffix").unwrap().as_str());

      // Skip the snippet if we already echoed the same line for the
      // previous frame (recursive calls land on the same source line and
      // would just produce N copies of the same text).
      if last_snippet_line.as_ref() == Some(&(file.to_string(), new_line)) {
        continue;
      }

      if let Some(source) = self.cell_sources.get(file).cloned()
        && let Some(snippet) =
          source.lines().nth((new_line as usize).saturating_sub(1))
      {
        let trimmed = snippet.trim_end();
        if !trimmed.is_empty() {
          // Indent the snippet beneath the `at ` prefix so it visually
          // nests under the frame.
          let indent = prefix
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect::<String>();
          out.push('\n');
          out.push_str(&indent);
          out.push_str("    ");
          if trimmed.len() > MAX_SNIPPET_LEN {
            // Step backwards to the nearest UTF-8 char boundary so we
            // don't split a multi-byte codepoint.
            let mut cut = MAX_SNIPPET_LEN;
            while cut > 0 && !trimmed.is_char_boundary(cut) {
              cut -= 1;
            }
            out.push_str(&trimmed[..cut]);
            out.push_str("...");
          } else {
            out.push_str(trimmed);
          }
        }
        last_snippet_line = Some((file.to_string(), new_line));
      }
    }
    out
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
          await_promise: Some(true),
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
