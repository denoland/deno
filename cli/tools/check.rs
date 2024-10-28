// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_runtime::deno_node::NodeResolver;
use deno_terminal::colors;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::args::check_warn_tsconfig;
use crate::args::CheckFlags;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TsConfig;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::FastInsecureHasher;
use crate::cache::TypeCheckCache;
use crate::factory::CliFactory;
use crate::graph_util::BuildFastCheckGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::npm::CliNpmResolver;
use crate::tsc;
use crate::tsc::Diagnostics;
use crate::util::extract;
use crate::util::path::to_percent_decoded_str;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);

  let main_graph_container = factory.main_module_graph_container().await?;

  let specifiers =
    main_graph_container.collect_specifiers(&check_flags.files)?;
  if specifiers.is_empty() {
    log::warn!("{} No matching files found.", colors::yellow("Warning"));
  }

  let specifiers_for_typecheck = if check_flags.doc || check_flags.doc_only {
    let file_fetcher = factory.file_fetcher()?;
    let root_permissions = factory.root_permissions_container()?;

    let mut specifiers_for_typecheck = if check_flags.doc {
      specifiers.clone()
    } else {
      vec![]
    };

    for s in specifiers {
      let file = file_fetcher.fetch(&s, root_permissions).await?;
      let snippet_files = extract::extract_snippet_files(file)?;
      for snippet_file in snippet_files {
        specifiers_for_typecheck.push(snippet_file.specifier.clone());
        file_fetcher.insert_memory_files(snippet_file);
      }
    }

    specifiers_for_typecheck
  } else {
    specifiers
  };

  main_graph_container
    .check_specifiers(&specifiers_for_typecheck, None)
    .await
}

/// Options for performing a check of a module graph. Note that the decision to
/// emit or not is determined by the `ts_config` settings.
pub struct CheckOptions {
  /// Whether to build the fast check type graph if necessary.
  ///
  /// Note: For perf reasons, the fast check type graph is only
  /// built if type checking is necessary.
  pub build_fast_check_graph: bool,
  /// Default type library to type check with.
  pub lib: TsTypeLib,
  /// Whether to log about any ignored compiler options.
  pub log_ignored_options: bool,
  /// If true, valid `.tsbuildinfo` files will be ignored and type checking
  /// will always occur.
  pub reload: bool,
  /// Mode to type check with.
  pub type_check_mode: TypeCheckMode,
}

pub struct TypeChecker {
  caches: Arc<Caches>,
  cli_options: Arc<CliOptions>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  node_resolver: Arc<NodeResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
}

impl TypeChecker {
  pub fn new(
    caches: Arc<Caches>,
    cli_options: Arc<CliOptions>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    node_resolver: Arc<NodeResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
  ) -> Self {
    Self {
      caches,
      cli_options,
      module_graph_builder,
      node_resolver,
      npm_resolver,
    }
  }

  /// Type check the module graph.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  pub async fn check(
    &self,
    graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<Arc<ModuleGraph>, AnyError> {
    let (graph, mut diagnostics) =
      self.check_diagnostics(graph, options).await?;
    diagnostics.emit_warnings();
    if diagnostics.is_empty() {
      Ok(graph)
    } else {
      Err(diagnostics.into())
    }
  }

