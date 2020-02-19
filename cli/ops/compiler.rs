// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::Deserialize;
use super::dispatch_json::JsonOp;
use super::dispatch_json::Value;
use crate::futures::future::try_join_all;
use crate::msg;
use crate::ops::json_op;
use crate::state::State;
use deno_core::Loader;
use deno_core::*;

pub fn init(i: &mut Isolate, s: &State) {
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
) -> Result<JsonOp, ErrBox> {
  let args: CacheArgs = serde_json::from_value(args)?;

  let module_specifier = ModuleSpecifier::resolve_url(&args.module_id)
    .expect("Should be valid module specifier");

  state
    .borrow()
    .global_state
    .ts_compiler
    .cache_compiler_output(
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
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SpecifiersReferrerArgs = serde_json::from_value(args)?;
  let (referrer, is_main) = if let Some(referrer) = args.referrer {
    (referrer, false)
  } else {
    ("<unknown>".to_owned(), true)
  };

  let mut specifiers = vec![];

  for specifier in &args.specifiers {
    let resolved_specifier = state.resolve(specifier, &referrer, is_main);
    match resolved_specifier {
      Ok(ms) => specifiers.push(ms.as_str().to_owned()),
      Err(err) => return Err(err),
    }
  }

  Ok(JsonOp::Sync(json!(specifiers)))
}

fn op_fetch_source_files(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
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
  let global_state = state.borrow().global_state.clone();

  for specifier in &args.specifiers {
    let resolved_specifier =
      ModuleSpecifier::resolve_url(&specifier).expect("Invalid specifier");
    let fut = global_state
      .file_fetcher
      .fetch_source_file_async(&resolved_specifier, ref_specifier.clone());
    futures.push(fut);
  }

  let future = Box::pin(async move {
    let files = try_join_all(futures).await?;

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
              .fetch_source_file_async(&types_specifier, ref_specifier.clone())
              .await?
          }
          _ => f,
        };
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

#[derive(Deserialize, Debug)]
struct FetchRemoteAssetArgs {
  name: String,
}

fn op_fetch_asset(
  _state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FetchRemoteAssetArgs = serde_json::from_value(args)?;
  debug!("args.name: {}", args.name);

  let source_code =
    if let Some(source_code) = deno_typescript::get_asset(&args.name) {
      source_code.to_string()
    } else {
      panic!("Asset not found: \"{}\"", args.name)
    };

  Ok(JsonOp::Sync(json!({ "sourceCode": source_code })))
}
