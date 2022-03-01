// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cdp;
use crate::colors;
use crate::lsp::ReplLanguageServer;
use deno_ast::DiagnosticsError;
use deno_ast::ImportsNotUsedAsValues;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::LocalInspectorSession;
use deno_runtime::worker::MainWorker;

static PRELUDE: &str = r#"
Object.defineProperty(globalThis, "_", {
  configurable: true,
  get: () => Deno[Deno.internal].lastEvalResult,
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
  get: () => Deno[Deno.internal].lastThrownError,
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
"#;

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

struct TsEvaluateResponse {
  ts_code: String,
  value: cdp::EvaluateResponse,
}

pub struct ReplSession {
  pub worker: MainWorker,
  session: LocalInspectorSession,
  pub context_id: u64,
  pub language_server: ReplLanguageServer,
}

impl ReplSession {
  pub async fn initialize(mut worker: MainWorker) -> Result<Self, AnyError> {
    let language_server = ReplLanguageServer::new_initialized().await?;
    let mut session = worker.create_inspector_session().await;

    worker
      .with_event_loop(
        session
          .post_message::<()>("Runtime.enable", None)
          .boxed_local(),
      )
      .await?;

    // Enabling the runtime domain will always send trigger one executionContextCreated for each
    // context the inspector knows about so we grab the execution context from that since
    // our inspector does not support a default context (0 is an invalid context id).
    let mut context_id: u64 = 0;
    for notification in session.notifications() {
      let method = notification.get("method").unwrap().as_str().unwrap();
      let params = notification.get("params").unwrap();

      if method == "Runtime.executionContextCreated" {
        context_id = params
          .get("context")
          .unwrap()
          .get("id")
          .unwrap()
          .as_u64()
          .unwrap();
      }
    }

    let mut repl_session = ReplSession {
      worker,
      session,
      context_id,
      language_server,
    };

    // inject prelude
    repl_session.evaluate_expression(PRELUDE).await?;

    Ok(repl_session)
  }

  pub async fn is_closing(&mut self) -> Result<bool, AnyError> {
    let closed = self
      .evaluate_expression("(this.closed)")
      .await?
      .result
      .value
      .unwrap()
      .as_bool()
      .unwrap();

    Ok(closed)
  }

  pub async fn post_message_with_event_loop<T: serde::Serialize>(
    &mut self,
    method: &str,
    params: Option<T>,
  ) -> Result<Value, AnyError> {
    self
      .worker
      .with_event_loop(self.session.post_message(method, params).boxed_local())
      .await
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    self.worker.run_event_loop(true).await
  }

  pub async fn evaluate_line_and_get_output(
    &mut self,
    line: &str,
  ) -> Result<EvaluationOutput, AnyError> {
    fn format_diagnostic(diagnostic: &deno_ast::Diagnostic) -> String {
      format!(
        "{}: {} at {}:{}",
        colors::red("parse error"),
        diagnostic.message(),
        diagnostic.display_position.line_number,
        diagnostic.display_position.column_number,
      )
    }

    match self.evaluate_line_with_object_wrapping(line).await {
      Ok(evaluate_response) => {
        let cdp::EvaluateResponse {
          result,
          exception_details,
        } = evaluate_response.value;

        if exception_details.is_some() {
          self.set_last_thrown_error(&result).await?;
        } else {
          self
            .language_server
            .commit_text(&evaluate_response.ts_code)
            .await;

          self.set_last_eval_result(&result).await?;
        }

        let value = self.get_eval_value(&result).await?;
        Ok(match exception_details {
          Some(_) => EvaluationOutput::Error(format!("Uncaught {}", value)),
          None => EvaluationOutput::Value(value),
        })
      }
      Err(err) => {
        // handle a parsing diagnostic
        match err.downcast_ref::<deno_ast::Diagnostic>() {
          Some(diagnostic) => {
            Ok(EvaluationOutput::Error(format_diagnostic(diagnostic)))
          }
          None => match err.downcast_ref::<DiagnosticsError>() {
            Some(diagnostics) => Ok(EvaluationOutput::Error(
              diagnostics
                .0
                .iter()
                .map(format_diagnostic)
                .collect::<Vec<_>>()
                .join("\n\n"),
            )),
            None => Err(err),
          },
        }
      }
    }
  }

  async fn evaluate_line_with_object_wrapping(
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
    if wrapped_line != line
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
    }
  }

  async fn set_last_thrown_error(
    &mut self,
    error: &cdp::RemoteObject,
  ) -> Result<(), AnyError> {
    self.post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(cdp::CallFunctionOnArgs {
        function_declaration: "function (object) { Deno[Deno.internal].lastThrownError = object; }".to_string(),
        object_id: None,
        arguments: Some(vec![error.into()]),
        silent: None,
        return_by_value: None,
        generate_preview: None,
        user_gesture: None,
        await_promise: None,
        execution_context_id: Some(self.context_id),
        object_group: None,
        throw_on_side_effect: None
      }),
    ).await?;
    Ok(())
  }

  async fn set_last_eval_result(
    &mut self,
    evaluate_result: &cdp::RemoteObject,
  ) -> Result<(), AnyError> {
    self
      .post_message_with_event_loop(
        "Runtime.callFunctionOn",
        Some(cdp::CallFunctionOnArgs {
          function_declaration:
            "function (object) { Deno[Deno.internal].lastEvalResult = object; }"
              .to_string(),
          object_id: None,
          arguments: Some(vec![evaluate_result.into()]),
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
      .await?;
    Ok(())
  }

  pub async fn get_eval_value(
    &mut self,
    evaluate_result: &cdp::RemoteObject,
  ) -> Result<String, AnyError> {
    // TODO(caspervonb) we should investigate using previews here but to keep things
    // consistent with the previous implementation we just get the preview result from
    // Deno.inspectArgs.
    let inspect_response = self.post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(cdp::CallFunctionOnArgs {
        function_declaration: r#"function (object) {
          try {
            return Deno[Deno.internal].inspectArgs(["%o", object], { colors: !Deno.noColor });
          } catch (err) {
            return Deno[Deno.internal].inspectArgs(["%o", err]);
          }
        }"#.to_string(),
        object_id: None,
        arguments: Some(vec![evaluate_result.into()]),
        silent: None,
        return_by_value: None,
        generate_preview: None,
        user_gesture: None,
        await_promise: None,
        execution_context_id: Some(self.context_id),
        object_group: None,
        throw_on_side_effect: None
      }),
    ).await?;

    let response: cdp::CallFunctionOnResponse =
      serde_json::from_value(inspect_response)?;
    let value = response.result.value.unwrap();
    let s = value.as_str().unwrap();

    Ok(s.to_string())
  }

  async fn evaluate_ts_expression(
    &mut self,
    expression: &str,
  ) -> Result<TsEvaluateResponse, AnyError> {
    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: "repl.ts".to_string(),
      source: deno_ast::SourceTextInfo::from_string(expression.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })?;

    let transpiled_src = parsed_module
      .transpile(&deno_ast::EmitOptions {
        emit_metadata: false,
        source_map: false,
        inline_source_map: false,
        inline_sources: false,
        imports_not_used_as_values: ImportsNotUsedAsValues::Preserve,
        // JSX is not supported in the REPL
        transform_jsx: false,
        jsx_automatic: false,
        jsx_development: false,
        jsx_factory: "React.createElement".into(),
        jsx_fragment_factory: "React.Fragment".into(),
        jsx_import_source: None,
        var_decl_imports: true,
      })?
      .text;

    let value = self
      .evaluate_expression(&format!(
        "'use strict'; void 0;\n{}",
        transpiled_src
      ))
      .await?;

    Ok(TsEvaluateResponse {
      ts_code: expression.to_string(),
      value,
    })
  }

  async fn evaluate_expression(
    &mut self,
    expression: &str,
  ) -> Result<cdp::EvaluateResponse, AnyError> {
    self
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
      .await
      .and_then(|res| serde_json::from_value(res).map_err(|e| e.into()))
  }
}
