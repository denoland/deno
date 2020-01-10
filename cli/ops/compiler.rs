// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::compilers::runtime_compile_async;
use crate::compilers::runtime_transpile_async;
use crate::futures::future::try_join_all;
use crate::msg;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno_core::Loader;
use deno_core::*;
use std::collections::HashMap;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("cache", s.core_op(json_op(s.stateful_op(op_cache))));
  i.register_op(
    "resolve_modules",
    s.core_op(json_op(s.stateful_op(op_resolve_modules))),
  );
  i.register_op(
    "fetch_source_files",
    s.core_op(json_op(s.stateful_op(op_fetch_source_files))),
  );
  i.register_op(
    "fetch_asset",
    s.core_op(json_op(s.stateful_op(op_fetch_asset))),
  );
  i.register_op("compile", s.core_op(json_op(s.stateful_op(op_compile))));
  i.register_op("transpile", s.core_op(json_op(s.stateful_op(op_transpile))));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheArgs {
  module_id: String,
  contents: String,
  extension: String,
}

fn op_cache(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CacheArgs = serde_json::from_value(args)?;

  let module_specifier = ModuleSpecifier::resolve_url(&args.module_id)
    .expect("Should be valid module specifier");

  state.global_state.ts_compiler.cache_compiler_output(
    &module_specifier,
    &args.extension,
    &args.contents,
  )?;

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize, Debug)]
struct SpecifiersReferrerArgs {
  specifiers: Vec<String>,
  referrer: Option<String>,
}

fn op_resolve_modules(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SpecifiersReferrerArgs = serde_json::from_value(args)?;

  // TODO(ry) Maybe a security hole. Only the compiler worker should have access
  // to this. Need a test to demonstrate the hole.
  let is_dyn_import = false;

  let (referrer, is_main) = if let Some(referrer) = args.referrer {
    (referrer, false)
  } else {
    ("<unknown>".to_owned(), true)
  };

  let mut specifiers = vec![];

  for specifier in &args.specifiers {
    let resolved_specifier =
      state.resolve(specifier, &referrer, is_main, is_dyn_import);
    match resolved_specifier {
      Ok(ms) => specifiers.push(ms.as_str().to_owned()),
      Err(err) => return Err(err),
    }
  }

  Ok(JsonOp::Sync(json!(specifiers)))
}

fn op_fetch_source_files(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SpecifiersReferrerArgs = serde_json::from_value(args)?;

  let ref_specifier = if let Some(referrer) = args.referrer {
    let specifier = ModuleSpecifier::resolve_url(&referrer)
      .expect("Referrer is not a valid specifier");
    Some(specifier)
  } else {
    None
  };

  let mut futures = vec![];
  for specifier in &args.specifiers {
    let resolved_specifier =
      ModuleSpecifier::resolve_url(&specifier).expect("Invalid specifier");
    let fut = state
      .global_state
      .file_fetcher
      .fetch_source_file_async(&resolved_specifier, ref_specifier.clone());
    futures.push(fut);
  }

  let global_state = state.global_state.clone();

  let future = Box::pin(async move {
    let files = try_join_all(futures).await?;

    // We want to get an array of futures that resolves to
    let v = files.into_iter().map(|file| {
      async {
        // Special handling of Wasm files:
        // compile them into JS first!
        // This allows TS to do correct export types.
        let source_code = match file.media_type {
          msg::MediaType::Wasm => {
            global_state
              .wasm_compiler
              .compile_async(global_state.clone(), &file)
              .await?
              .code
          }
          _ => String::from_utf8(file.source_code).unwrap(),
        };
        Ok::<_, ErrBox>(json!({
          "url": file.url.to_string(),
          "filename": file.filename.to_str().unwrap(),
          "mediaType": file.media_type as i32,
          "sourceCode": source_code,
        }))
      }
    });

    let v = try_join_all(v).await?;
    Ok(v.into())
  });

  Ok(JsonOp::Async(future))
}

#[derive(Deserialize)]
struct FetchAssetArgs {
  name: String,
}

fn op_fetch_asset(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FetchAssetArgs = serde_json::from_value(args)?;
  if let Some(source_code) = crate::js::get_asset(&args.name) {
    Ok(JsonOp::Sync(json!(source_code)))
  } else {
    panic!("op_fetch_asset bad asset {}", args.name)
  }
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CompileArgs = serde_json::from_value(args)?;
  Ok(JsonOp::Async(runtime_compile_async(
    state.global_state.clone(),
    &args.root_name,
    &args.sources,
    args.bundle,
    &args.options,
  )))
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

fn op_transpile(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: TranspileArgs = serde_json::from_value(args)?;
  Ok(JsonOp::Async(runtime_transpile_async(
    state.global_state.clone(),
    &args.sources,
    &args.options,
  )))
}
