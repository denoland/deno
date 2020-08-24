// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

extern crate base64;
extern crate bytecount;
extern crate deno_core;
extern crate either;
extern crate futures;
extern crate jsonc_parser;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate ring;
extern crate serde;
extern crate serde_json;
extern crate sourcemap;
extern crate swc_common;
extern crate swc_ecma_transforms;
extern crate swc_ecmascript;
extern crate termcolor;
extern crate url;

mod ast;
mod bundler;
pub mod colors;
mod compiler;
mod config;
mod import_map;
mod module_graph;
mod msg;
mod ops;
mod source_map_bundler;

use deno_core::ErrBox;
use std::result;

pub use crate::bundler::bundle;
pub use crate::compiler::create_compiler_snapshot;
pub use crate::compiler::CompilerIsolate;
pub use crate::compiler::Sources;
pub use crate::import_map::ImportMap;
pub use crate::import_map::ImportMapError;
pub use crate::module_graph::CacheType;
pub use crate::module_graph::CachedDependencies;
pub use crate::module_graph::CachedDependency;
pub use crate::module_graph::CachedModule;
pub use crate::module_graph::DependencyType;
pub use crate::module_graph::FetchFuture;
pub use crate::module_graph::Graph;
pub use crate::module_graph::GraphBuilder;
pub use crate::module_graph::GraphError;
pub use crate::module_graph::SpecifierHandler;
pub use crate::msg::IgnoredCompilerOptions;
pub use crate::msg::MediaType;

type Result<V> = result::Result<V, ErrBox>;

pub struct BundleOptions<'a> {
  /// Indicates if the source code should be typed checked or simply transpiled.
  /// Defaults to `true`.
  pub check: bool,
  /// Informs the TypeScript compiler to output debug logging.
  pub debug: bool,
  /// Utilizes build information associated with the graph to speed up type
  /// checking.
  pub incremental: bool,
  /// Indicates if the source map should be inlined into the emitted bundle
  /// code.  This defaults to `true`.
  pub inline_source_map: bool,
  /// A vector of libs to be used when type checking the code.  For example:
  ///
  /// ```rust
  /// let lib = vec!["deno.ns"];
  /// ```
  ///
  pub lib: Vec<&'a str>,
  /// A string of configuration data that augments the the default configuration
  /// passed to the TypeScript compiler.  This is typically the contents of a
  /// user supplied `tsconfig.json`.
  pub maybe_config: Option<String>,
}

impl<'a> Default for BundleOptions<'a> {
  fn default() -> Self {
    BundleOptions {
      check: true,
      debug: false,
      incremental: false,
      inline_source_map: true,
      lib: Vec::new(),
      maybe_config: None,
    }
  }
}

/// A structure which provides options when compiling modules.
#[derive(Default)]
pub struct CompileOptions<'a> {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// A flag to indicate that the compilation should be incremental, and only
  /// changed sources will be emitted based on information supplied in the
  /// `maybe_build_info` argument.
  pub incremental: bool,
  /// A vector of libs to be used when type checking the code.  For example:
  ///
  /// ```rust
  /// let lib = vec!["deno.ns"];
  /// ```
  ///
  pub lib: Vec<&'a str>,
  /// A string of configuration data that augments the the default configuration
  /// passed to the TypeScript compiler.  This is typically the contents of a
  /// user supplied `tsconfig.json`.
  pub maybe_config: Option<String>,
}

/// A structure which provides options when transpiling modules.
#[derive(Default)]
pub struct TranspileOptions {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// A string of configuration data that augments the the default configuration
  /// passed to the TypeScript compiler.  This is typically the contents of a
  /// user supplied `tsconfig.json`.
  pub maybe_config: Option<String>,
}

#[cfg(test)]
pub mod tests {
  use super::*;

  use deno_core::ModuleSpecifier;
  use futures::future;
  use std::cell::RefCell;
  use std::collections::HashMap;
  use std::env;
  use std::fs;
  use std::path::PathBuf;
  use std::rc::Rc;

