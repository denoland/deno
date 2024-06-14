// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::path::Path;
use std::rc::Rc;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::FsModuleLoader;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;

#[op2(fast)]
fn op_hello(#[string] text: &str) {
  println!("Hello {} from an op!", text);
}

deno_core::extension!(
  hello_runtime,
  ops = [op_hello],
  esm_entry_point = "ext:hello_runtime/bootstrap.js",
  esm = [dir "examples/extension", "bootstrap.js"]
);

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let js_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/extension/main.js");
  let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();
  eprintln!("Running {main_module}...");
  let mut worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    PermissionsContainer::allow_all(),
    WorkerOptions {
      module_loader: Rc::new(FsModuleLoader),
      extensions: vec![hello_runtime::init_ops_and_esm()],
      ..Default::default()
    },
  );
  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;
  Ok(())
}
