// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod util;
use deno_core::anyhow::Error;
use deno_core::FsModuleLoader;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::rc::Rc;

fn main() -> Result<(), Error> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    println!("Usage: target/examples/debug/fs_module_loader <path_to_module>");
    std::process::exit(1);
  }
  let main_url = args[1].clone();
  println!("Run {}", main_url);

  let js_runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),
    ..Default::default()
  });

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

  let main_module = deno_core::resolve_path(&main_url)?;

  let future = util::run_event_loop(js_runtime, main_module);
  runtime.block_on(future)
}
