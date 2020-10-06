// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostics;
use crate::file_fetcher::TextDocument;
use crate::graph::ModuleProvider;
use crate::graph::Stats;
use crate::media_type::MediaType;
use crate::tsc_config::TsConfig;

use deno_core::error::anyhow;
use deno_core::error::bail;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::json_op_sync;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EmittedFile {
  pub data: String,
  pub maybe_specifiers: Option<Vec<ModuleSpecifier>>,
  pub media_type: MediaType,
}

impl std::fmt::Display for EmittedFile {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    f.write_str(&self.data)
  }
}

pub struct CheckerState {
  hash_data: Vec<Vec<u8>>,
  emitted_files: Vec<EmittedFile>,
  maybe_build_info: Option<TextDocument>,
  maybe_result: Option<CheckerResult>,
  provider: Rc<RefCell<dyn ModuleProvider>>,
}

impl CheckerState {
  pub fn new(
    provider: Rc<RefCell<dyn ModuleProvider>>,
    hash_data: Vec<Vec<u8>>,
    maybe_build_info: Option<TextDocument>,
  ) -> Self {
    CheckerState {
      hash_data,
      emitted_files: Vec::new(),
      maybe_build_info,
      maybe_result: None,
      provider,
    }
  }
}

fn checker_op<F>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut CheckerState, Value) -> Result<Value, AnyError> + 'static,
{
  json_op_sync(move |s, args, _bufs| {
    let state = s.borrow_mut::<CheckerState>();
    op_fn(state, args)
  })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateHashArgs {
  /// The string data to be used to generate the hash.  This will be mixed with
  /// other state data in Deno to derive the final hash.
  data: String,
}

fn op_create_hash(
  state: &mut CheckerState,
  args: Value,
) -> Result<Value, AnyError> {
  let v: CreateHashArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_create_hash\".")?;
  let mut data = vec![v.data.as_bytes().to_owned()];
  data.extend_from_slice(&state.hash_data);
  let hash = crate::checksum::gen(&data);

  Ok(json!({ "hash": hash }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  /// The text data/contents of the file.
  data: String,
  /// The _internal_ filename for the file.  This will be used to determine how
  /// the file is cached and stored.
  file_name: String,
  /// A string representation of the specifier that was associated with a
  /// module.  This should be present on every module that represents a module
  /// that was requested to be transformed.
  maybe_specifiers: Option<Vec<String>>,
}

fn op_emit(state: &mut CheckerState, args: Value) -> Result<Value, AnyError> {
  let v: EmitArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_emit\".")?;
  match v.file_name.as_ref() {
    "deno:///.tsbuildinfo" => {
      state.maybe_build_info = Some(TextDocument::new(
        v.data.as_bytes().to_owned(),
        Option::<&str>::None,
      ))
    }
    _ => state.emitted_files.push(EmittedFile {
      data: v.data,
      maybe_specifiers: if let Some(specifiers) = &v.maybe_specifiers {
        let specifiers = specifiers
          .iter()
          .map(|s| ModuleSpecifier::resolve_url_or_path(s).unwrap())
          .collect();
        Some(specifiers)
      } else {
        None
      },
      media_type: MediaType::from(&v.file_name),
    }),
  }

  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveArgs {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  base: String,
  /// A list of specifiers that should be resolved.
  specifiers: Vec<String>,
}

fn op_resolve(
  state: &mut CheckerState,
  args: Value,
) -> Result<Value, AnyError> {
  let provider = state.provider.borrow();
  let v: ResolveArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_resolve\".")?;
  let mut resolved: Vec<(String, String)> = Vec::new();
  let referrer = ModuleSpecifier::resolve_url_or_path(&v.base).context(
    "Error converting a string module specifier for \"op_resolve\".",
  )?;
  for specifier in &v.specifiers {
    if specifier.starts_with("asset:///") {
      resolved.push((
        specifier.clone(),
        MediaType::from(specifier).as_ts_extension().to_string(),
      ));
    } else {
      let resolved_specifier = provider.resolve(specifier, &referrer)?;
      let media_type = if let Some(media_type) =
        provider.get_media_type(&resolved_specifier)
      {
        media_type
      } else {
        bail!(
          "Unable to resolve media type for specifier: \"{}\"",
          resolved_specifier
        )
      };
      resolved.push((
        resolved_specifier.to_string(),
        media_type.as_ts_extension().to_string(),
      ));
    }
  }

  Ok(json!(resolved))
}

#[derive(Debug, Deserialize)]
struct LoadArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn op_load(state: &mut CheckerState, args: Value) -> Result<Value, AnyError> {
  let provider = state.provider.borrow();
  let v: LoadArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_load\".")?;
  let specifier = ModuleSpecifier::resolve_url_or_path(&v.specifier)
    .context("Error converting a string module specifier for \"op_load\".")?;
  let mut hash: Option<String> = None;
  let data = if &v.specifier == "deno:///.tsbuildinfo" {
    if let Some(build_info) = &state.maybe_build_info {
      Some(build_info.to_string()?)
    } else {
      None
    }
  } else {
    let maybe_source = provider.get_source(&specifier);
    if let Some(source) = &maybe_source {
      let mut data = vec![source.as_bytes().to_owned()];
      data.extend_from_slice(&state.hash_data);
      hash = Some(crate::checksum::gen(&data));
    }
    maybe_source
  };

  Ok(json!({ "data": data, "hash": hash }))
}

