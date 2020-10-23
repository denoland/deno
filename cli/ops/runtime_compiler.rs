// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::media_type::MediaType;
use crate::permissions::Permissions;
use crate::tsc::runtime_bundle;
use crate::tsc::runtime_compile;
use crate::tsc_config;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
  super::check_unstable2(&state, "Deno.compile");
  let args: CompileArgs = serde_json::from_value(args)?;
  let cli_state = super::global_state2(&state);
  let program_state = cli_state.clone();
  let permissions = {
    let state = state.borrow();
    state.borrow::<Permissions>().clone()
  };
  let fut = if args.bundle {
    runtime_bundle(
      &program_state,
      permissions,
      &args.root_name,
      &args.sources,
      &args.options,
    )
    .boxed_local()
  } else {
    runtime_compile(
      &program_state,
      permissions,
      &args.root_name,
      &args.sources,
      &args.options,
    )
    .boxed_local()
  };
  let result = fut.await?;
  Ok(result)
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
  super::check_unstable2(&state, "Deno.transpile");
  let args: TranspileArgs = serde_json::from_value(args)?;

  let mut compiler_options = tsc_config::TsConfig::new(json!({
    "checkJs": true,
    "emitDecoratorMetadata": false,
    "jsx": "react",
    "jsxFactory": "React.createElement",
    "jsxFragmentFactory": "React.Fragment",
    "esModuleInterop": true,
    "module": "esnext",
    "sourceMap": true,
    "scriptComments": true,
    "target": "esnext",
  }));

  let user_options = if let Some(options) = args.options {
    tsc_config::parse_raw_config(&options)?
  } else {
    json!({})
  };
  compiler_options.merge(&user_options);

  let emit_options: ast::EmitOptions = compiler_options.into();
  let mut emit_map = HashMap::new();

  for (specifier, source) in args.sources {
    let media_type = MediaType::from(&specifier);
    let module_specifier = ModuleSpecifier::resolve_url(&specifier)?;
    let parsed_module = ast::parse(&module_specifier, &source, &media_type)?;
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
