// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::state::ThreadSafeState;
use futures::Future;
use deno::*;

#[derive(Deserialize)]
struct DepsArgs {
  url: String,
}

pub fn op_deps(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DepsArgs = serde_json::from_value(args)?;

  let state_ = state.clone();
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&args.url)?;
  let module_specifier_ = module_specifier.clone();

  let fut = state_
    .file_fetcher
    .fetch_source_file_async(&module_specifier)
    .and_then(move |_out| {
      state_
        .fetch_compiled_module(&module_specifier_)
        .and_then(move |compiled| {
          let modules = state_.modules.lock().unwrap();

          let deps_json_string = match modules.deps(&compiled.name.to_string()) {
            None => "".to_string(),
            Some(deps) => deps.to_json_object(),
          };

          futures::future::ok(json!(deps_json_string))
        })
    });

  Ok(JsonOp::Async(Box::new(fut)))
}
