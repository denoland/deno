// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::futures::FutureExt;
use crate::op_error::OpError;
use crate::state::State;
use crate::swc_util::AstParser;
use crate::tsc::runtime_bundle;
use crate::tsc::runtime_compile;
use crate::tsc::runtime_transpile;
use deno_core::CoreIsolate;
use deno_core::ModuleSpecifier;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_compile", s.stateful_json_op(op_compile));
  i.register_op("op_transpile", s.stateful_json_op(op_transpile));
  i.register_op("op_parse", s.stateful_json_op(op_parse));
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CompileArgs {
  root_name: String,
  sources: Option<HashMap<String, String>>,
  bundle: bool,
  options: Option<String>,
}

fn op_compile(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.compile");
  let args: CompileArgs = serde_json::from_value(args)?;
  let s = state.borrow();
  let global_state = s.global_state.clone();
  let permissions = s.permissions.clone();
  let fut = async move {
    let fut = if args.bundle {
      runtime_bundle(
        global_state,
        permissions,
        &args.root_name,
        &args.sources,
        &args.options,
      )
      .boxed_local()
    } else {
      runtime_compile(
        global_state,
        permissions,
        &args.root_name,
        &args.sources,
        &args.options,
      )
      .boxed_local()
    };

    fut.await
  }
  .boxed_local();
  Ok(JsonOp::Async(fut))
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

fn op_transpile(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.transpile");
  let args: TranspileArgs = serde_json::from_value(args)?;
  let s = state.borrow();
  let global_state = s.global_state.clone();
  let permissions = s.permissions.clone();
  let fut = async move {
    runtime_transpile(global_state, permissions, &args.sources, &args.options)
      .await
  }
  .boxed_local();
  Ok(JsonOp::Async(fut))
}

#[derive(Deserialize, Debug)]
struct ParseArgs {
  source: String,
}

// TODO(divy-work): Should we take swc parse options?
fn op_parse(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.ast");
  let args: ParseArgs = serde_json::from_value(args)?;
  let s = state.borrow();
  let global_state = s.global_state.clone();
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&args.source)?;
  let permissions = s.permissions.clone();
  let fut = async move {
    let out = global_state
      .file_fetcher
      .fetch_source_file(&module_specifier, None, permissions)
      .await?;
    let src = std::str::from_utf8(&out.source_code).unwrap();
    let parser = AstParser::new();
    parser.parse_module::<_, Result<Value, OpError>>(
      &module_specifier.to_string(),
      out.media_type,
      &src,
      |parse_result| {
        let module = parse_result.unwrap();
        Ok(serde_json::to_value(module)?)
      },
    )
  }
  .boxed_local();
  Ok(JsonOp::Async(fut))
}