/// An internal structure that contains the final values from a request.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckerResult {
  pub diagnostics: Diagnostics,
  pub emit_skipped: bool,
  pub stats: Stats,
}

fn op_set_result(
  state: &mut CheckerState,
  args: Value,
) -> Result<Value, AnyError> {
  let v: CheckerResult = serde_json::from_value(args)
    .context("Error converting the result for \"op_set_result\".")?;
  state.maybe_result = Some(v);
  Ok(json!(true))
}

#[derive(Debug)]
pub struct ExecResult {
  /// Any diagnostics that have been returned from the checker.
  pub diagnostics: Diagnostics,
  /// Any files that were emitted during the check.
  pub emitted_files: Vec<EmittedFile>,
  /// If there was any build info associated with the exec request.
  pub maybe_build_info: Option<TextDocument>,
  /// Statistics from the check.
  pub stats: Stats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request<'a> {
  /// The compiler configuration, as a JSON value, that should be passed to the
  /// checker.
  pub config: &'a TsConfig,
  /// A flag which is passed to the runtime which indicates if debug messages
  /// should be logged or not.
  pub debug: bool,
  /// A list of root files that should be type checked.
  pub root_names: &'a Vec<String>,
}

/// Process a request to type check modules, based on the passed modules. The
/// check result will be returned.
pub fn exec<'a>(
  request: Request<'a>,
  snapshot: Snapshot,
  provider: Rc<RefCell<dyn ModuleProvider>>,
  hash_data: Vec<Vec<u8>>,
  maybe_build_info: Option<TextDocument>,
) -> Result<ExecResult, AnyError> {
  let mut checker_runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  {
    // Add the checker state to `op_state`
    let op_state = checker_runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(CheckerState::new(provider, hash_data, maybe_build_info));
  }

  // Register the ops for the runtime
  checker_runtime.register_op("op_create_hash", checker_op(op_create_hash));
  checker_runtime.register_op("op_emit", checker_op(op_emit));
  checker_runtime.register_op("op_load", checker_op(op_load));
  checker_runtime.register_op("op_resolve", checker_op(op_resolve));
  checker_runtime.register_op("op_set_result", checker_op(op_set_result));

  let startup_source = "globalThis.startup({ legacy: false })";
  let req_str = serde_json::to_string(&request)
    .context("Could not serialize request before execution.")?;
  let exec_source = format!("globalThis.exec({})", req_str);

  checker_runtime
    .execute("[native code]", startup_source)
    .context("failed to bootstrap the checker isolate")?;
  checker_runtime
    .execute("[native code]", &exec_source)
    .context("failed to execute a request to the checker isolate")?;

  let op_state = checker_runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.take::<CheckerState>();

  if let Some(result) = state.maybe_result {
    let diagnostics = result.diagnostics;
    let emitted_files = state.emitted_files;
    let maybe_build_info = state.maybe_build_info;
    let stats = result.stats;
    Ok(ExecResult {
      diagnostics,
      emitted_files,
      maybe_build_info,
      stats,
    })
  } else {
    Err(anyhow!("Checker result was not set."))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::diagnostics::Diagnostic;
  use crate::diagnostics::DiagnosticCategory;
  use crate::graph::tests::MockModuleProvider;
  use crate::graph::Stat;
  use crate::js;
  use std::collections::HashMap;

  #[test]
  fn test_op_create_hash() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let hash_data = vec![b"something".to_vec()];
    let mut state = CheckerState::new(provider, hash_data, None);
    let actual = op_create_hash(
      &mut state,
      json!({
        "data": "some sort of content"
      }),
    )
    .expect("should have returned");
    assert_eq!(
      actual,
      json!({
        "hash": "ae92df8f104748768838916857a1623b6a3c593110131b0a00f81ad9dac16511"
      })
    );
  }

  #[test]
  fn test_op_emit() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    op_emit(
      &mut state,
      json!({
        "data": "some file content",
        "fileName": "cache:///some/file.js",
        "maybeSpecifiers": ["file:///some/file.ts"]
      }),
    )
    .expect("should have not errored");
    assert_eq!(state.emitted_files.len(), 1);
    assert!(state.maybe_build_info.is_none());
    assert_eq!(
      state.emitted_files[0],
      EmittedFile {
        data: "some file content".to_string(),
        maybe_specifiers: Some(vec![ModuleSpecifier::resolve_url_or_path(
          "file:///some/file.ts"
        )
        .unwrap()]),
        media_type: MediaType::JavaScript,
      }
    );
  }

  #[test]
  fn test_op_emit_build_info() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    op_emit(
      &mut state,
      json!({
        "data": "some file content",
        "fileName": "deno:///.tsbuildinfo",
      }),
    )
    .expect("should not have errored");
    assert_eq!(state.emitted_files.len(), 0);
    assert_eq!(
      state.maybe_build_info,
      Some(TextDocument::new(
        b"some file content".to_vec(),
        Option::<&str>::None
      ))
    );
  }

  #[test]
  fn test_op_load() {
    let mut sources = HashMap::new();
    sources.insert(
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .unwrap(),
      "some file content".to_string(),
    );
    let provider = Rc::new(RefCell::new(MockModuleProvider {
      sources,
      ..Default::default()
    }));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    let actual = op_load(
      &mut state,
      json!({
        "specifier": "https://deno.land/x/mod.ts"
      }),
    )
    .expect("op should not have errored");
    assert_eq!(
      actual,
      json!({
        "data": "some file content",
        "hash": "b05ffa4eea8fb5609d576a68c1066be3f99e4dc53d365a0ac2a78259b2dd91f9"
      })
    );
  }

  #[test]
  fn test_op_load_buildinfo() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(
      provider,
      Vec::new(),
      Some(TextDocument::new(
        b"some build info".to_vec(),
        Option::<&str>::None,
      )),
    );
    let actual = op_load(
      &mut state,
      json!({
        "specifier": "deno:///.tsbuildinfo"
      }),
    )
    .expect("op should not have errored");
    assert_eq!(
      actual,
      json!({
        "data": "some build info",
        "hash": null
      })
    );
  }

  #[test]
  fn test_op_load_error() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    let actual = op_load(
      &mut state,
      json!({
        "specifier": "https://deno.land/x/mod.ts"
      }),
    )
    .expect("should not have errored");
    assert_eq!(
      actual,
      json!({
        "data": null,
        "hash": null
      })
    );
  }

  #[test]
  fn test_op_resolve() {
    let mut main_deps = HashMap::new();
    main_deps.insert(
      "./a.ts".to_string(),
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap(),
    );
    let mut resolution_map = HashMap::new();
    resolution_map.insert(
      ModuleSpecifier::resolve_url_or_path("file:///main.ts").unwrap(),
      main_deps,
    );
    let mut media_types = HashMap::new();
    media_types.insert(
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap(),
      MediaType::TypeScript,
    );
    let provider = Rc::new(RefCell::new(MockModuleProvider {
      resolution_map,
      media_types,
      ..Default::default()
    }));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    let actual = op_resolve(
      &mut state,
      json!({
        "base": "file:///main.ts",
        "specifiers": [ "./a.ts" ],
      }),
    )
    .expect("should have resolved");
    assert_eq!(actual, json!([["file:///a.ts", ".ts"]]));
  }

  #[test]
  fn test_op_resolve_error() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    op_resolve(
      &mut state,
      json!({
        "base": "file:///main.ts",
        "specifiers": ["./a.ts"]
      }),
    )
    .expect_err("should have failed");
  }

  #[test]
  fn test_op_set_result() {
    let provider = Rc::new(RefCell::new(MockModuleProvider::default()));
    let mut state = CheckerState::new(provider, Vec::new(), None);
    op_set_result(
      &mut state,
      json!({
        "diagnostics": [
          {
            "messageText": "Unknown compiler option 'invalid'.",
            "category": 1,
            "code": 5023
          }
        ],
        "emitSkipped": false,
        "stats": [["a", 12]]
      }),
    )
    .expect("should not error");
    assert_eq!(
      state.maybe_result,
      Some(CheckerResult {
        diagnostics: Diagnostics(vec![Diagnostic {
          category: DiagnosticCategory::Error,
          code: 5023,
          start: None,
          end: None,
          message_text: Some(
            "Unknown compiler option \'invalid\'.".to_string()
          ),
          message_chain: None,
          source: None,
          source_line: None,
          file_name: None,
          related_information: None,
        }]),
        emit_skipped: false,
        stats: Stats(vec![Stat::new("a".to_string(), 12)])
      })
    );
  }

  #[test]
  fn test_exec() {
    let mut sources = HashMap::new();
    sources.insert(
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .unwrap(),
      "import * as a from \"./a.ts\";\n\nconsole.log(a);".to_string(),
    );
    sources.insert(
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap(),
      "export const a = \"a\";\n".to_string(),
    );
    let mut mod_deps = HashMap::new();
    mod_deps.insert(
      "./a.ts".to_string(),
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap(),
    );
    let mut resolution_map = HashMap::new();
    resolution_map.insert(
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .unwrap(),
      mod_deps,
    );
    let mut media_types = HashMap::new();
    media_types.insert(
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap(),
      MediaType::TypeScript,
    );
    let provider = Rc::new(RefCell::new(MockModuleProvider {
      resolution_map,
      sources,
      media_types,
    }));
    let hash_data = vec![b"{}".to_vec(), b"1.2.3".to_vec()];
    let config = &TsConfig::new(json!({
      "allowJs": true,
      "checkJs": false,
      "esModuleInterop": true,
      "emitDecoratorMetadata": false,
      "incremental": true,
      "isolatedModules": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "lib": ["deno.window"],
      "module": "esnext",
      "noEmit": true,
      "outDir": "deno:///",
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "deno:///.tsbuildinfo",
    }));
    let actual = exec(
      Request {
        config,
        debug: false,
        root_names: &vec!["https://deno.land/x/mod.ts".to_string()],
      },
      js::compiler_isolate_init(),
      provider,
      hash_data,
      None,
    )
    .expect("should not have errored");
    assert_eq!(actual.stats.0.len(), 12);
    assert_eq!(actual.diagnostics.0.len(), 0);
    assert_eq!(actual.emitted_files.len(), 0);
    assert!(actual.maybe_build_info.is_some());
  }
}
