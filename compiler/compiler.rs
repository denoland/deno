// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::config::json_merge;
use crate::config::parse_config;
use crate::module_graph::ModuleProvider;
use crate::msg::as_ts_filename;
use crate::msg::CompilerStats;
use crate::msg::Diagnostics;
use crate::msg::EmittedFile;
use crate::msg::IgnoredCompilerOptions;
use crate::msg::TranspileSourceFile;
use crate::ops;
use crate::ops::compiler_op;
use crate::ops::json_op;
use crate::CompileOptions;
use crate::Result;
use crate::TranspileOptions;

use deno_core::js_check;
use deno_core::CoreIsolate;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use serde::Deserialize;
use serde_json::json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub type Sources = HashMap<String, String>;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmitResult {
  pub emit_skipped: bool,
  pub diagnostics: Diagnostics,
  pub emitted_files: Option<Vec<String>>,
  pub stats: CompilerStats,
}

#[derive(Default)]
pub struct CompilerState {
  pub emit_result: Option<EmitResult>,
  pub hash_data: Vec<Vec<u8>>,
  pub maybe_assets_path: Option<PathBuf>,
  pub maybe_build_info: Option<String>,
  pub maybe_provider: Option<Rc<RefCell<dyn ModuleProvider>>>,
  pub maybe_shared_path: Option<String>,
  pub sources: Sources,
  pub version: String,
  pub written_files: Vec<EmittedFile>,
}

pub struct InternalCompileOptions<'a> {
  pub bundle: bool,
  pub check_only: bool,
  pub provider: Rc<RefCell<dyn ModuleProvider>>,
  pub root_names: Vec<&'a ModuleSpecifier>,
  pub maybe_build_info: Option<String>,
  pub maybe_shared_path: Option<String>,
  pub compile_options: CompileOptions<'a>,
}

pub struct InternalTranspileOptions {
  pub bundle: bool,
  pub sources: HashMap<String, TranspileSourceFile>,
  pub transpile_options: TranspileOptions,
}

#[derive(Debug, Clone, PartialEq)]
/// The result of a compilation.
pub struct CompilerEmit {
  pub cache_js: bool,
  /// Statistics returned from the compiler regarding the transpilation.
  pub stats: CompilerStats,
  pub maybe_build_info: Option<String>,
  /// If a configuration was supplied and the configuration contained options
  /// that were ignored, the would be returned here.
  pub maybe_ignored_options: Option<IgnoredCompilerOptions>,
  pub written_files: Vec<EmittedFile>,
}

/// Register operations that will be called from the compiler's JavaScript code.
fn register_ops(isolate: &mut CoreIsolate, state: &Arc<Mutex<CompilerState>>) {
  isolate.register_op(
    "op_create_hash",
    compiler_op(state.clone(), json_op(ops::op_create_hash)),
  );
  isolate.register_op(
    "op_load_module",
    compiler_op(state.clone(), json_op(ops::op_load_module)),
  );
  isolate.register_op(
    "op_read_file",
    compiler_op(state.clone(), json_op(ops::op_read_file)),
  );
  isolate.register_op(
    "op_resolve_specifiers",
    compiler_op(state.clone(), json_op(ops::op_resolve_specifiers)),
  );
  isolate.register_op(
    "op_set_emit_result",
    compiler_op(state.clone(), json_op(ops::op_set_emit_result)),
  );
  isolate.register_op(
    "op_set_version",
    compiler_op(state.clone(), json_op(ops::op_set_version)),
  );
  isolate.register_op(
    "op_write_file",
    compiler_op(state.clone(), json_op(ops::op_write_file)),
  );
}

