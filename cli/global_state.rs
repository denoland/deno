// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::JsCompiler;
use crate::compilers::JsonCompiler;
use crate::compilers::TsCompiler;
use crate::compilers::WasmCompiler;
use crate::deno_dir;
use crate::deno_error::permission_denied;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::lockfile::Lockfile;
use crate::metrics::Metrics;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::progress::Progress;
use deno::ErrBox;
use deno::ModuleSpecifier;
use futures::Future;
use std;
use std::env;
use std::ops::Deref;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

/// Holds state of the program and can be accessed by V8 isolate.
pub struct ThreadSafeGlobalState(Arc<GlobalState>);

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct GlobalState {
  /// Vector of CLI arguments - these are user script arguments, all Deno specific flags are removed.
  pub argv: Vec<String>,
  /// Flags parsed from `argv` contents.
  pub flags: flags::DenoFlags,
  /// Entry script parsed from CLI arguments.
  pub main_module: Option<ModuleSpecifier>,
  /// Permissions parsed from `flags`.
  pub permissions: DenoPermissions,
  pub dir: deno_dir::DenoDir,
  pub metrics: Metrics,
  pub progress: Progress,
  pub file_fetcher: SourceFileFetcher,
  pub js_compiler: JsCompiler,
  pub json_compiler: JsonCompiler,
  pub ts_compiler: TsCompiler,
  pub wasm_compiler: WasmCompiler,
  pub lockfile: Option<Mutex<Lockfile>>,
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

impl ThreadSafeGlobalState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    progress: Progress,
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

    // Note: reads lazily from disk on first call to lockfile.check()
    let lockfile = if let Some(filename) = &flags.lock {
      Some(Mutex::new(Lockfile::new(filename.to_string())))
    } else {
      None
    };

    let state = GlobalState {
      main_module,
      dir,
      argv: argv_rest,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      metrics: Metrics::default(),
      progress,
      file_fetcher,
      ts_compiler,
      js_compiler: JsCompiler {},
      json_compiler: JsonCompiler {},
      wasm_compiler: WasmCompiler::default(),
      lockfile,
    };

    Ok(ThreadSafeGlobalState(Arc::new(state)))
  }

  pub fn fetch_compiled_module(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> impl Future<Item = CompiledModule, Error = ErrBox> {
    let state1 = self.clone();
    let state2 = self.clone();

    self
      .file_fetcher
      .fetch_source_file_async(&module_specifier)
      .and_then(move |out| match out.media_type {
        msg::MediaType::Unknown => state1.js_compiler.compile_async(&out),
        msg::MediaType::Json => state1.json_compiler.compile_async(&out),
        msg::MediaType::Wasm => {
          state1.wasm_compiler.compile_async(state1.clone(), &out)
        }
        msg::MediaType::TypeScript
        | msg::MediaType::TSX
        | msg::MediaType::JSX => {
          state1.ts_compiler.compile_async(state1.clone(), &out)
        }
        msg::MediaType::JavaScript => {
          if state1.ts_compiler.compile_js {
            state1.ts_compiler.compile_async(state1.clone(), &out)
          } else {
            state1.js_compiler.compile_async(&out)
          }
        }
      })
      .and_then(move |compiled_module| {
        if let Some(ref lockfile) = state2.lockfile {
          let mut g = lockfile.lock().unwrap();
          if state2.flags.lock_write {
            g.insert(&compiled_module);
          } else if !g.check(&compiled_module)? {
            eprintln!(
              "Subresource integrety check failed --lock={}\n{}",
              g.filename, compiled_module.name
            );
            std::process::exit(10);
          }
        }
        Ok(compiled_module)
      })
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
    )
    .unwrap()
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
  );
}
