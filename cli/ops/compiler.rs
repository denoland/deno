// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::Deserialize;
use super::dispatch_json::JsonOp;
use super::dispatch_json::Value;
use crate::futures::future::try_join_all;
use crate::msg;
use crate::op_error::OpError;
use crate::state::State;
use deno_core::ModuleLoader;
use deno_core::*;
use futures::future::FutureExt;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_cache", s.stateful_json_op(op_cache));
  i.register_op("op_resolve_modules", s.stateful_json_op(op_resolve_modules));
  i.register_op(
    "op_fetch_source_files",
    s.stateful_json_op(op_fetch_source_files),
  );
  let custom_assets = std::collections::HashMap::new(); // TODO(ry) use None.
  i.register_op(
    "op_fetch_asset",
    deno_typescript::op_fetch_asset(custom_assets),
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CacheArgs = serde_json::from_value(args)?;

  let module_specifier = ModuleSpecifier::resolve_url(&args.module_id)
    .expect("Should be valid module specifier");

  let state_ = &state.borrow().global_state;
  let ts_compiler = state_.ts_compiler.clone();
  let fut = ts_compiler.cache_compiler_output(
    &module_specifier,
    &args.extension,
    &args.contents,
  );

  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize, Debug)]
struct SpecifiersReferrerArgs {
  specifiers: Vec<String>,
  referrer: Option<String>,
}

fn op_resolve_modules(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SpecifiersReferrerArgs = serde_json::from_value(args)?;
  let (referrer, is_main) = if let Some(referrer) = args.referrer {
    (referrer, false)
  } else {
    ("<unknown>".to_owned(), true)
  };

  let mut specifiers = vec![];

  for specifier in &args.specifiers {
    let specifier = state
      .resolve(specifier, &referrer, is_main)
      .map_err(OpError::from)?;
    specifiers.push(specifier.as_str().to_owned());
  }

  Ok(JsonOp::Sync(json!(specifiers)))
}

fn op_fetch_source_files(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SpecifiersReferrerArgs = serde_json::from_value(args)?;

  let ref_specifier = if let Some(referrer) = args.referrer {
    let specifier = ModuleSpecifier::resolve_url(&referrer)
      .expect("Referrer is not a valid specifier");
    Some(specifier)
  } else {
    None
  };

  let global_state = state.borrow().global_state.clone();
  let file_fetcher = global_state.file_fetcher.clone();
  let specifiers = args.specifiers.clone();
  let future = async move {
    let file_futures: Vec<_> = specifiers
      .into_iter()
      .map(|specifier| {
        let file_fetcher_ = file_fetcher.clone();
        let ref_specifier_ = ref_specifier.clone();
        async move {
          let resolved_specifier = ModuleSpecifier::resolve_url(&specifier)
            .expect("Invalid specifier");
          file_fetcher_
            .fetch_source_file(&resolved_specifier, ref_specifier_)
            .await
        }
        .boxed_local()
      })
      .collect();

    let files = try_join_all(file_futures).await.map_err(OpError::from)?;
    // We want to get an array of futures that resolves to
    let v = files.into_iter().map(|f| {
      async {
        // if the source file contains a `types_url` we need to replace
        // the module with the type definition when requested by the compiler
        let file = match f.types_url {
          Some(types_url) => {
            let types_specifier = ModuleSpecifier::from(types_url);
            global_state
              .file_fetcher
              .fetch_source_file(&types_specifier, ref_specifier.clone())
              .await
              .map_err(OpError::from)?
          }
          _ => f,
        };
        // Special handling of WASM and JSON files:
        // compile them into JS first!
        // This allows TS to do correct export types as well as bundles.
        let source_code = match file.media_type {
          msg::MediaType::Wasm => {
            global_state
              .wasm_compiler
              .compile(global_state.clone(), &file)
              .await
              .map_err(|e| OpError::other(e.to_string()))?
              .code
          }
          msg::MediaType::Json => {
            global_state
              .json_compiler
              .compile(&file)
              .await
              .map_err(|e| OpError::other(e.to_string()))?
              .code
          }
          _ => String::from_utf8(file.source_code).unwrap(),
        };
        Ok::<_, OpError>(json!({
          "url": file.url.to_string(),
          "filename": file.filename.to_str().unwrap(),
          "mediaType": file.media_type as i32,
          "sourceCode": source_code,
        }))
      }
    });

    let v = try_join_all(v).await?;
    Ok(v.into())
  }
  .boxed_local();

  Ok(JsonOp::Async(future))
}
