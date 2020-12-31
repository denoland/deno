// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::colors;
use crate::media_type::MediaType;
use crate::module_graph::BundleType;
use crate::module_graph::EmitOptions;
use crate::module_graph::GraphBuilder;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::specifier_handler::MemoryHandler;
use crate::specifier_handler::SpecifierHandler;
use crate::tsc_config;

use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_compile", op_compile);
  super::reg_json_async(rt, "op_transpile", op_transpile);
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CompileArgs {
  root_name: String,
  sources: Option<HashMap<String, String>>,
  bundle: bool,
  options: Option<String>,
}

async fn op_compile(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _data: BufVec,
) -> Result<Value, AnyError> {
  let args: CompileArgs = serde_json::from_value(args)?;
  if args.bundle {
    deno_runtime::ops::check_unstable2(&state, "Deno.bundle");
  } else {
    deno_runtime::ops::check_unstable2(&state, "Deno.compile");
  }
  let program_state = state.borrow().borrow::<Arc<ProgramState>>().clone();
  let runtime_permissions = {
    let state = state.borrow();
    state.borrow::<Permissions>().clone()
  };
  let handler: Arc<Mutex<dyn SpecifierHandler>> =
    if let Some(sources) = args.sources {
      Arc::new(Mutex::new(MemoryHandler::new(sources)))
    } else {
      Arc::new(Mutex::new(FetchHandler::new(
        &program_state,
        runtime_permissions,
      )?))
    };
  let mut builder = GraphBuilder::new(handler, None, None);
  let specifier = ModuleSpecifier::resolve_url_or_path(&args.root_name)
    .context("The root specifier is invalid.")?;
  builder.add(&specifier, false).await?;
  let graph = builder.get_graph();
  let bundle_type = if args.bundle {
    BundleType::Esm
  } else {
    BundleType::None
  };
  let debug = program_state.flags.log_level == Some(log::Level::Debug);
  let maybe_user_config: Option<HashMap<String, Value>> =
    if let Some(options) = args.options {
      Some(serde_json::from_str(&options)?)
    } else {
      None
    };
  let (emitted_files, result_info) = graph.emit(EmitOptions {
    bundle_type,
    debug,
    maybe_user_config,
  })?;

  Ok(json!({
    "emittedFiles": emitted_files,
    "diagnostics": result_info.diagnostics,
  }))
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

#[derive(Debug, Serialize)]
struct RuntimeTranspileEmit {
  source: String,
  map: Option<String>,
}

async fn op_transpile(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _data: BufVec,
) -> Result<Value, AnyError> {
  deno_runtime::ops::check_unstable2(&state, "Deno.transpileOnly");
  let args: TranspileArgs = serde_json::from_value(args)?;

  let mut compiler_options = tsc_config::TsConfig::new(json!({
    "checkJs": true,
    "emitDecoratorMetadata": false,
    "jsx": "react",
    "jsxFactory": "React.createElement",
    "jsxFragmentFactory": "React.Fragment",
    "inlineSourceMap": false,
  }));

  let user_options: HashMap<String, Value> = if let Some(options) = args.options
  {
    serde_json::from_str(&options)?
  } else {
    HashMap::new()
  };
  let maybe_ignored_options =
    compiler_options.merge_user_config(&user_options)?;
  // TODO(@kitsonk) these really should just be passed back to the caller
  if let Some(ignored_options) = maybe_ignored_options {
    info!("{}: {}", colors::yellow("warning"), ignored_options);
  }

  let emit_options: ast::EmitOptions = compiler_options.into();
  let mut emit_map = HashMap::new();

  for (specifier, source) in args.sources {
    let media_type = MediaType::from(&specifier);
    let parsed_module = ast::parse(&specifier, &source, &media_type)?;
    let (source, maybe_source_map) = parsed_module.transpile(&emit_options)?;

    emit_map.insert(
      specifier.to_string(),
      RuntimeTranspileEmit {
        source,
        map: maybe_source_map,
      },
    );
  }
  let result = serde_json::to_value(emit_map)?;
  Ok(result)
}
