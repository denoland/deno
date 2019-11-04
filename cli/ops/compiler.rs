// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::futures::future::join_all;
use crate::futures::Future;
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
  referrer: String,
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

  let mut futures = vec![];
  for specifier in &args.specifiers {
    let resolved_specifier =
      state.resolve(specifier, &args.referrer, false, is_dyn_import)?;
    let fut = state
      .global_state
      .file_fetcher
      .fetch_source_file_async(&resolved_specifier);
    futures.push(fut);
  }

  let future = join_all(futures)
    .map_err(ErrBox::from)
    .and_then(move |files| {
      let res = files
        .into_iter()
        .map(|file| {
          json!({
            "url": file.url.to_string(),
            "filename": file.filename.to_str().unwrap(),
            "mediaType": file.media_type as i32,
            "sourceCode": String::from_utf8(file.source_code).unwrap(),
          })
        })
        .collect();

      futures::future::ok(res)
    });

  Ok(JsonOp::Async(Box::new(future)))
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
