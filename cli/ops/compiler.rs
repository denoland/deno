// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::Deserialize;
use super::dispatch_json::JsonOp;
use super::dispatch_json::Value;
use crate::futures::future::try_join_all;
use crate::msg;
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;

pub fn init(i: &mut CoreIsolate, s: &State) {
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
          // TODO(bartlomieju): duplicated from `state.rs::ModuleLoader::load` - deduplicate
          // Verify that remote file doesn't try to statically import local file.
          if let Some(referrer) = ref_specifier_.as_ref() {
            let referrer_url = referrer.as_url();
            match referrer_url.scheme() {
              "http" | "https" => {
                let specifier_url = resolved_specifier.as_url();
                match specifier_url.scheme() {
                  "http" | "https" => {},
                  _ => {
                    let e = OpError::permission_denied("Remote module are not allowed to statically import local modules. Use dynamic import instead.".to_string());
                    return Err(e.into());
                  }
                }
              },
              _ => {}
            }
          }
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
          _ => String::from_utf8(file.source_code)
            .map_err(|_| OpError::invalid_utf8())?,
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