/// Create a snapshot of a Deno TypeScript compiler.  The return value is the
/// version of the TypeScript compiler that the snapshot was created for.
///
/// # Arguments
///
/// * `snapshot_path` - the path the resulting snapshot (`.bin`) should be
///   written to.
/// * `assets_path` - the path that contains `typescript.js` and all of the
///   default `.d.ts` files which will be built into the snapshot.
/// * `custom_libs` - A hashmap that contains any custom libs to be registered
///   with the compiler snapshot.
///
/// # Examples
///
/// ```
/// use deno_compiler::create_compiler_snapshot;
///
/// # let c = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
/// # let mut custom_libs = std::collections::HashMap::new();
/// # custom_libs.insert("test".to_string(), c.join("fixtures/lib.test.d.ts"));
/// let version = create_compiler_snapshot(
///   std::env::temp_dir().join("TEST_COMPILER.bin"),
///   c.join("../cli/dts"),
///   custom_libs,
/// ).expect("snapshot creation failed");
/// ```
///
pub fn create_compiler_snapshot(
  snapshot_path: PathBuf,
  assets_path: PathBuf,
  custom_libs: HashMap<String, PathBuf>,
) -> Result<String> {
  let mut isolate = CoreIsolate::new(StartupData::None, true);
  let typescript_path = assets_path.join("../tsc/00_typescript.js");
  let typescript_source = fs::read_to_string(typescript_path)?;
  let compiler_source = fs::read_to_string("compiler.js")?;
  js_check(isolate.execute("typescript.js", &typescript_source));
  js_check(isolate.execute("compiler.js", &compiler_source));

  let mut libs: HashMap<&str, &str> = HashMap::new();
  libs.insert("dom", "lib.dom.d.ts");
  libs.insert("dom.iterable", "lib.dom.iterable.d.ts");
  libs.insert("es5", "lib.es5.d.ts");
  libs.insert("es6", "lib.es6.d.ts");
  libs.insert("es2015.collection", "lib.es2015.collection.d.ts");
  libs.insert("es2015.core", "lib.es2015.core.d.ts");
  libs.insert("es2015", "lib.es2015.d.ts");
  libs.insert("es2015.generator", "lib.es2015.generator.d.ts");
  libs.insert("es2015.iterable", "lib.es2015.iterable.d.ts");
  libs.insert("es2015.promise", "lib.es2015.promise.d.ts");
  libs.insert("es2015.proxy", "lib.es2015.proxy.d.ts");
  libs.insert("es2015.reflect", "lib.es2015.reflect.d.ts");
  libs.insert("es2015.symbol", "lib.es2015.symbol.d.ts");
  libs.insert(
    "es2015.symbol.wellknown",
    "lib.es2015.symbol.wellknown.d.ts",
  );
  libs.insert("es2016.array.include", "lib.es2016.array.include.d.ts");
  libs.insert("es2016", "lib.es2016.d.ts");
  libs.insert("es2017", "lib.es2017.d.ts");
  libs.insert("es2017.intl", "lib.es2017.intl.d.ts");
  libs.insert("es2017.object", "lib.es2017.object.d.ts");
  libs.insert("es2017.sharedmemory", "lib.es2017.sharedmemory.d.ts");
  libs.insert("es2017.string", "lib.es2017.string.d.ts");
  libs.insert("es2017.typedarrays", "lib.es2017.typedarrays.d.ts");
  libs.insert("es2018.asyncgenerator", "lib.es2018.asyncgenerator.d.ts");
  libs.insert("es2018.asynciterable", "lib.es2018.asynciterable.d.ts");
  libs.insert("es2018", "lib.es2018.d.ts");
  libs.insert("es2018.intl", "lib.es2018.intl.d.ts");
  libs.insert("es2018.promise", "lib.es2018.promise.d.ts");
  libs.insert("es2018.regexp", "lib.es2018.regexp.d.ts");
  libs.insert("es2019.array", "lib.es2019.array.d.ts");
  libs.insert("es2019", "lib.es2019.d.ts");
  libs.insert("es2019.object", "lib.es2019.object.d.ts");
  libs.insert("es2019.string", "lib.es2019.string.d.ts");
  libs.insert("es2019.symbol", "lib.es2019.symbol.d.ts");
  libs.insert("es2020.bigint", "lib.es2020.bigint.d.ts");
  libs.insert("es2020", "lib.es2020.d.ts");
  libs.insert("es2020.intl", "lib.es2020.intl.d.ts");
  libs.insert("es2020.promise", "lib.es2020.promise.d.ts");
  libs.insert("es2020.string", "lib.es2020.string.d.ts");
  libs.insert(
    "es2020.symbol.wellknown",
    "lib.es2020.symbol.wellknown.d.ts",
  );
  libs.insert("esnext", "lib.esnext.d.ts");
  libs.insert("esnext.intl", "lib.esnext.intl.d.ts");
  libs.insert("esnext.promise", "lib.esnext.promise.d.ts");
  libs.insert("esnext.string", "lib.esnext.string.d.ts");
  libs.insert("scripthost", "lib.scripthost.d.ts");
  libs.insert("webworker", "lib.webworker.d.ts");
  libs.insert(
    "webworker.importscripts",
    "lib.webworker.importscripts.d.ts",
  );

  let mut sources: HashMap<String, String> = HashMap::new();
  sources.insert(
    "file:///bootstrap.ts".to_string(),
    "console.log(\"hello deno\");\nexport {}; \n".to_string(),
  );

  for (lib, file_path) in custom_libs.iter() {
    let source = std::fs::read_to_string(file_path)?;
    let file_name = file_path.file_name().unwrap().to_str().unwrap();
    let specifier = format!("asset:///{}", file_name);
    libs.insert(lib, file_name);
    sources.insert(specifier, source);
  }

  let state = Arc::new(Mutex::new(CompilerState {
    maybe_assets_path: Some(assets_path),
    sources,
    ..Default::default()
  }));

  register_ops(&mut isolate, &state);

  let compiler_options = json!({
    "lib": ["esnext"],
    "module": "esnext",
    "target": "esnext",
  });

  let config = json!({
    "bootSpecifier": "file:///bootstrap.ts",
    "compilerOptions": compiler_options,
    "libs": libs,
  });

  let js_source = format!("main({})", config);

  js_check(isolate.execute("<anon>", &js_source));

  let snapshot = isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("[RS]: snapshot size: {}", snapshot_slice.len());
  fs::write(snapshot_path.clone(), snapshot_slice)?;
  println!("[RS]: snapshot written to: {} ", snapshot_path.display());
  let state = state.lock().expect("could not lock state");
  let version = state.version.clone();

  Ok(version)
}

