// Copyright 2018-2025 the Deno authors. MIT license.

use anyhow::Context;
use deno_core::*;
use std::rc::Rc;

#[op2]
fn op_use_state(
  state: &mut OpState,
  #[scoped] callback: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
  state.put(callback);
  Ok(())
}

extension!(
  op2_sample,
  ops = [op_use_state],
  esm_entry_point = "ext:op2_sample/op2.js",
  esm = [ dir "examples", "op2.js" ],
  docs = "A small example demonstrating op2 usage.", "Contains one op."
);

fn main() -> Result<(), anyhow::Error> {
  let module_name = "test.js";
  let module_code = "
      op2_sample.use_state(() => {
          console.log('Hello World');
      });
  "
  .to_string();

  let mut js_runtime = JsRuntime::new(deno_core::RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),
    extensions: vec![op2_sample::init()],
    ..Default::default()
  });

  let main_module = resolve_path(
    module_name,
    &std::env::current_dir()
      .context("Unable to get current working directory")?,
  )?;

  let future = async move {
    let mod_id = js_runtime
      .load_main_es_module_from_code(&main_module, module_code)
      .await?;

    let result = js_runtime.mod_evaluate(mod_id);
    js_runtime.run_event_loop(Default::default()).await?;
    result.await?;
    Ok::<(), anyhow::Error>(())
  };

  tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(future)
}
