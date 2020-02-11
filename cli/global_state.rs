// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::JsCompiler;
use crate::compilers::JsonCompiler;
use crate::compilers::TargetLib;
use crate::compilers::TsCompiler;
use crate::compilers::WasmCompiler;
use crate::deno_dir;
use crate::deno_error::permission_denied;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::lockfile::Lockfile;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::progress::Progress;
use crate::shell::Shell;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use std;
use std::env;
use std::ops::Deref;
use std::path::Path;
use std::str;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Mutex as AsyncMutex;

/// Holds state of the program and can be accessed by V8 isolate.
#[derive(Clone)]
pub struct GlobalState(Arc<GlobalStateInner>);

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
pub struct GlobalStateInner {
  /// Flags parsed from `argv` contents.
  pub flags: flags::DenoFlags,
  /// Permissions parsed from `flags`.
  pub permissions: DenoPermissions,
  pub dir: deno_dir::DenoDir,
  pub progress: Progress,
  pub file_fetcher: SourceFileFetcher,
  pub js_compiler: JsCompiler,
  pub json_compiler: JsonCompiler,
  pub ts_compiler: TsCompiler,
  pub wasm_compiler: WasmCompiler,
  pub lockfile: Option<Mutex<Lockfile>>,
  pub compiler_starts: AtomicUsize,
  compile_lock: AsyncMutex<()>,
}

impl Deref for GlobalState {
  type Target = GlobalStateInner;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl GlobalState {
  pub fn new(flags: flags::DenoFlags) -> Result<Self, ErrBox> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root)?;

    // TODO(ry) Shell is a useless abstraction and should be removed at
    // some point.
    let shell = Arc::new(Mutex::new(Shell::new()));

    let progress = Progress::new();
    progress.set_callback(move |_done, _completed, _total, status, msg| {
      if !status.is_empty() {
        let mut s = shell.lock().unwrap();
        s.status(status, msg).expect("shell problem");
      }
    });

    let file_fetcher = SourceFileFetcher::new(
      dir.deps_cache.clone(),
      progress.clone(),
      !flags.reload,
      flags.cache_blacklist.clone(),
      flags.no_remote,
      flags.cached_only,
    )?;

    let ts_compiler = TsCompiler::new(
      file_fetcher.clone(),
      dir.gen_cache.clone(),
      !flags.reload,
      flags.config_path.clone(),
    )?;

    // Note: reads lazily from disk on first call to lockfile.check()
    let lockfile = if let Some(filename) = &flags.lock {
      Some(Mutex::new(Lockfile::new(filename.to_string())))
    } else {
      None
    };

    let inner = GlobalStateInner {
      dir,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      progress,
      file_fetcher,
      ts_compiler,
      js_compiler: JsCompiler {},
      json_compiler: JsonCompiler {},
      wasm_compiler: WasmCompiler::default(),
      lockfile,
      compiler_starts: AtomicUsize::new(0),
      compile_lock: AsyncMutex::new(()),
    };

    Ok(GlobalState(Arc::new(inner)))
  }

  pub async fn fetch_compiled_module(
    &self,
    module_specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    target_lib: TargetLib,
  ) -> Result<CompiledModule, ErrBox> {
    let state1 = self.clone();
    let state2 = self.clone();
    let module_specifier = module_specifier.clone();

    let out = self
      .file_fetcher
      .fetch_source_file_async(&module_specifier, maybe_referrer)
      .await?;

    // TODO(ry) Try to lift compile_lock as high up in the call stack for
    // sanity.
    let compile_lock = self.compile_lock.lock().await;

    let compiled_module = match out.media_type {
      msg::MediaType::Unknown => state1.js_compiler.compile_async(out).await,
      msg::MediaType::Json => state1.json_compiler.compile_async(&out).await,
      msg::MediaType::Wasm => {
        state1
          .wasm_compiler
          .compile_async(state1.clone(), &out)
          .await
      }
      msg::MediaType::TypeScript
      | msg::MediaType::TSX
      | msg::MediaType::JSX => {
        state1
          .ts_compiler
          .compile_async(state1.clone(), &out, target_lib)
          .await
      }
      msg::MediaType::JavaScript => {
        if state1.ts_compiler.compile_js {
          state2
            .ts_compiler
            .compile_async(state1.clone(), &out, target_lib)
            .await
        } else {
          state1.js_compiler.compile_async(out).await
        }
      }
    }?;
    drop(compile_lock);

    if let Some(ref lockfile) = state2.lockfile {
      let mut g = lockfile.lock().unwrap();
      if state2.flags.lock_write {
        g.insert(&compiled_module);
      } else {
        let check = match g.check(&compiled_module) {
          Err(e) => return Err(ErrBox::from(e)),
          Ok(v) => v,
        };
        if !check {
          eprintln!(
            "Subresource integrity check failed --lock={}\n{}",
            g.filename, compiled_module.name
          );
          std::process::exit(10);
        }
      }
    }
    Ok(compiled_module)
  }

  #[inline]
  pub fn check_read(&self, filename: &Path) -> Result<(), ErrBox> {
    self.permissions.check_read(filename)
  }

  #[inline]
  pub fn check_write(&self, filename: &Path) -> Result<(), ErrBox> {
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
    &self,
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
        self.check_read(Path::new(&filename))?;
        Ok(())
      }
      _ => Err(permission_denied()),
    }
  }

  #[cfg(test)]
  pub fn mock(argv: Vec<String>) -> GlobalState {
    GlobalState::new(flags::DenoFlags {
      argv,
      ..flags::DenoFlags::default()
    })
    .unwrap()
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(GlobalState::mock(vec![
    String::from("./deno"),
    String::from("hello.js"),
  ]));
}

#[test]
fn import_map_given_for_repl() {
  let _result = GlobalState::new(flags::DenoFlags {
    import_map_path: Some("import_map.json".to_string()),
    ..flags::DenoFlags::default()
  });
}
