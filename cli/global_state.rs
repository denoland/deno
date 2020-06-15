// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::http_cache;
use crate::import_map::ImportMap;
use crate::lockfile::Lockfile;
use crate::module_graph::ModuleGraphFile;
use crate::module_graph::ModuleGraphLoader;
use crate::msg;
use crate::msg::MediaType;
use crate::op_error::OpError;
use crate::permissions::Permissions;
use crate::state::exit_unstable;
use crate::tsc::CompiledModule;
use crate::tsc::TargetLib;
use crate::tsc::TsCompiler;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
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
  pub permissions: Permissions,
  pub dir: deno_dir::DenoDir,
  pub file_fetcher: SourceFileFetcher,
  pub ts_compiler: TsCompiler,
  pub lockfile: Option<Mutex<Lockfile>>,
  pub compiler_starts: AtomicUsize,
  pub maybe_import_map: Option<ImportMap>,
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
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);

    let file_fetcher = SourceFileFetcher::new(
      http_cache,
      !flags.reload,
      flags.cache_blocklist.clone(),
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

    let maybe_import_map: Option<ImportMap> =
      match flags.import_map_path.as_ref() {
        None => None,
        Some(file_path) => {
          if !flags.unstable {
            exit_unstable("--importmap")
          }
          Some(ImportMap::load(file_path)?)
        }
      };

    let inner = GlobalStateInner {
      dir,
      permissions: Permissions::from_flags(&flags),
      flags,
      file_fetcher,
      ts_compiler,
      lockfile,
      maybe_import_map,
      compiler_starts: AtomicUsize::new(0),
      compile_lock: AsyncMutex::new(()),
    };
    Ok(GlobalState(Arc::new(inner)))
  }

  /// This function is called when new module load is
  /// initialized by the EsIsolate. Its resposibility is to collect
  /// all dependencies and if it is required then also perform TS typecheck
  /// and traspilation.
  pub async fn prepare_module_load(
    &self,
    module_specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    target_lib: TargetLib,
    permissions: Permissions,
    is_dyn_import: bool,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), ErrBox> {
    let module_specifier = module_specifier.clone();

    // TODO(ry) Try to lift compile_lock as high up in the call stack for
    // sanity.
    let compile_lock = self.compile_lock.lock().await;

    let mut module_graph_loader = ModuleGraphLoader::new(
      self.file_fetcher.clone(),
      maybe_import_map,
      permissions.clone(),
      is_dyn_import,
      false,
    );
    module_graph_loader
      .add_to_graph(&module_specifier, maybe_referrer)
      .await?;
    let module_graph = module_graph_loader.get_graph();

    let out = self
      .file_fetcher
      .fetch_cached_source_file(&module_specifier, permissions.clone())
      .expect("Source file not found");

    // Check if we need to compile files.
    let module_graph_files = module_graph.values().collect::<Vec<_>>();
    let should_compile = needs_compilation(
      self.ts_compiler.compile_js,
      out.media_type,
      &module_graph_files,
    );
    let allow_js = should_allow_js(&module_graph_files);

    if should_compile {
      self
        .ts_compiler
        .compile_module_graph(
          self.clone(),
          &out,
          target_lib,
          permissions,
          module_graph,
          allow_js,
        )
        .await?;
    }

    drop(compile_lock);

    Ok(())
  }

  // TODO(bartlomieju): this method doesn't need to be async anymore
  /// This method is used after `prepare_module_load` finishes and EsIsolate
  /// starts loading source and executing source code. This method shouldn't
  /// perform any IO (besides $DENO_DIR) and only operate on sources collected
  /// during `prepare_module_load`.
  pub async fn fetch_compiled_module(
    &self,
    module_specifier: ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<CompiledModule, ErrBox> {
    let state1 = self.clone();
    let state2 = self.clone();
    let module_specifier = module_specifier.clone();

    let out = self
      .file_fetcher
      .fetch_cached_source_file(&module_specifier, Permissions::allow_all())
      .expect("Cached source file doesn't exist");

    // TODO(ry) Try to lift compile_lock as high up in the call stack for
    // sanity.
    let compile_lock = self.compile_lock.lock().await;

    // Check if we need to compile files
    let was_compiled = match out.media_type {
      msg::MediaType::TypeScript
      | msg::MediaType::TSX
      | msg::MediaType::JSX => true,
      msg::MediaType::JavaScript => self.ts_compiler.compile_js,
      _ => false,
    };

    let compiled_module = if was_compiled {
      state1
        .ts_compiler
        .get_compiled_module(&out.url)
        .map_err(|e| {
          let msg = e.to_string();
          OpError::other(format!(
            "Failed to get compiled source code of {}.\nReason: {}",
            out.url, msg
          ))
        })?
    } else {
      CompiledModule {
        code: String::from_utf8(out.source_code.clone())?,
        name: out.url.to_string(),
      }
    };

    drop(compile_lock);

    if let Some(ref lockfile) = state2.lockfile {
      let mut g = lockfile.lock().unwrap();
      if state2.flags.lock_write {
        g.insert(&out.url, out.source_code);
      } else {
        let check = match g.check(&out.url, out.source_code) {
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

/// Determine if TS compiler should be run with `allowJs` setting on. This
/// is the case when there's either:
///  - a JavaScript file with non-JavaScript import
///  - JSX import
fn should_allow_js(module_graph_files: &[&ModuleGraphFile]) -> bool {
  module_graph_files.iter().any(|module_file| {
    if module_file.media_type == (MediaType::JSX as i32) {
      true
    } else if module_file.media_type == (MediaType::JavaScript as i32) {
      module_file.imports.iter().any(|import_desc| {
        let import_file = module_graph_files
          .iter()
          .find(|f| {
            f.specifier == import_desc.resolved_specifier.to_string().as_str()
          })
          .expect("Failed to find imported file");
        let media_type = import_file.media_type;
        media_type == (MediaType::TypeScript as i32)
          || media_type == (MediaType::TSX as i32)
          || media_type == (MediaType::JSX as i32)
      })
    } else {
      false
    }
  })
}

// Compilation happens if either:
// - `checkJs` is set to true in TS config
// - entry point is a TS file
// - any dependency in module graph is a TS file
fn needs_compilation(
  compile_js: bool,
  media_type: MediaType,
  module_graph_files: &[&ModuleGraphFile],
) -> bool {
  let mut needs_compilation = match media_type {
    msg::MediaType::TypeScript | msg::MediaType::TSX | msg::MediaType::JSX => {
      true
    }
    msg::MediaType::JavaScript => compile_js,
    _ => false,
  };

  needs_compilation |= module_graph_files.iter().any(|module_file| {
    let media_type = module_file.media_type;

    media_type == (MediaType::TypeScript as i32)
      || media_type == (MediaType::TSX as i32)
      || media_type == (MediaType::JSX as i32)
  });

  needs_compilation
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(GlobalState::mock(vec![]));
}

#[test]
fn test_should_allow_js() {
  use crate::module_graph::ImportDescriptor;

  assert!(should_allow_js(&[
    &ModuleGraphFile {
      specifier: "file:///some/file.ts".to_string(),
      url: "file:///some/file.ts".to_string(),
      redirect: None,
      filename: "some/file.ts".to_string(),
      imports: vec![],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::TypeScript as i32,
      source_code: "function foo() {}".to_string(),
    },
    &ModuleGraphFile {
      specifier: "file:///some/file1.js".to_string(),
      url: "file:///some/file1.js".to_string(),
      redirect: None,
      filename: "some/file1.js".to_string(),
      imports: vec![ImportDescriptor {
        specifier: "./file.ts".to_string(),
        resolved_specifier: ModuleSpecifier::resolve_url(
          "file:///some/file.ts",
        )
        .unwrap(),
        type_directive: None,
        resolved_type_directive: None,
      }],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::JavaScript as i32,
      source_code: "function foo() {}".to_string(),
    },
  ],));

  assert!(should_allow_js(&[
    &ModuleGraphFile {
      specifier: "file:///some/file.jsx".to_string(),
      url: "file:///some/file.jsx".to_string(),
      redirect: None,
      filename: "some/file.jsx".to_string(),
      imports: vec![],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::JSX as i32,
      source_code: "function foo() {}".to_string(),
    },
    &ModuleGraphFile {
      specifier: "file:///some/file.ts".to_string(),
      url: "file:///some/file.ts".to_string(),
      redirect: None,
      filename: "some/file.ts".to_string(),
      imports: vec![ImportDescriptor {
        specifier: "./file.jsx".to_string(),
        resolved_specifier: ModuleSpecifier::resolve_url(
          "file:///some/file.jsx",
        )
        .unwrap(),
        type_directive: None,
        resolved_type_directive: None,
      }],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::TypeScript as i32,
      source_code: "function foo() {}".to_string(),
    },
  ]));

  assert!(!should_allow_js(&[
    &ModuleGraphFile {
      specifier: "file:///some/file.js".to_string(),
      url: "file:///some/file.js".to_string(),
      redirect: None,
      filename: "some/file.js".to_string(),
      imports: vec![],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::JavaScript as i32,
      source_code: "function foo() {}".to_string(),
    },
    &ModuleGraphFile {
      specifier: "file:///some/file1.js".to_string(),
      url: "file:///some/file1.js".to_string(),
      redirect: None,
      filename: "some/file1.js".to_string(),
      imports: vec![ImportDescriptor {
        specifier: "./file.js".to_string(),
        resolved_specifier: ModuleSpecifier::resolve_url(
          "file:///some/file.js",
        )
        .unwrap(),
        type_directive: None,
        resolved_type_directive: None,
      }],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::JavaScript as i32,
      source_code: "function foo() {}".to_string(),
    },
  ],));
}

#[test]
fn test_needs_compilation() {
  assert!(!needs_compilation(
    false,
    MediaType::JavaScript,
    &[&ModuleGraphFile {
      specifier: "some/file.js".to_string(),
      url: "file:///some/file.js".to_string(),
      redirect: None,
      filename: "some/file.js".to_string(),
      imports: vec![],
      referenced_files: vec![],
      lib_directives: vec![],
      types_directives: vec![],
      type_headers: vec![],
      media_type: MediaType::JavaScript as i32,
      source_code: "function foo() {}".to_string(),
    }],
  ));

  assert!(!needs_compilation(false, MediaType::JavaScript, &[]));
  assert!(needs_compilation(true, MediaType::JavaScript, &[]));
  assert!(needs_compilation(false, MediaType::TypeScript, &[]));
  assert!(needs_compilation(false, MediaType::JSX, &[]));
  assert!(needs_compilation(false, MediaType::TSX, &[]));
  assert!(needs_compilation(
    false,
    MediaType::JavaScript,
    &[
      &ModuleGraphFile {
        specifier: "file:///some/file.ts".to_string(),
        url: "file:///some/file.ts".to_string(),
        redirect: None,
        filename: "some/file.ts".to_string(),
        imports: vec![],
        referenced_files: vec![],
        lib_directives: vec![],
        types_directives: vec![],
        type_headers: vec![],
        media_type: MediaType::TypeScript as i32,
        source_code: "function foo() {}".to_string(),
      },
      &ModuleGraphFile {
        specifier: "file:///some/file1.js".to_string(),
        url: "file:///some/file1.js".to_string(),
        redirect: None,
        filename: "some/file1.js".to_string(),
        imports: vec![],
        referenced_files: vec![],
        lib_directives: vec![],
        types_directives: vec![],
        type_headers: vec![],
        media_type: MediaType::JavaScript as i32,
        source_code: "function foo() {}".to_string(),
      },
    ],
  ));
}
