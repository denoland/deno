// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fmt_errors::PrettyJsError;
use crate::inspector::DenoInspector;
use crate::inspector::InspectorServer;
use crate::inspector::InspectorSession;
use crate::js;
use crate::metrics::Metrics;
use crate::module_loader::CliModuleLoader;
use crate::ops;
use crate::ops::worker_host::CreateWebWorkerCb;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use crate::version;
use crate::web_worker::WebWorker;
use crate::web_worker::WebWorkerOptions;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
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

pub fn create_web_worker_callback(
  program_state: Arc<ProgramState>,
) -> Arc<CreateWebWorkerCb> {
  Arc::new(
    move |name, worker_id, permissions, main_module, has_deno_namespace| {
      let global_state_ = program_state.clone();
      let js_error_create_fn = Rc::new(move |core_js_error| {
        let source_mapped_error =
          apply_source_map(&core_js_error, global_state_.clone());
        PrettyJsError::create(source_mapped_error)
      });

      let attach_inspector = program_state.maybe_inspector_server.is_some()
        || program_state.flags.coverage;
      let maybe_inspector_server = program_state.maybe_inspector_server.clone();

      let module_loader =
        CliModuleLoader::new_for_worker(program_state.clone());
      let create_web_worker_cb =
        create_web_worker_callback(program_state.clone());

      let options = WebWorkerOptions {
        args: program_state.flags.argv.clone(),
        apply_source_maps: true,
        debug_flag: program_state
          .flags
          .log_level
          .map_or(false, |l| l == log::Level::Debug),
        unstable: program_state.flags.unstable,
        ca_filepath: program_state.flags.ca_file.clone(),
        seed: program_state.flags.seed,
        module_loader,
        create_web_worker_cb,
        js_error_create_fn: Some(js_error_create_fn),
        has_deno_namespace,
        attach_inspector,
        maybe_inspector_server,
      };

      let mut worker = WebWorker::from_options(
        name,
        permissions,
        main_module,
        worker_id,
        &options,
      );

      // NOTE(bartlomieju): ProgramState is CLI only construct,
      // hence we're not using it in `Self::from_options`.
      {
        let js_runtime = &mut worker.js_runtime;
        js_runtime
          .op_state()
          .borrow_mut()
          .put::<Arc<ProgramState>>(program_state.clone());
        // Applies source maps - works in conjuction with `js_error_create_fn`
        // above
        ops::errors::init(js_runtime);
        if has_deno_namespace {
          ops::runtime_compiler::init(js_runtime);
        }
      }
      worker.bootstrap(&options);

      worker
    },
  )
}

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

pub struct WorkerOptions {
  pub apply_source_maps: bool,
  pub args: Vec<String>,
  pub debug_flag: bool,
  pub unstable: bool,
  pub ca_filepath: Option<String>,
  pub seed: Option<u64>,
  pub module_loader: Rc<dyn ModuleLoader>,
  // Callback that will be invoked when creating new instance
  // of WebWorker
  // pub create_module_loader_cb: Arc<ops::worker_host::LoaderCb>,
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
  pub attach_inspector: bool,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub should_break_on_first_statement: bool,
}

