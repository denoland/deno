// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::FsModuleLoader;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use std::path::Path;
use std::rc::Rc;

deno_core::extension!(hello_runtime, esm = ["hello_runtime_bootstrap.js"]);

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let js_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/hello_runtime.js");
  let main_module = deno_core::resolve_path(
    &js_path.to_string_lossy(),
    &std::env::current_dir()?,
  )?;
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