/// An abstraction for a Deno isolate which allows code to be compiled and
/// transformed from TypeScript to JavaScript.
pub struct CompilerIsolate {
  isolate: CoreIsolate,
  state: Arc<Mutex<CompilerState>>,
}

impl CompilerIsolate {
  /// Returns an instance of a Deno compiler isolate based on the provided
  /// start-up data.
  ///
  /// # Arguments
  ///
  /// * `startup_data` - This should be startup data based on a snapshot created
  ///   by `create_compiler_snapshot()`.
  ///
  pub fn new(startup_data: StartupData) -> Self {
    let mut isolate = CoreIsolate::new(startup_data, false);

    let state = Arc::new(Mutex::new(CompilerState::default()));

    register_ops(&mut isolate, &state);

    CompilerIsolate { isolate, state }
  }

  /// Given a module graph, perform a type check and return the emitted files
  /// plus other data.  This method also supports incremental compilation.
  ///
  /// # Arguments
  ///
  /// * `options` - A structure of compile options.
  ///
  /// # Errors
  ///
  /// If there are TypeScript diagnostics returned from the isolate when
  /// emitting the program, this method will error with those diagnostics.
  ///
  pub fn compile(
    &mut self,
    options: InternalCompileOptions,
  ) -> Result<CompilerEmit> {
    let mut compiler_options = json!({
      "allowJs": true,
      "esModuleInterop": true,
      "inlineSourceMap": true,
      "jsx": "react",
      "lib": options.compile_options.lib,
      "module": "esnext",
      "outDir": "cache:///",
      "removeComments": true,
      "strict": true,
      "target": "esnext",
    });
    if options.compile_options.incremental {
      let incremental_options = json!({
        "incremental": true,
        "tsBuildInfoFile": "cache:///.tsbuildinfo",
      });
      json_merge(&mut compiler_options, &incremental_options);
    }
    if options.bundle {
      let bundle_options = json!({
        "inlineSourceMap": false,
        "module": "system",
        "sourceMap": true,
      });
      json_merge(&mut compiler_options, &bundle_options);
    }
    if options.check_only {
      let check_only_options = json!({
        "noEmit": true,
      });
      json_merge(&mut compiler_options, &check_only_options);
    }

    let maybe_ignored_options =
      if let Some(config_text) = options.compile_options.maybe_config {
        let (user_config, ignored_options) = parse_config(config_text)?;
        json_merge(&mut compiler_options, &user_config);
        ignored_options
      } else {
        None
      };

    let mut cache_js = false;
    if let Some(value) = compiler_options.as_object().unwrap().get("checkJs") {
      cache_js = value.as_bool().unwrap_or(false);
    }

    let mut state = self.state.lock().expect("clould not lock state");
    state.maybe_provider = Some(options.provider);
    let maybe_shared_path = options.maybe_shared_path.clone();
    state.maybe_build_info = options.maybe_build_info;
    state.maybe_shared_path = options.maybe_shared_path.clone();
    drop(state);

    let root_names: Vec<String> = options
      .root_names
      .iter()
      .map(|s| as_ts_filename(s, &maybe_shared_path))
      .collect();
    let request = json!({
      "compilerOptions": compiler_options,
      "debug": options.compile_options.debug,
      "rootNames": root_names,
    });

    let js_source = format!("compile({})", request);
    js_check(self.isolate.execute("<anon>", &js_source));

    let state = self.state.lock().unwrap();
    let emit_result = state.emit_result.clone().unwrap();
    if !emit_result.diagnostics.0.is_empty() {
      Err(emit_result.diagnostics.into())
    } else {
      Ok(CompilerEmit {
        cache_js,
        stats: emit_result.stats,
        maybe_build_info: state.maybe_build_info.clone(),
        maybe_ignored_options,
        written_files: state.written_files.clone(),
      })
    }
  }