impl MainWorker {
  pub fn new(
    program_state: &Arc<ProgramState>,
    main_module: ModuleSpecifier,
    permissions: Permissions,
  ) -> Self {
    let module_loader = CliModuleLoader::new(program_state.clone());

    let global_state_ = program_state.clone();

    let js_error_create_fn = Rc::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, global_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let attach_inspector = program_state.maybe_inspector_server.is_some()
      || program_state.flags.repl
      || program_state.flags.coverage;
    let maybe_inspector_server = program_state.maybe_inspector_server.clone();
    let should_break_on_first_statement =
      program_state.flags.inspect_brk.is_some();

    let create_web_worker_cb =
      create_web_worker_callback(program_state.clone());

    let options = WorkerOptions {
      apply_source_maps: true,
      args: program_state.flags.argv.clone(),
      debug_flag: program_state
        .flags
        .log_level
        .map_or(false, |l| l == log::Level::Debug),
      unstable: program_state.flags.unstable,
      ca_filepath: program_state.flags.ca_file.clone(),
      seed: program_state.flags.seed,
      js_error_create_fn: Some(js_error_create_fn),
      create_web_worker_cb,
      attach_inspector,
      maybe_inspector_server,
      should_break_on_first_statement,
      module_loader,
    };

    let mut worker = Self::from_options(main_module, permissions, &options);

    // NOTE(bartlomieju): ProgramState is CLI only construct,
    // hence we're not using it in `Self::from_options`.
    {
      let js_runtime = &mut worker.js_runtime;
      js_runtime
        .op_state()
        .borrow_mut()
        .put::<Arc<ProgramState>>(program_state.clone());
      // Applies source maps - works in conjuction with `js_error_create_fn`
      // above
      ops::errors::init(js_runtime);
      ops::runtime_compiler::init(js_runtime);
      worker.bootstrap(&options);
    }

    worker
  }

  pub fn from_options(
    main_module: ModuleSpecifier,
    permissions: Permissions,
    options: &WorkerOptions,
  ) -> Self {
    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(js::deno_isolate_init()),
      js_error_create_fn: options.js_error_create_fn.clone(),
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      ..Default::default()
    });

    let inspector = if options.attach_inspector {
      Some(DenoInspector::new(
        &mut js_runtime,
        options.maybe_inspector_server.clone(),
      ))
    } else {
      None
    };
    let should_break_on_first_statement =
      inspector.is_some() && options.should_break_on_first_statement;

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
        op_state.put::<Permissions>(permissions);
        op_state.put::<ops::UnstableChecker>(ops::UnstableChecker {
          unstable: options.unstable,
        });
      }

      ops::runtime::init(js_runtime, main_module);
      ops::fetch::init(js_runtime, options.ca_filepath.as_deref());
      ops::timers::init(js_runtime);
      ops::worker_host::init(
        js_runtime,
        None,
        options.create_web_worker_cb.clone(),
      );
      ops::crypto::init(js_runtime, options.seed);
      ops::reg_json_sync(js_runtime, "op_close", deno_core::op_close);
      ops::reg_json_sync(js_runtime, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        js_runtime,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::fs_events::init(js_runtime);
      ops::fs::init(js_runtime);
      ops::io::init(js_runtime);
      ops::net::init(js_runtime);
      ops::os::init(js_runtime);
      ops::permissions::init(js_runtime);
      ops::plugin::init(js_runtime);
      ops::process::init(js_runtime);
      ops::signal::init(js_runtime);
      ops::tls::init(js_runtime);
      ops::tty::init(js_runtime);
      ops::websocket::init(js_runtime, options.ca_filepath.as_deref());
    }
    {
      let op_state = js_runtime.op_state();
      let mut op_state = op_state.borrow_mut();
      let t = &mut op_state.resource_table;
      let (stdin, stdout, stderr) = ops::io::get_stdio();
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
  }

  pub fn bootstrap(&mut self, options: &WorkerOptions) {
    let runtime_options = json!({
      "args": options.args,
      "applySourceMaps": options.apply_source_maps,
      "debugFlag": options.debug_flag,
      "denoVersion": version::deno(),
      "noColor": !colors::use_color(),
      "pid": std::process::id(),
      "ppid": ops::runtime::ppid(),
      "target": env!("TARGET"),
      "tsVersion": version::TYPESCRIPT,
      "unstableFlag": options.unstable,
      "v8Version": version::v8(),
    });

    let script = format!(
      "bootstrap.mainRuntime({})",
      serde_json::to_string_pretty(&runtime_options).unwrap()
    );
    self
      .execute(&script)
      .expect("Failed to execute bootstrap script");
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
