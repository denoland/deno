// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::assets;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;

// Cache

pub struct OpCache;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheArgs {
  module_id: String,
  contents: String,
  extension: String,
}

impl DenoOpDispatcher for OpCache {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: CacheArgs = serde_json::from_value(args)?;

        let module_specifier = ModuleSpecifier::resolve_url(&args.module_id)
          .expect("Should be valid module specifier");

        state.ts_compiler.cache_compiler_output(
          &module_specifier,
          &args.extension,
          &args.contents,
        )?;

        Ok(JsonOp::Sync(json!({})))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "cache";
}

// Fetch Source File

pub struct OpFetchSourceFile;

#[derive(Deserialize)]
struct FetchSourceFileArgs {
  specifier: String,
  referrer: String,
}

impl DenoOpDispatcher for OpFetchSourceFile {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: FetchSourceFileArgs = serde_json::from_value(args)?;

        // TODO(ry) Maybe a security hole. Only the compiler worker should have access
        // to this. Need a test to demonstrate the hole.
        let is_dyn_import = false;

        let resolved_specifier = state.resolve(
          &args.specifier,
          &args.referrer,
          false,
          is_dyn_import,
        )?;

        let fut = state
          .file_fetcher
          .fetch_source_file_async(&resolved_specifier);

        // WARNING: Here we use tokio_util::block_on() which starts a new Tokio
        // runtime for executing the future. This is so we don't inadvernently run
        // out of threads in the main runtime.
        let out = tokio_util::block_on(fut)?;
        Ok(JsonOp::Sync(json!({
          "moduleName": out.url.to_string(),
          "filename": out.filename.to_str().unwrap(),
          "mediaType": out.media_type as i32,
          "sourceCode": String::from_utf8(out.source_code).unwrap(),
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "fetchSourceFile";
}

pub struct OpFetchAsset;

#[derive(Deserialize)]
struct FetchAssetArgs {
  name: String,
}

impl DenoOpDispatcher for OpFetchAsset {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: FetchAssetArgs = serde_json::from_value(args)?;
        if let Some(source_code) = assets::get_source_code(&args.name) {
          Ok(JsonOp::Sync(json!(source_code)))
        } else {
          panic!("op_fetch_asset bad asset {}", args.name)
        }
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "fetchAsset";
}