  /// Given a module graph, return transpiled sources, without performing any
  /// type checking.
  ///
  /// # Arguments
  ///
  /// * `options` - A structure of transpile options.
  ///
  /// # Errors
  ///
  /// If there are TypeScript diagnostics returned from the isolate when
  /// emitting the program, this method will error with those diagnostics.
  ///
  pub fn transpile(
    &mut self,
    options: InternalTranspileOptions,
  ) -> Result<CompilerEmit> {
    let mut compiler_options = json!({
      "esModuleInterop": true,
      "inlineSourceMap": true,
      "jsx": "react",
      "module": "esnext",
      "removeComments": true,
      "target": "esnext",
    });

    if options.bundle {
      let bundle_options = json!({
        "inlineSourceMap": false,
        "module": "system",
        "sourceMap": true,
      });
      json_merge(&mut compiler_options, &bundle_options);
    }

    let maybe_ignored_options =
      if let Some(config_text) = options.transpile_options.maybe_config {
        let (user_config, ignored_options) = parse_config(config_text)?;
        json_merge(&mut compiler_options, &user_config);
        ignored_options
      } else {
        None
      };

    let request = json!({
      "compilerOptions": compiler_options,
      "debug": options.transpile_options.debug,
      "sources": options.sources,
    });

    let js_source = format!("transpile({})", request);
    js_check(self.isolate.execute("<anon>", &js_source));

    let state = self.state.lock().unwrap();
    let emit_result = state.emit_result.clone().unwrap();
    if !emit_result.diagnostics.0.is_empty() {
      Err(emit_result.diagnostics.into())
    } else {
      Ok(CompilerEmit {
        cache_js: false,
        stats: emit_result.stats,
        maybe_build_info: None,
        maybe_ignored_options,
        written_files: state.written_files.clone(),
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::module_graph::GraphBuilder;
  use crate::tests::MockSpecifierHandler;
  use deno_core::ModuleSpecifier;
  use deno_core::Snapshot;
  use std::cell::RefCell;
  use std::env;
  use std::rc::Rc;
  use tempfile::TempDir;

  fn get_compiler(rebuild: bool) -> Result<CompilerIsolate> {
    let o = env::temp_dir();
    let snapshot_path = o.join("TEST_COMPILER.bin");
    if rebuild || !snapshot_path.is_file() {
      let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
      let assets_path = c.join("../cli/dts");
      assert!(assets_path.is_dir());
      let test_fixtures_path = c.join("fixtures");
      assert!(test_fixtures_path.is_dir());
      let mut custom_libs = HashMap::new();
      custom_libs
        .insert("test".to_string(), test_fixtures_path.join("lib.test.d.ts"));
      create_compiler_snapshot(
        snapshot_path.clone(),
        assets_path,
        custom_libs,
      )?;
    }
    let snapshot_data = std::fs::read(snapshot_path)?.into_boxed_slice();
    let startup_data = StartupData::Snapshot(Snapshot::Boxed(snapshot_data));

    Ok(CompilerIsolate::new(startup_data))
  }

  #[test]
  fn test_create_compiler_snapshot() {
    let temp_dir = TempDir::new().unwrap();
    let o = temp_dir.path();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let snapshot_path = o.join("TEST.bin");
    let assets_path = c.join("../cli/dts");
    let test_fixtures_path = c.join("fixtures");
    let mut custom_libs = HashMap::new();
    custom_libs
      .insert("test".to_string(), test_fixtures_path.join("lib.test.d.ts"));
    let version =
      create_compiler_snapshot(snapshot_path.clone(), assets_path, custom_libs)
        .unwrap();
    assert!(snapshot_path.is_file());
    assert_eq!(version, "4.0.2".to_string());
  }

  #[tokio::test]
  async fn test_compile_bundle() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts")
        .expect("could not resolve");
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let lib = vec!["test"];

    let mut compiler = get_compiler(false).expect("failed to get compiler");
    let compile_result = compiler
      .compile(InternalCompileOptions {
        provider: Rc::new(RefCell::new(graph)),
        root_names: vec![&specifier],
        maybe_build_info: None,
        maybe_shared_path: None,
        bundle: true,
        check_only: false,
        compile_options: CompileOptions {
          lib,
          ..CompileOptions::default()
        },
      })
      .unwrap();
    assert!(compile_result.maybe_ignored_options.is_none());
    assert_eq!(compile_result.written_files.len(), 4);
  }

  #[tokio::test]
  async fn test_compile_incremental() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts")
        .expect("could not resolve");
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let lib = vec!["test"];

    let mut compiler = get_compiler(false).expect("failed to get compiler");
    let compiler_result = compiler
      .compile(InternalCompileOptions {
        provider: Rc::new(RefCell::new(graph)),
        bundle: false,
        check_only: false,
        root_names: vec![&specifier],
        maybe_build_info: None,
        maybe_shared_path: None,
        compile_options: CompileOptions {
          lib,
          incremental: true,
          ..CompileOptions::default()
        },
      })
      .expect("failed to compile");
    assert_eq!(
      compiler_result.written_files.len(),
      2,
      "should have written to files"
    );
    assert!(compiler_result.maybe_build_info.is_some());
    let maybe_build_info = compiler_result.maybe_build_info;

    let mut builder = GraphBuilder::new(handler.clone(), None);
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let lib = vec!["test"];

    let mut compiler = get_compiler(false).expect("failed to get compiler");
    let compiler_result = compiler
      .compile(InternalCompileOptions {
        provider: Rc::new(RefCell::new(graph)),
        bundle: false,
        check_only: false,
        root_names: vec![&specifier],
        maybe_build_info: maybe_build_info.clone(),
        maybe_shared_path: None,
        compile_options: CompileOptions {
          lib,
          incremental: true,
          ..CompileOptions::default()
        },
      })
      .expect("failed to compile");
    assert_eq!(
      compiler_result.written_files.len(),
      0,
      "should not have written any files"
    );
    assert_eq!(compiler_result.maybe_build_info, maybe_build_info);
  }
}
