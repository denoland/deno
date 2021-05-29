// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::import_map::ImportMap;
use crate::module_graph::BundleType;
use crate::module_graph::EmitBundleOptions;
use crate::module_graph::EmitOptions;
use crate::module_graph::Graph;
use crate::module_graph::GraphBuilder;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::specifier_handler::MemoryHandler;
use crate::specifier_handler::SpecifierHandler;

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_async(rt, "op_emit", op_emit);
  super::reg_async(rt, "op_emit_bundle", op_emit_bundle);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  check: Option<bool>,
  compiler_options: Option<HashMap<String, Value>>,
  import_map: Option<Value>,
  import_map_path: Option<String>,
  root_specifier: String,
  sources: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitBundleArgs {
  #[serde(flatten)]
  emit_args: EmitArgs,
  #[serde(rename = "type")]
  bundle_type: Option<BundleType>,
}

async fn prepare_emit(
  state: &Rc<RefCell<OpState>>,
  args: EmitArgs,
) -> Result<(Graph, EmitOptions), AnyError> {
  let root_specifier = args.root_specifier;
  let program_state = state.borrow().borrow::<Arc<ProgramState>>().clone();
  let mut runtime_permissions = {
    let state = state.borrow();
    state.borrow::<Permissions>().clone()
  };
  // when we are actually resolving modules without provided sources, we should
  // treat the root module as a dynamic import so that runtime permissions are
  // applied.
  let handler: Arc<Mutex<dyn SpecifierHandler>> =
    if let Some(sources) = args.sources {
      Arc::new(Mutex::new(MemoryHandler::new(sources)))
    } else {
      Arc::new(Mutex::new(FetchHandler::new(
        &program_state,
        runtime_permissions.clone(),
        runtime_permissions.clone(),
      )?))
    };
  let maybe_import_map = if let Some(import_map_str) = args.import_map_path {
    let import_map_specifier = resolve_url_or_path(&import_map_str)
      .context(format!("Bad URL (\"{}\") for import map.", import_map_str))?;
    let import_map = if let Some(value) = args.import_map {
      ImportMap::from_json(import_map_specifier.as_str(), &value.to_string())?
    } else {
      let file = program_state
        .file_fetcher
        .fetch(&import_map_specifier, &mut runtime_permissions)
        .await
        .map_err(|e| {
          generic_error(format!(
            "Unable to load '{}' import map: {}",
            import_map_specifier, e
          ))
        })?;
      ImportMap::from_json(import_map_specifier.as_str(), &file.source)?
    };
    Some(import_map)
  } else if args.import_map.is_some() {
    return Err(generic_error("An importMap was specified, but no importMapPath was provided, which is required."));
  } else {
    None
  };
  let mut builder = GraphBuilder::new(handler, maybe_import_map, None);
  let root_specifier = resolve_url_or_path(&root_specifier)?;
  builder.add(&root_specifier, false).await.map_err(|_| {
    type_error(format!(
      "Unable to handle the given specifier: {}",
      &root_specifier
    ))
  })?;
  builder
    .analyze_compiler_options(&args.compiler_options)
    .await?;
  Ok((
    builder.get_graph(),
    EmitOptions {
      check: args.check.unwrap_or(true),
      debug: program_state.flags.log_level == Some(log::Level::Debug),
      maybe_user_config: args.compiler_options,
    },
  ))
}

async fn op_emit(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _: (),
) -> Result<Value, AnyError> {
  deno_runtime::ops::check_unstable2(&state, "Deno.emit");
  let args: EmitArgs = serde_json::from_value(args)?;
  let (graph, emit_options) = prepare_emit(&state, args).await?;
  let result = graph.emit(emit_options)?;
  Ok(json!({
    "diagnostics": result.info.diagnostics,
    "modules": result.modules,
    "ignoredOptions": result.info.maybe_ignored_options,
    "stats": result.info.stats,
  }))
}

async fn op_emit_bundle(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _: (),
) -> Result<Value, AnyError> {
  deno_runtime::ops::check_unstable2(&state, "Deno.emitBundle");
  let args: EmitBundleArgs = serde_json::from_value(args)?;
  let (graph, emit_options) = prepare_emit(&state, args.emit_args).await?;
  let result = graph.emit_bundle(EmitBundleOptions {
    emit_options,
    bundle_type: args.bundle_type.unwrap_or(BundleType::Module),
  })?;
  Ok(json!({
    "diagnostics": result.info.diagnostics,
    "code": result.code,
    "map": result.map,
    "ignoredOptions": result.info.maybe_ignored_options,
    "stats": result.info.stats,
  }))
}
