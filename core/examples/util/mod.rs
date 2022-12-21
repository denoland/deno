use anyhow::Error;
use deno_core::{JsRuntime, ModuleSpecifier};

pub(crate) async fn run_event_loop(
  mut js_runtime: JsRuntime,
  main_module: ModuleSpecifier,
) -> Result<(), Error> {
  let mod_id = js_runtime.load_main_module(&main_module, None).await?;
  let result = js_runtime.mod_evaluate(mod_id);
  js_runtime.run_event_loop(false).await?;
  result.await?
}
