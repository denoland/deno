// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::futures::future::try_join_all;
use crate::futures::future::FutureExt;
use crate::futures::future::TryFutureExt;
use crate::msg;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::Loader;
use deno::*;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("cache", s.core_op(json_op(s.stateful_op(op_cache))));
  i.register_op(
    "fetch_source_files",
    s.core_op(json_op(s.stateful_op(op_fetch_source_files))),
  );
  i.register_op(
    "fetch_asset",
    s.core_op(json_op(s.stateful_op(op_fetch_asset))),
  );
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

#[derive(Deserialize)]
struct FetchSourceFilesArgs {
  specifiers: Vec<String>,
  referrer: Option<String>,
}

fn op_fetch_source_files(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FetchSourceFilesArgs = serde_json::from_value(args)?;

  // TODO(ry) Maybe a security hole. Only the compiler worker should have access
  // to this. Need a test to demonstrate the hole.
  let is_dyn_import = false;

  let (referrer, ref_specifier) = if let Some(referrer) = args.referrer {
    let specifier = ModuleSpecifier::resolve_url(&referrer)
      .expect("Referrer is not a valid specifier");
    (referrer, Some(specifier))
  } else {
    // main script import
    (".".to_string(), None)
  };

  let mut futures = vec![];
  for specifier in &args.specifiers {
    let resolved_specifier =
      state.resolve(specifier, &referrer, false, is_dyn_import)?;
    let fut = state
      .global_state
      .file_fetcher
      .fetch_source_file_async(&resolved_specifier, ref_specifier.clone());
    futures.push(fut);
  }

  let global_state = state.global_state.clone();

  let future = try_join_all(futures)
    .map_err(ErrBox::from)
    .and_then(move |files| {
      // We want to get an array of futures that resolves to
      let v: Vec<_> = files
        .into_iter()
        .map(|file| {
          // Special handling of Wasm files:
          // compile them into JS first!
          // This allows TS to do correct export types.
          if file.media_type == msg::MediaType::Wasm {
            return futures::future::Either::Left(
              global_state
                .wasm_compiler
                .compile_async(global_state.clone(), &file)
                .and_then(|compiled_mod| {
                  futures::future::ok((file, Some(compiled_mod.code)))
                }),
            );
          }
          futures::future::Either::Right(futures::future::ok((file, None)))
        })
        .collect();
      try_join_all(v)
    })
    .and_then(move |files_with_code| {
      let res = files_with_code
        .into_iter()
        .map(|(file, maybe_code)| {
          json!({
            "url": file.url.to_string(),
            "filename": file.filename.to_str().unwrap(),
            "mediaType": file.media_type as i32,
            "sourceCode": if let Some(code) = maybe_code {
              code
            } else {
              String::from_utf8(file.source_code).unwrap()
            },
          })
        })
        .collect();

      futures::future::ok(res)
    });

  Ok(JsonOp::Async(future.boxed()))
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
