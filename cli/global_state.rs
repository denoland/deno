// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::JsCompiler;
use crate::compilers::JsonCompiler;
use crate::compilers::TsCompiler;
use crate::deno_dir;
use crate::deno_error::permission_denied;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::global_timer::GlobalTimer;
use crate::metrics::Metrics;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::progress::Progress;
use deno::ErrBox;
use deno::Loader;
use deno::ModuleSpecifier;
use futures::Future;
use std;
use std::env;
use std::ops::Deref;
use std::str;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

/// Isolate cannot be passed between threads but ThreadSafeGlobalState can.
/// ThreadSafeGlobalState satisfies Send and Sync. So any state that needs to be
/// accessed outside the main V8 thread should be inside ThreadSafeGlobalState.
pub struct ThreadSafeGlobalState(Arc<GlobalState>);

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct GlobalState {
  pub modules: Arc<Mutex<deno::Modules>>,
  pub main_module: Option<ModuleSpecifier>,
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub flags: flags::DenoFlags,
  pub permissions: DenoPermissions,
  pub metrics: Metrics,
  pub global_timer: Mutex<GlobalTimer>,
  pub start_time: Instant,
  pub progress: Progress,
  pub file_fetcher: SourceFileFetcher,
  pub js_compiler: JsCompiler,
  pub json_compiler: JsonCompiler,
  pub ts_compiler: TsCompiler,
}

impl Clone for ThreadSafeGlobalState {
  fn clone(&self) -> Self {
    ThreadSafeGlobalState(self.0.clone())
  }
}

impl Deref for ThreadSafeGlobalState {
  type Target = Arc<GlobalState>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Loader for ThreadSafeGlobalState {
  // TODO: implement Resolver trait and it should be implemented per worker
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
    is_dyn_import: bool,
  ) -> Result<ModuleSpecifier, ErrBox> {
    let module_specifier =
      ModuleSpecifier::resolve_import(specifier, referrer)?;

    if is_dyn_import {
      self.check_dyn_import(&module_specifier)?;
    }

    Ok(module_specifier)
  }

  /// Given an absolute url, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Box<deno::SourceCodeInfoFuture> {
    self.metrics.resolve_count.fetch_add(1, Ordering::SeqCst);
    let module_url_specified = module_specifier.to_string();
    Box::new(self.fetch_compiled_module(module_specifier).map(
      |compiled_module| deno::SourceCodeInfo {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      },
    ))
  }
}

impl ThreadSafeGlobalState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    progress: Progress,
    _include_deno_namespace: bool,
  ) -> Result<Self, ErrBox> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root)?;

    let file_fetcher = SourceFileFetcher::new(
      dir.deps_cache.clone(),
      progress.clone(),
      !flags.reload,
      flags.cache_blacklist.clone(),
      flags.no_fetch,
    )?;

    let ts_compiler = TsCompiler::new(
      file_fetcher.clone(),
      dir.gen_cache.clone(),
      !flags.reload,
      flags.config_path.clone(),
    )?;

    let main_module: Option<ModuleSpecifier> = if argv_rest.len() <= 1 {
      None
    } else {
      let root_specifier = argv_rest[1].clone();
      Some(ModuleSpecifier::resolve_url_or_path(&root_specifier)?)
    };

    let modules = Arc::new(Mutex::new(deno::Modules::new()));

    let state = GlobalState {
      main_module,
      modules,
      dir,
      argv: argv_rest,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      metrics: Metrics::default(),
      global_timer: Mutex::new(GlobalTimer::new()),
      start_time: Instant::now(),
      progress,
      file_fetcher,
      ts_compiler,
      js_compiler: JsCompiler {},
      json_compiler: JsonCompiler {},
    };

    Ok(ThreadSafeGlobalState(Arc::new(state)))
  }

  pub fn fetch_compiled_module(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> impl Future<Item = CompiledModule, Error = ErrBox> {
    let state_ = self.clone();

    self
      .file_fetcher
      .fetch_source_file_async(&module_specifier)
      .and_then(move |out| match out.media_type {
        msg::MediaType::Unknown => state_.js_compiler.compile_async(&out),
        msg::MediaType::Json => state_.json_compiler.compile_async(&out),
        msg::MediaType::TypeScript
        | msg::MediaType::TSX
        | msg::MediaType::JSX => {
          state_.ts_compiler.compile_async(state_.clone(), &out)
        }
        msg::MediaType::JavaScript => {
          if state_.ts_compiler.compile_js {
            state_.ts_compiler.compile_async(state_.clone(), &out)
          } else {
            state_.js_compiler.compile_async(&out)
          }
        }
      })
  }

  /// Read main module from argv
  pub fn main_module(&self) -> Option<ModuleSpecifier> {
    match &self.main_module {
      Some(module_specifier) => Some(module_specifier.clone()),
      None => None,
    }
  }

  #[inline]
  pub fn check_read(&self, filename: &str) -> Result<(), ErrBox> {
    self.permissions.check_read(filename)
  }

  #[inline]
  pub fn check_write(&self, filename: &str) -> Result<(), ErrBox> {
    self.permissions.check_write(filename)
  }

  #[inline]
  pub fn check_env(&self) -> Result<(), ErrBox> {
    self.permissions.check_env()
  }

  #[inline]
  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    self.permissions.check_net(hostname, port)
  }

  #[inline]
  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    self.permissions.check_net_url(url)
  }

  #[inline]
  pub fn check_run(&self) -> Result<(), ErrBox> {
    self.permissions.check_run()
  }

  pub fn check_dyn_import(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    let u = module_specifier.as_url();
    match u.scheme() {
      "http" | "https" => {
        self.check_net_url(u)?;
        Ok(())
      }
      "file" => {
        let filename = u
          .to_file_path()
          .unwrap()
          .into_os_string()
          .into_string()
          .unwrap();
        self.check_read(&filename)?;
        Ok(())
      }
      _ => Err(permission_denied()),
    }
  }

  #[cfg(test)]
  pub fn mock(argv: Vec<String>) -> ThreadSafeGlobalState {
    ThreadSafeGlobalState::new(
      flags::DenoFlags::default(),
      argv,
      Progress::new(),
      true,
    )
    .unwrap()
  }

  pub fn metrics_op_dispatched(
    &self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.metrics.ops_dispatched.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_control
      .fetch_add(bytes_sent_control, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_data
      .fetch_add(bytes_sent_data, Ordering::SeqCst);
  }

  pub fn metrics_op_completed(&self, bytes_received: usize) {
    self.metrics.ops_completed.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_received
      .fetch_add(bytes_received, Ordering::SeqCst);
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(ThreadSafeGlobalState::mock(vec![
    String::from("./deno"),
    String::from("hello.js"),
  ]));
}

#[test]
fn import_map_given_for_repl() {
  let _result = ThreadSafeGlobalState::new(
    flags::DenoFlags {
      import_map_path: Some("import_map.json".to_string()),
      ..flags::DenoFlags::default()
    },
    vec![String::from("./deno")],
    Progress::new(),
    true,
  );
}
