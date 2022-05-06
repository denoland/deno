// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Error;
use deno_core::FsModuleLoader;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_core::*;
use std::rc::Rc;
use std::time::Duration;

fn main() -> Result<(), Error> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    println!("Usage: target/examples/debug/fs_module_loader <path_to_module>");
    std::process::exit(1);
  }
  let main_url = args[1].clone();
  println!("Run {}", main_url);

  let ext = Extension::builder().ops(vec![op_sleep::decl()]).build();

  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),
    extensions: vec![ext],
    ..Default::default()
  });

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

  let main_module = deno_core::resolve_path(&main_url)?;

  let future = async move {
    let mod_id = js_runtime.load_main_module(&main_module, None).await?;
    let _ = js_runtime.mod_evaluate(mod_id);
    js_runtime.run_event_loop(false).await?;
    Ok(())
  };
  runtime.block_on(future)
}

#[op]
async fn op_sleep() -> Result<(), Error> {
  tokio::time::sleep(Duration::from_secs(1)).await;
  Ok(())
}
