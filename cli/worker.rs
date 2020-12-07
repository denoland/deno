// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fmt_errors::PrettyJsError;
use crate::inspector::DenoInspector;
use crate::inspector::InspectorSession;
use crate::js;
use crate::metrics::Metrics;
use crate::module_loader::CliModuleLoader;
use crate::ops;
use crate::ops::io::get_stdio;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::url::Url;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use std::env;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

/// This worker is created and used by almost all
/// subcommands in Deno executable.
///
/// It provides ops available in the `Deno` namespace.
///
/// All `WebWorker`s created during program execution
/// are descendants of this worker.
pub struct MainWorker {
  inspector: Option<Box<DenoInspector>>,
  js_runtime: JsRuntime,
  should_break_on_first_statement: bool,
}

impl MainWorker {
  pub fn new(
    program_state: &Arc<ProgramState>,
    main_module: ModuleSpecifier,
    permissions: Permissions,
  ) -> Self {
    let module_loader =
      CliModuleLoader::new(program_state.maybe_import_map.clone());

    let global_state_ = program_state.clone();

    let js_error_create_fn = Box::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, global_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    Self::from_options(
      program_state,
      main_module,
      permissions,
      module_loader,
      Some(js_error_create_fn),
    )
  }

  pub fn from_options(
    program_state: &Arc<ProgramState>,
    main_module: ModuleSpecifier,
    permissions: Permissions,
    module_loader: Rc<dyn ModuleLoader>,
    js_error_create_fn: Option<Box<JsErrorCreateFn>>,
  ) -> Self {
    // TODO(bartlomieju): this is hacky way to not apply source
    // maps in JS
    let apply_source_maps = js_error_create_fn.is_some();

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(module_loader),
      startup_snapshot: Some(js::deno_isolate_init()),
      js_error_create_fn,
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      ..Default::default()
    });

    let inspector =
      if let Some(inspector_server) = &program_state.maybe_inspector_server {
        Some(DenoInspector::new(
          &mut js_runtime,
          Some(inspector_server.clone()),
        ))
      } else if program_state.flags.coverage_dir.is_some()
        || program_state.flags.repl
      {
        Some(DenoInspector::new(&mut js_runtime, None))
      } else {
        None
      };

    let should_break_on_first_statement =
      inspector.is_some() && program_state.flags.inspect_brk.is_some();

    let mut worker = Self {
      inspector,
      js_runtime,
      should_break_on_first_statement,
    };

    let js_runtime = &mut worker.js_runtime;
    {
      // All ops registered in this function depend on these
      {
        let op_state = js_runtime.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<ProgramState>>(program_state.clone());
        op_state.put::<Permissions>(permissions);
      }

      ops::runtime::init(js_runtime, main_module, apply_source_maps);
      ops::fetch::init(js_runtime, program_state.flags.ca_file.as_deref());
      ops::timers::init(js_runtime);
      ops::worker_host::init(js_runtime, None);
      ops::crypto::init(js_runtime, program_state.flags.seed);
      ops::reg_json_sync(js_runtime, "op_close", deno_core::op_close);
      ops::reg_json_sync(js_runtime, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        js_runtime,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(js_runtime);
      ops::fs_events::init(js_runtime);
      ops::fs::init(js_runtime);
      ops::io::init(js_runtime);
      ops::net::init(js_runtime);
      ops::os::init(js_runtime);
      ops::permissions::init(js_runtime);
      ops::plugin::init(js_runtime);
      ops::process::init(js_runtime);
      ops::runtime_compiler::init(js_runtime);
      ops::signal::init(js_runtime);
      ops::tls::init(js_runtime);
      ops::tty::init(js_runtime);
      ops::websocket::init(js_runtime);
    }
    {
      let op_state = js_runtime.op_state();
      let mut op_state = op_state.borrow_mut();
      let t = &mut op_state.resource_table;
      let (stdin, stdout, stderr) = get_stdio();
      if let Some(stream) = stdin {
        t.add("stdin", Box::new(stream));
      }
      if let Some(stream) = stdout {
        t.add("stdout", Box::new(stream));
      }
      if let Some(stream) = stderr {
        t.add("stderr", Box::new(stream));
      }
    }
    worker
      .execute("bootstrap.mainRuntime()")
      .expect("Failed to execute bootstrap script");
    worker
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), AnyError> {
    let path = env::current_dir().unwrap().join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.js_runtime.execute(url.as_str(), js_source)
  }

  /// Loads and instantiates specified JavaScript module.
  pub async fn preload_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, AnyError> {
    self.js_runtime.load_module(module_specifier, None).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_module(module_specifier).await?;
    self.wait_for_inspector_session();
    self.js_runtime.mod_evaluate(id).await
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .inspector
        .as_mut()
        .unwrap()
        .wait_for_session_and_break_on_next_statement()
    }
  }

  /// Create new inspector session. This function panics if Worker
  /// was not configured to create inspector.
  pub fn create_inspector_session(&mut self) -> Box<InspectorSession> {
    let inspector = self.inspector.as_mut().unwrap();

    InspectorSession::new(&mut **inspector)
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    // We always poll the inspector if it exists.
    let _ = self.inspector.as_mut().map(|i| i.poll_unpin(cx));
    self.js_runtime.poll_event_loop(cx)
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }
}

impl Drop for MainWorker {
  fn drop(&mut self) {
    // The Isolate object must outlive the Inspector object, but this is
    // currently not enforced by the type system.
    self.inspector.take();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags::DenoSubcommand;
  use crate::flags::Flags;
  use crate::program_state::ProgramState;

  fn create_test_worker() -> MainWorker {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("./hello.js").unwrap();
    let flags = Flags {
      subcommand: DenoSubcommand::Run {
        script: main_module.to_string(),
      },
      ..Default::default()
    };
    let permissions = Permissions::from_flags(&flags);
    let program_state =
      ProgramState::mock(vec!["deno".to_string()], Some(flags));
    MainWorker::new(&program_state, main_module, permissions)
  }

  #[tokio::test]
  async fn execute_mod_esm_imports_a() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/esm_imports_a.js");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = worker.run_event_loop().await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = worker.run_event_loop().await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_006_url_imports() {
    let _http_server_guard = test_util::http_server();
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/006_url_imports.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = worker.run_event_loop().await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_mod_resolve_error() {
    // "foo" is not a valid module specifier so this should return an error.
    let mut worker = create_test_worker();
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
    let result = worker.execute_module(&module_specifier).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let mut worker = create_test_worker();
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let result = worker.execute_module(&module_specifier).await;
    assert!(result.is_ok());
  }
}