  /// A mock specifier handler which can be used to stub off the handler for a
  /// graph.
  #[derive(Debug, Default)]
  pub struct MockSpecifierHandler {
    pub fixtures: PathBuf,
    pub build_info: HashMap<ModuleSpecifier, String>,
    pub build_info_calls: Vec<(ModuleSpecifier, CacheType, String)>,
    pub cache_calls: Vec<(ModuleSpecifier, CacheType, String, Option<String>)>,
    pub deps_calls: Vec<(ModuleSpecifier, CachedDependencies)>,
    pub types_calls: Vec<(ModuleSpecifier, String)>,
  }

  impl MockSpecifierHandler {}

  impl SpecifierHandler for MockSpecifierHandler {
    fn fetch(&mut self, specifier: &ModuleSpecifier) -> FetchFuture {
      Box::pin(future::ready(Ok(specifier.clone())))
    }
    fn get_cache(&self, specifier: &ModuleSpecifier) -> Result<CachedModule> {
      let specifier_text = specifier
        .to_string()
        .replace(":///", "_")
        .replace("://", "_")
        .replace("/", "-");
      let specifier_path = self.fixtures.join(specifier_text);
      let media_type =
        match specifier_path.extension().unwrap().to_str().unwrap() {
          "ts" => MediaType::TypeScript,
          "tsx" => MediaType::TSX,
          "js" => MediaType::JavaScript,
          "jsx" => MediaType::JSX,
          _ => MediaType::Unknown,
        };
      let source = fs::read_to_string(specifier_path)?;

      Ok(CachedModule {
        source,
        media_type,
        ..CachedModule::default()
      })
    }
    fn get_build_info(
      &self,
      specifier: &ModuleSpecifier,
      _cache_type: &CacheType,
    ) -> Result<Option<String>> {
      Ok(self.build_info.get(specifier).cloned())
    }
    fn set_cache(
      &mut self,
      specifier: &ModuleSpecifier,
      cache_type: &CacheType,
      code: String,
      maybe_map: Option<String>,
    ) -> Result<()> {
      self.cache_calls.push((
        specifier.clone(),
        cache_type.clone(),
        code,
        maybe_map,
      ));
      Ok(())
    }
    fn set_types(
      &mut self,
      specifier: &ModuleSpecifier,
      types: String,
    ) -> Result<()> {
      self.types_calls.push((specifier.clone(), types));
      Ok(())
    }
    fn set_build_info(
      &mut self,
      specifier: &ModuleSpecifier,
      cache_type: &CacheType,
      build_info: String,
    ) -> Result<()> {
      self
        .build_info
        .insert(specifier.clone(), build_info.clone());
      self.build_info_calls.push((
        specifier.clone(),
        cache_type.clone(),
        build_info,
      ));
      Ok(())
    }
    fn set_deps(
      &mut self,
      specifier: &ModuleSpecifier,
      dependencies: CachedDependencies,
    ) -> Result<()> {
      self.deps_calls.push((specifier.clone(), dependencies));
      Ok(())
    }
  }

  fn get_snapshot_data(rebuild: bool) -> Result<Box<[u8]>> {
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

    Ok(std::fs::read(snapshot_path)?.into_boxed_slice())
  }