  /// Type check the module graph returning its diagnostics.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  pub async fn check_diagnostics(
    &self,
    mut graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<(Arc<ModuleGraph>, Diagnostics), AnyError> {
    if !options.type_check_mode.is_true() || graph.roots.is_empty() {
      return Ok((graph.into(), Default::default()));
    }

    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now (the caller should do this after the lockfile
    // has been written)
    if let Some(npm_resolver) = self.npm_resolver.as_managed() {
      if graph.has_node_specifier {
        npm_resolver.inject_synthetic_types_node_package().await?;
      }
    }

    log::debug!("Type checking.");
    let ts_config_result = self
      .cli_options
      .resolve_ts_config_for_emit(TsConfigType::Check { lib: options.lib })?;
    if options.log_ignored_options {
      check_warn_tsconfig(&ts_config_result);
    }

    let type_check_mode = options.type_check_mode;
    let ts_config = ts_config_result.ts_config;
    let maybe_check_hash = match self.npm_resolver.check_state_hash() {
      Some(npm_check_hash) => {
        match get_check_hash(
          &graph,
          npm_check_hash,
          type_check_mode,
          &ts_config,
        ) {
          CheckHashResult::NoFiles => {
            return Ok((graph.into(), Default::default()))
          }
          CheckHashResult::Hash(hash) => Some(hash),
        }
      }
      None => None, // we can't determine a check hash
    };

    // do not type check if we know this is type checked
    let cache = TypeCheckCache::new(self.caches.type_checking_cache_db());
    if !options.reload {
      if let Some(check_hash) = maybe_check_hash {
        if cache.has_check_hash(check_hash) {
          log::debug!("Already type checked.");
          return Ok((graph.into(), Default::default()));
        }
      }
    }

    for root in &graph.roots {
      let root_str = root.as_str();
      log::info!(
        "{} {}",
        colors::green("Check"),
        to_percent_decoded_str(root_str)
      );
    }

    let check_js = ts_config.get_check_js();
    // while there might be multiple roots, we can't "merge" the build info, so we
    // try to retrieve the build info for first root, which is the most common use
    // case.
    let maybe_tsbuildinfo = if options.reload {
      None
    } else {
      cache.get_tsbuildinfo(&graph.roots[0])
    };
    // to make tsc build info work, we need to consistently hash modules, so that
    // tsc can better determine if an emit is still valid or not, so we provide
    // that data here.
    let hash_data = FastInsecureHasher::new_deno_versioned()
      .write(&ts_config.as_bytes())
      .finish();

    // add fast check to the graph before getting the roots
    if options.build_fast_check_graph {
      self.module_graph_builder.build_fast_check_graph(
        &mut graph,
        BuildFastCheckGraphOptions {
          workspace_fast_check: deno_graph::WorkspaceFastCheckOption::Disabled,
        },
      )?;
    }

    let root_names = get_tsc_roots(&graph, check_js);
    let graph = Arc::new(graph);
    let response = tsc::exec(tsc::Request {
      config: ts_config,
      debug: self.cli_options.log_level() == Some(log::Level::Debug),
      graph: graph.clone(),
      hash_data,
      maybe_npm: Some(tsc::RequestNpmState {
        node_resolver: self.node_resolver.clone(),
        npm_resolver: self.npm_resolver.clone(),
      }),
      maybe_tsbuildinfo,
      root_names,
      check_mode: type_check_mode,
    })?;

    let mut diagnostics = response.diagnostics.filter(|d| {
      if self.is_remote_diagnostic(d) {
        type_check_mode == TypeCheckMode::All && d.include_when_remote()
      } else {
        true
      }
    });

    diagnostics.apply_fast_check_source_maps(&graph);

    if let Some(tsbuildinfo) = response.maybe_tsbuildinfo {
      cache.set_tsbuildinfo(&graph.roots[0], &tsbuildinfo);
    }

    if diagnostics.is_empty() {
      if let Some(check_hash) = maybe_check_hash {
        cache.add_check_hash(check_hash);
      }
    }

    log::debug!("{}", response.stats);

    Ok((graph, diagnostics))
  }

  fn is_remote_diagnostic(&self, d: &tsc::Diagnostic) -> bool {
    let Some(file_name) = &d.file_name else {
      return false;
    };
    if file_name.starts_with("https://") || file_name.starts_with("http://") {
      return true;
    }
    // check if in an npm package
    let Ok(specifier) = ModuleSpecifier::parse(file_name) else {
      return false;
    };
    self.node_resolver.in_npm_package(&specifier)
  }
}

enum CheckHashResult {
  Hash(CacheDBHash),
  NoFiles,
}

/// Gets a hash of the inputs for type checking. This can then
/// be used to tell
fn get_check_hash(
  graph: &ModuleGraph,
  package_reqs_hash: u64,
  type_check_mode: TypeCheckMode,
  ts_config: &TsConfig,
) -> CheckHashResult {
  let mut hasher = FastInsecureHasher::new_deno_versioned();
  hasher.write_u8(match type_check_mode {
    TypeCheckMode::All => 0,
    TypeCheckMode::Local => 1,
    TypeCheckMode::None => 2,
  });
  hasher.write(&ts_config.as_bytes());

  let check_js = ts_config.get_check_js();
  let mut has_file = false;
  let mut has_file_to_type_check = false;
  // this iterator of modules is already deterministic, so no need to sort it
  for module in graph.modules() {
    match module {
      Module::Js(module) => {
        let ts_check = has_ts_check(module.media_type, &module.source);
        if ts_check {
          has_file_to_type_check = true;
        }

        match module.media_type {
          MediaType::TypeScript
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Tsx => {
            has_file = true;
            has_file_to_type_check = true;
          }
          MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Jsx => {
            has_file = true;
            if !check_js && !ts_check {
              continue;
            }
          }
          MediaType::Json
          | MediaType::TsBuildInfo
          | MediaType::SourceMap
          | MediaType::Wasm
          | MediaType::Unknown => continue,
        }

        hasher.write_str(module.specifier.as_str());
        hasher.write_str(
          // the fast check module will only be set when publishing
          module
            .fast_check_module()
            .map(|s| s.source.as_ref())
            .unwrap_or(&module.source),
        );
      }
      Module::Node(_) => {
        // the @types/node package will be in the resolved
        // snapshot below so don't bother including it here
      }
      Module::Npm(_) => {
        // don't bother adding this specifier to the hash
        // because what matters is the resolved npm snapshot,
        // which is hashed below
      }
      Module::Json(module) => {
        has_file_to_type_check = true;
        hasher.write_str(module.specifier.as_str());
        hasher.write_str(&module.source);
      }
      Module::External(module) => {
        hasher.write_str(module.specifier.as_str());
      }
    }
  }

  hasher.write_hashable(package_reqs_hash);

  if !has_file || !check_js && !has_file_to_type_check {
    // no files to type check
    CheckHashResult::NoFiles
  } else {
    CheckHashResult::Hash(CacheDBHash::new(hasher.finish()))
  }
}

/// Transform the graph into root specifiers that we can feed `tsc`. We have to
/// provide the media type for root modules because `tsc` does not "resolve" the
/// media type like other modules, as well as a root specifier needs any
/// redirects resolved. We need to include all the emittable files in
/// the roots, so they get type checked and optionally emitted,
/// otherwise they would be ignored if only imported into JavaScript.
fn get_tsc_roots(
  graph: &ModuleGraph,
  check_js: bool,
) -> Vec<(ModuleSpecifier, MediaType)> {
  fn maybe_get_check_entry(
    module: &deno_graph::Module,
    check_js: bool,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    match module {
      Module::Js(module) => match module.media_type {
        MediaType::TypeScript
        | MediaType::Tsx
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts => {
          Some((module.specifier.clone(), module.media_type))
        }
        MediaType::JavaScript
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::Jsx => {
          if check_js || has_ts_check(module.media_type, &module.source) {
            Some((module.specifier.clone(), module.media_type))
          } else {
            None
          }
        }
        MediaType::Json
        | MediaType::Wasm
        | MediaType::TsBuildInfo
        | MediaType::SourceMap
        | MediaType::Unknown => None,
      },
      Module::External(_)
      | Module::Node(_)
      | Module::Npm(_)
      | Module::Json(_) => None,
    }
  }

  let mut result = Vec::with_capacity(graph.specifiers_count());
  if graph.has_node_specifier {
    // inject a specifier that will resolve node types
    result.push((
      ModuleSpecifier::parse("asset:///node_types.d.ts").unwrap(),
      MediaType::Dts,
    ));
  }

  let mut seen =
    HashSet::with_capacity(graph.imports.len() + graph.specifiers_count());
  let mut pending = VecDeque::new();

  // put in the global types first so that they're resolved before anything else
  for import in graph.imports.values() {
    for dep in import.dependencies.values() {
      let specifier = dep.get_type().or_else(|| dep.get_code());
      if let Some(specifier) = &specifier {
        let specifier = graph.resolve(specifier);
        if seen.insert(specifier.clone()) {
          pending.push_back(specifier);
        }
      }
    }
  }

  // then the roots
  for root in &graph.roots {
    let specifier = graph.resolve(root);
    if seen.insert(specifier.clone()) {
      pending.push_back(specifier);
    }
  }

  // now walk the graph that only includes the fast check dependencies
  while let Some(specifier) = pending.pop_front() {
    let Some(module) = graph.get(specifier) else {
      continue;
    };
    if let Some(entry) = maybe_get_check_entry(module, check_js) {
      result.push(entry);
    }
    if let Some(module) = module.js() {
      let deps = module.dependencies_prefer_fast_check();
      for dep in deps.values() {
        // walk both the code and type dependencies
        if let Some(specifier) = dep.get_code() {
          let specifier = graph.resolve(specifier);
          if seen.insert(specifier.clone()) {
            pending.push_back(specifier);
          }
        }
        if let Some(specifier) = dep.get_type() {
          let specifier = graph.resolve(specifier);
          if seen.insert(specifier.clone()) {
            pending.push_back(specifier);
          }
        }
      }

      if let Some(dep) = module
        .maybe_types_dependency
        .as_ref()
        .and_then(|d| d.dependency.ok())
      {
        let specifier = graph.resolve(&dep.specifier);
        if seen.insert(specifier.clone()) {
          pending.push_back(specifier);
        }
      }
    }
  }

  result
}

/// Matches the `@ts-check` pragma.
static TS_CHECK_RE: Lazy<Regex> =
  lazy_regex::lazy_regex!(r#"(?i)^\s*@ts-check(?:\s+|$)"#);

fn has_ts_check(media_type: MediaType, file_text: &str) -> bool {
  match &media_type {
    MediaType::JavaScript
    | MediaType::Mjs
    | MediaType::Cjs
    | MediaType::Jsx => get_leading_comments(file_text)
      .iter()
      .any(|text| TS_CHECK_RE.is_match(text)),
    MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Dts
    | MediaType::Dcts
    | MediaType::Dmts
    | MediaType::Tsx
    | MediaType::Json
    | MediaType::Wasm
    | MediaType::TsBuildInfo
    | MediaType::SourceMap
    | MediaType::Unknown => false,
  }
}

fn get_leading_comments(file_text: &str) -> Vec<String> {
  let mut chars = file_text.chars().peekable();

  // skip over the shebang
  if file_text.starts_with("#!") {
    // skip until the end of the line
    for c in chars.by_ref() {
      if c == '\n' {
        break;
      }
    }
  }

  let mut results = Vec::new();
  // now handle the comments
  while chars.peek().is_some() {
    // skip over any whitespace
    while chars
      .peek()
      .map(|c| char::is_whitespace(*c))
      .unwrap_or(false)
    {
      chars.next();
    }

    if chars.next() != Some('/') {
      break;
    }
    match chars.next() {
      Some('/') => {
        let mut text = String::new();
        for c in chars.by_ref() {
          if c == '\n' {
            break;
          } else {
            text.push(c);
          }
        }
        results.push(text);
      }
      Some('*') => {
        let mut text = String::new();
        while let Some(c) = chars.next() {
          if c == '*' && chars.peek() == Some(&'/') {
            chars.next();
            break;
          } else {
            text.push(c);
          }
        }
        results.push(text);
      }
      _ => break,
    }
  }
  results
}

#[cfg(test)]
mod test {
  use deno_ast::MediaType;

  use super::get_leading_comments;
  use super::has_ts_check;

  #[test]
  fn get_leading_comments_test() {
    assert_eq!(
      get_leading_comments(
        "#!/usr/bin/env deno\r\n// test\n/* 1 *//*2*///3\n//\n /**/  /*4 */"
      ),
      vec![
        " test".to_string(),
        " 1 ".to_string(),
        "2".to_string(),
        "3".to_string(),
        "".to_string(),
        "".to_string(),
        "4 ".to_string(),
      ]
    );
    assert_eq!(
      get_leading_comments("//1 /* */ \na;"),
      vec!["1 /* */ ".to_string(),]
    );
    assert_eq!(get_leading_comments("//"), vec!["".to_string()]);
  }

  #[test]
  fn has_ts_check_test() {
    assert!(has_ts_check(
      MediaType::JavaScript,
      "// @ts-check\nconsole.log(5);"
    ));
    assert!(has_ts_check(
      MediaType::JavaScript,
      "// deno-lint-ignore\n// @ts-check\n"
    ));
    assert!(!has_ts_check(
      MediaType::JavaScript,
      "test;\n// @ts-check\n"
    ));
    assert!(!has_ts_check(
      MediaType::JavaScript,
      "// ts-check\nconsole.log(5);"
    ));
  }
}
