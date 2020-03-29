// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::JsCompiler;
use crate::compilers::JsonCompiler;
use crate::compilers::TargetLib;
use crate::compilers::TsCompiler;
use crate::compilers::WasmCompiler;
use crate::deno_dir;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::http_cache;
use crate::inspector::InspectorServer;
use crate::lockfile::Lockfile;
use crate::msg;
use crate::permissions::DenoPermissions;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use std;
use std::env;
use std::ops::Deref;
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
  pub flags: flags::Flags,
  /// Permissions parsed from `flags`.
  pub permissions: DenoPermissions,
  pub dir: deno_dir::DenoDir,
  pub file_fetcher: SourceFileFetcher,
  pub js_compiler: JsCompiler,
  pub json_compiler: JsonCompiler,
  pub ts_compiler: TsCompiler,
  pub wasm_compiler: WasmCompiler,
  pub lockfile: Option<Mutex<Lockfile>>,
  pub compiler_starts: AtomicUsize,
  pub inspector_server: Option<InspectorServer>,
  compile_lock: AsyncMutex<()>,
}

impl Deref for GlobalState {
  type Target = GlobalStateInner;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl GlobalState {
  pub fn new(flags: flags::Flags) -> Result<Self, ErrBox> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root)?;
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location)?;

    let file_fetcher = SourceFileFetcher::new(
      http_cache,
      !flags.reload,
      flags.cache_blacklist.clone(),
      flags.no_remote,
      flags.cached_only,
      flags.ca_file.clone(),
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

    let inspector_server = if let Some(ref host) = flags.inspect {
      Some(InspectorServer::new(host, false))
    } else if let Some(ref host) = flags.inspect_brk {
      Some(InspectorServer::new(host, true))
    } else {
      None
    };

    let inner = GlobalStateInner {
      inspector_server,
      dir,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
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
      .fetch_source_file(&module_specifier, maybe_referrer)
      .await?;

    // TODO(ry) Try to lift compile_lock as high up in the call stack for
    // sanity.
    let compile_lock = self.compile_lock.lock().await;

    let compiled_module = match out.media_type {
      msg::MediaType::Unknown => state1.js_compiler.compile(out).await,
      msg::MediaType::Json => state1.json_compiler.compile(&out).await,
      msg::MediaType::Wasm => {
        state1.wasm_compiler.compile(state1.clone(), &out).await
      }
      msg::MediaType::TypeScript
      | msg::MediaType::TSX
      | msg::MediaType::JSX => {
        state1
          .ts_compiler
          .compile(state1.clone(), &out, target_lib)
          .await
      }
      msg::MediaType::JavaScript => {
        if state1.ts_compiler.compile_js {
          state2
            .ts_compiler
            .compile(state1.clone(), &out, target_lib)
            .await
        } else {
          state1.js_compiler.compile(out).await
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

  #[cfg(test)]
  pub fn mock(argv: Vec<String>) -> GlobalState {
    GlobalState::new(flags::Flags {
      argv,
      ..flags::Flags::default()
    })
    .unwrap()
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(GlobalState::mock(vec![]));
}

#[test]
fn import_map_given_for_repl() {
  let _result = GlobalState::new(flags::Flags {
    import_map_path: Some("import_map.json".to_string()),
    ..flags::Flags::default()
  });
}