  fn get_handler() -> Rc<RefCell<MockSpecifierHandler>> {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");

    Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }))
  }

  type Handler = Rc<RefCell<MockSpecifierHandler>>;
  type SetupResult = Result<(GraphBuilder, Handler, Box<[u8]>)>;

  fn setup(rebuild: bool, maybe_import_map: Option<ImportMap>) -> SetupResult {
    let handler = get_handler();

    Ok((
      GraphBuilder::new(handler.clone(), maybe_import_map),
      handler,
      get_snapshot_data(rebuild)?,
    ))
  }

  #[tokio::test]
  async fn test_graph_compile() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (_, maybe_ignored_options) = graph
      .compile(
        CompileOptions {
          lib: vec!["test"],
          ..CompileOptions::default()
        },
        false,
        Some(snapshot_data),
      )
      .unwrap();
    assert!(maybe_ignored_options.is_none());
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2, "should have 2 calls");
    assert_eq!(
      h.cache_calls[0].1,
      CacheType::Cli,
      "cache type should be set to Cli"
    );
    assert_eq!(
      h.cache_calls[1].1,
      CacheType::Cli,
      "cache type should be set to Cli"
    );
    assert!(h.cache_calls[0].3.is_none(), "shouldn't have maybe map");
    assert!(h.cache_calls[1].3.is_none(), "shouldn't have maybe map");
  }

  #[tokio::test]
  async fn test_graph_compile_user_config() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/decorators.ts")
        .unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let maybe_config = Some(
      r#"{
      "compilerOptions": {
        "experimentalDecorators": true,
        "outFile": "something.js"
      }
    }"#
        .to_string(),
    );
    let (_, maybe_ignored_options) = graph
      .compile(
        CompileOptions {
          lib: vec!["test"],
          maybe_config,
          ..CompileOptions::default()
        },
        false,
        Some(snapshot_data),
      )
      .unwrap();
    assert_eq!(
      maybe_ignored_options,
      Some(IgnoredCompilerOptions(vec!["outFile".to_string()])),
      "should have ignored outFile"
    );
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 1, "should have 1 calls");
    let code = h.cache_calls[0].2.clone();
    assert!(code.contains("__decorate = "));
  }

  #[tokio::test]
  async fn test_graph_compile_mixed_1() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("/tests/mixed/main_js.js").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    graph
      .compile(
        CompileOptions {
          lib: vec!["test"],
          ..CompileOptions::default()
        },
        false,
        Some(snapshot_data),
      )
      .unwrap();
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 1, "only emit TS file");
    assert_eq!(
      h.cache_calls[0].0,
      ModuleSpecifier::resolve_url_or_path("file:///tests/mixed/a.ts").unwrap()
    );
  }

  #[tokio::test]
  async fn test_graph_compile_mixed_2() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("/tests/mixed/main_ts.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    graph
      .compile(
        CompileOptions {
          lib: vec!["test"],
          ..CompileOptions::default()
        },
        false,
        Some(snapshot_data),
      )
      .unwrap();
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2, "only emit TS file");
    assert_eq!(
      h.cache_calls[0].0,
      ModuleSpecifier::resolve_url_or_path("file:///tests/mixed/main_ts.ts")
        .unwrap()
    );
    assert_eq!(
      h.cache_calls[1].0,
      ModuleSpecifier::resolve_url_or_path("file:///tests/mixed/d.ts").unwrap()
    );
  }

  #[tokio::test]
  async fn test_graph_compile_check_only() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (_, maybe_ignored_options) = graph
      .compile(
        CompileOptions {
          lib: vec!["test"],
          ..CompileOptions::default()
        },
        true,
        Some(snapshot_data),
      )
      .unwrap();
    assert!(maybe_ignored_options.is_none());
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 0, "should have 2 calls");
  }

  #[tokio::test]
  async fn test_graph_transpile() {
    let (mut builder, handler, _) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (_, maybe_ignored_options) =
      graph.transpile(TranspileOptions::default(), None).unwrap();
    assert!(maybe_ignored_options.is_none());
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2, "should have 2 calls");
    assert_eq!(
      h.cache_calls[0].1,
      CacheType::Cli,
      "cache type should be set to Cli"
    );
    assert_eq!(
      h.cache_calls[1].1,
      CacheType::Cli,
      "cache type should be set to Cli"
    );
    assert!(h.cache_calls[0].3.is_none(), "shouldn't have maybe map");
    assert!(h.cache_calls[1].3.is_none(), "shouldn't have maybe map");
  }

  #[tokio::test]
  async fn test_graph_transpile_user_config() {
    let (mut builder, handler, _) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/preact.tsx").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let maybe_config = Some(
      r#"{
        "compilerOptions": {
          "jsxFactory": "Preact.h",
          "jsxFragmentFactory": "Preact.Fragment",
          "outFile": "something.js"
        }
      }"#
        .to_string(),
    );
    let (_, maybe_ignored_options) = graph
      .transpile(
        TranspileOptions {
          maybe_config,
          ..Default::default()
        },
        None,
      )
      .unwrap();
    assert_eq!(
      maybe_ignored_options,
      Some(IgnoredCompilerOptions(vec!["outFile".to_string()])),
      "should have ignored outFile"
    );
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 1, "should have 1 calls");
    let code = h.cache_calls[0].2.clone();
    assert!(code.contains("Preact.Fragment"));
  }

  #[tokio::test]
  async fn test_graph_bundle() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (code, maybe_map, _, maybe_ignored_options) = graph
      .bundle(
        BundleOptions {
          lib: vec!["test"],
          ..Default::default()
        },
        Some(snapshot_data),
      )
      .unwrap();
    assert!(maybe_ignored_options.is_none());
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2, "should have 2 calls");
    assert_eq!(
      h.cache_calls[0].1,
      CacheType::Bundle,
      "cache type should be set to Cli"
    );
    assert_eq!(
      h.cache_calls[1].1,
      CacheType::Bundle,
      "cache type should be set to Cli"
    );
    assert!(h.cache_calls[0].3.is_some(), "should have had a map");
    assert!(h.cache_calls[1].3.is_some(), "should have had a map");
    assert!(code.contains(
      "\n\nvar __exp = __instantiate(\"/https/deno.land/x/a.ts\", false);\n"
    ));
    assert!(maybe_map.is_none());
  }

  #[tokio::test]
  async fn test_graph_bundle_complex_exports() {
    let (mut builder, _, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/bundle/root.ts")
        .unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (code, maybe_map, _, _) = graph
      .bundle(
        BundleOptions {
          lib: vec!["test"],
          ..Default::default()
        },
        Some(snapshot_data),
      )
      .unwrap();
    assert!(code
      .contains("\n\nvar __exp = __instantiate(\"/file/root.ts\", false);\n"));
    assert!(code.contains("\nexport var b = __exp[\"b\"];\n"));
    assert!(code.contains("\nexport var C = __exp[\"C\"];\n"));
    assert!(code.contains("\nexport var a = __exp[\"a\"];\n"));
    assert!(maybe_map.is_none());
  }

  #[tokio::test]
  async fn test_graph_bundle_no_check() {
    let (mut builder, handler, snapshot_data) =
      setup(false, None).expect("could not setup");

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    builder
      .insert(&specifier)
      .await
      .expect("could not insert module");
    let graph = builder.get_graph();
    let (code, maybe_map, _, maybe_ignored_options) = graph
      .bundle(
        BundleOptions {
          check: false,
          lib: vec!["test"],
          ..Default::default()
        },
        Some(snapshot_data),
      )
      .unwrap();
    assert!(maybe_ignored_options.is_none());
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2, "should have 2 calls");
    assert_eq!(
      h.cache_calls[0].1,
      CacheType::Bundle,
      "cache type should be set to Cli"
    );
    assert_eq!(
      h.cache_calls[1].1,
      CacheType::Bundle,
      "cache type should be set to Cli"
    );
    assert!(h.cache_calls[0].3.is_some(), "should have had a map");
    assert!(h.cache_calls[1].3.is_some(), "should have had a map");
    assert!(code.contains("System.register(\"/https/deno.land/x/a.ts\", [\"/https/deno.land/x/b.ts\"]"));
    assert!(code.contains(
      "\n\nvar __exp = __instantiate(\"/https/deno.land/x/a.ts\", false);\n"
    ));
    assert!(maybe_map.is_none());
  }
}
