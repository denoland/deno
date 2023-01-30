// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::TypeCheckCache;
use crate::colors;
use crate::errors::get_error_class_name;
use crate::npm::resolve_graph_npm_info;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::proc_state::ProcState;
use crate::resolver::CliResolver;
use crate::tools::check;

use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::ModuleSpecifier;
use deno_graph::Dependency;
use deno_graph::GraphImport;
use deno_graph::MediaType;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::Range;
use deno_graph::ResolutionError;
use deno_graph::Resolved;
use deno_graph::SpecifierError;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMapError;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ModuleEntry {
  Module {
    code: Arc<str>,
    dependencies: BTreeMap<String, Dependency>,
    media_type: MediaType,
    /// A set of type libs that the module has passed a type check with this
    /// session. This would consist of window, worker or both.
    checked_libs: HashSet<TsTypeLib>,
    maybe_types: Option<Resolved>,
  },
  Error(ModuleGraphError),
  Redirect(ModuleSpecifier),
}

/// Composes data from potentially many `ModuleGraph`s.
#[derive(Debug, Default)]
pub struct GraphData {
  modules: HashMap<ModuleSpecifier, ModuleEntry>,
  /// Specifiers that are built-in or external.
  external_specifiers: HashSet<ModuleSpecifier>,
  npm_packages: Vec<NpmPackageReq>,
  has_node_builtin_specifier: bool,
  /// Map of first known referrer locations for each module. Used to enhance
  /// error messages.
  referrer_map: HashMap<ModuleSpecifier, Box<Range>>,
  graph_imports: Vec<GraphImport>,
  cjs_esm_translations: HashMap<ModuleSpecifier, String>,
}

impl GraphData {
  /// Store data from `graph` into `self`.
  pub fn add_graph(&mut self, graph: &ModuleGraph, reload: bool) {
    for graph_import in &graph.imports {
      for dep in graph_import.dependencies.values() {
        for resolved in [&dep.maybe_code, &dep.maybe_type] {
          if let Resolved::Ok {
            specifier, range, ..
          } = resolved
          {
            let entry = self.referrer_map.entry(specifier.clone());
            entry.or_insert_with(|| range.clone());
          }
        }
      }
      self.graph_imports.push(graph_import.clone())
    }

    let mut has_npm_specifier_in_graph = false;

    for (specifier, result) in graph.specifiers() {
      if !reload && self.modules.contains_key(specifier) {
        continue;
      }

      if !self.has_node_builtin_specifier && specifier.scheme() == "node" {
        self.has_node_builtin_specifier = true;
      }

      if let Some(found) = graph.redirects.get(specifier) {
        let module_entry = ModuleEntry::Redirect(found.clone());
        self.modules.insert(specifier.clone(), module_entry);
        continue;
      }

      match result {
        Ok((_, module_kind, media_type)) => {
          if module_kind == ModuleKind::External {
            if !has_npm_specifier_in_graph
              && NpmPackageReference::from_specifier(specifier).is_ok()
            {
              has_npm_specifier_in_graph = true;
            }
            self.external_specifiers.insert(specifier.clone());
            continue; // ignore npm and node specifiers
          }

          let module = graph.get(specifier).unwrap();
          let code = match &module.maybe_source {
            Some(source) => source.clone(),
            None => continue,
          };
          let maybe_types = module
            .maybe_types_dependency
            .as_ref()
            .map(|(_, r)| r.clone());
          if let Some(Resolved::Ok {
            specifier, range, ..
          }) = &maybe_types
          {
            let specifier = graph.redirects.get(specifier).unwrap_or(specifier);
            let entry = self.referrer_map.entry(specifier.clone());
            entry.or_insert_with(|| range.clone());
          }
          for dep in module.dependencies.values() {
            #[allow(clippy::manual_flatten)]
            for resolved in [&dep.maybe_code, &dep.maybe_type] {
              if let Resolved::Ok {
                specifier, range, ..
              } = resolved
              {
                let specifier =
                  graph.redirects.get(specifier).unwrap_or(specifier);
                let entry = self.referrer_map.entry(specifier.clone());
                entry.or_insert_with(|| range.clone());
              }
            }
          }
          let module_entry = ModuleEntry::Module {
            code,
            dependencies: module.dependencies.clone(),
            media_type,
            checked_libs: Default::default(),
            maybe_types,
          };
          self.modules.insert(specifier.clone(), module_entry);
        }
        Err(error) => {
          let module_entry = ModuleEntry::Error(error.clone());
          self.modules.insert(specifier.clone(), module_entry);
        }
      }
    }

    if has_npm_specifier_in_graph {
      self
        .npm_packages
        .extend(resolve_graph_npm_info(graph).package_reqs);
    }
  }

  pub fn entries(
    &self,
  ) -> impl Iterator<Item = (&ModuleSpecifier, &ModuleEntry)> {
    self.modules.iter()
  }

  /// Gets if the graph had a "node:" specifier.
  pub fn has_node_builtin_specifier(&self) -> bool {
    self.has_node_builtin_specifier
  }

  /// Gets the npm package requirements from all the encountered graphs
  /// in the order that they should be resolved.
  pub fn npm_package_reqs(&self) -> &Vec<NpmPackageReq> {
    &self.npm_packages
  }

  /// Walk dependencies from `roots` and return every encountered specifier.
  /// Return `None` if any modules are not known.
  pub fn walk<'a>(
    &'a self,
    roots: &[ModuleSpecifier],
    follow_dynamic: bool,
    follow_type_only: bool,
    check_js: bool,
  ) -> Option<HashMap<&'a ModuleSpecifier, &'a ModuleEntry>> {
    let mut result = HashMap::<&'a ModuleSpecifier, &'a ModuleEntry>::new();
    let mut seen = HashSet::<&ModuleSpecifier>::new();
    let mut visiting = VecDeque::<&ModuleSpecifier>::new();
    for root in roots {
      seen.insert(root);
      visiting.push_back(root);
    }
    for (_, dep) in self.graph_imports.iter().flat_map(|i| &i.dependencies) {
      let mut resolutions = vec![&dep.maybe_code];
      if follow_type_only {
        resolutions.push(&dep.maybe_type);
      }
      #[allow(clippy::manual_flatten)]
      for resolved in resolutions {
        if let Resolved::Ok { specifier, .. } = resolved {
          if !seen.contains(specifier) {
            seen.insert(specifier);
            visiting.push_front(specifier);
          }
        }
      }
    }
    while let Some(specifier) = visiting.pop_front() {
      let (specifier, entry) = match self.modules.get_key_value(specifier) {
        Some(pair) => pair,
        None => {
          if self.external_specifiers.contains(specifier) {
            continue;
          }
          return None;
        }
      };
      result.insert(specifier, entry);
      match entry {
        ModuleEntry::Module {
          dependencies,
          maybe_types,
          media_type,
          ..
        } => {
          let check_types = (check_js
            || !matches!(
              media_type,
              MediaType::JavaScript
                | MediaType::Mjs
                | MediaType::Cjs
                | MediaType::Jsx
            ))
            && follow_type_only;
          if check_types {
            if let Some(Resolved::Ok { specifier, .. }) = maybe_types {
              if !seen.contains(specifier) {
                seen.insert(specifier);
                visiting.push_front(specifier);
              }
            }
          }
          for (dep_specifier, dep) in dependencies.iter().rev() {
            // todo(dsherret): ideally there would be a way to skip external dependencies
            // in the graph here rather than specifically npm package references
            if NpmPackageReference::from_str(dep_specifier).is_ok() {
              continue;
            }

            if !dep.is_dynamic || follow_dynamic {
              let mut resolutions = vec![&dep.maybe_code];
              if check_types {
                resolutions.push(&dep.maybe_type);
              }
              #[allow(clippy::manual_flatten)]
              for resolved in resolutions {
                if let Resolved::Ok { specifier, .. } = resolved {
                  if !seen.contains(specifier) {
                    seen.insert(specifier);
                    visiting.push_front(specifier);
                  }
                }
              }
            }
          }
        }
        ModuleEntry::Error(_) => {}
        ModuleEntry::Redirect(specifier) => {
          if !seen.contains(specifier) {
            seen.insert(specifier);
            visiting.push_front(specifier);
          }
        }
      }
    }
    Some(result)
  }

  /// Clone part of `self`, containing only modules which are dependencies of
  /// `roots`. Returns `None` if any roots are not known.
  pub fn graph_segment(&self, roots: &[ModuleSpecifier]) -> Option<Self> {
    let mut modules = HashMap::new();
    let mut referrer_map = HashMap::new();
    let entries = match self.walk(roots, true, true, true) {
      Some(entries) => entries,
      None => return None,
    };
    for (specifier, module_entry) in entries {
      modules.insert(specifier.clone(), module_entry.clone());
      if let Some(referrer) = self.referrer_map.get(specifier) {
        referrer_map.insert(specifier.clone(), referrer.clone());
      }
    }
    Some(Self {
      modules,
      external_specifiers: self.external_specifiers.clone(),
      has_node_builtin_specifier: self.has_node_builtin_specifier,
      npm_packages: self.npm_packages.clone(),
      referrer_map,
      graph_imports: self.graph_imports.to_vec(),
      cjs_esm_translations: Default::default(),
    })
  }

  /// Check if `roots` and their deps are available. Returns `Some(Ok(()))` if
  /// so. Returns `Some(Err(_))` if there is a known module graph or resolution
  /// error statically reachable from `roots`. Returns `None` if any modules are
  /// not known.
  pub fn check(
    &self,
    roots: &[ModuleSpecifier],
    follow_type_only: bool,
    check_js: bool,
  ) -> Option<Result<(), AnyError>> {
    let entries = match self.walk(roots, false, follow_type_only, check_js) {
      Some(entries) => entries,
      None => return None,
    };
    for (specifier, module_entry) in entries {
      match module_entry {
        ModuleEntry::Module {
          dependencies,
          maybe_types,
          media_type,
          ..
        } => {
          let check_types = (check_js
            || !matches!(
              media_type,
              MediaType::JavaScript
                | MediaType::Mjs
                | MediaType::Cjs
                | MediaType::Jsx
            ))
            && follow_type_only;
          if check_types {
            if let Some(Resolved::Err(error)) = maybe_types {
              let range = error.range();
              return Some(handle_check_error(
                error.clone().into(),
                Some(range),
              ));
            }
          }
          for (_, dep) in dependencies.iter() {
            if !dep.is_dynamic {
              let mut resolutions = vec![&dep.maybe_code];
              if check_types {
                resolutions.push(&dep.maybe_type);
              }
              #[allow(clippy::manual_flatten)]
              for resolved in resolutions {
                if let Resolved::Err(error) = resolved {
                  let range = error.range();
                  return Some(handle_check_error(
                    error.clone().into(),
                    Some(range),
                  ));
                }
              }
            }
          }
        }
        ModuleEntry::Error(error) => {
          let maybe_range = if roots.contains(specifier) {
            None
          } else {
            self.referrer_map.get(specifier)
          };
          return Some(handle_check_error(
            error.clone().into(),
            maybe_range.map(|r| &**r),
          ));
        }
        _ => {}
      }
    }
    Some(Ok(()))
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub fn set_type_checked(
    &mut self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) {
    let specifiers: Vec<ModuleSpecifier> =
      match self.walk(roots, true, true, true) {
        Some(entries) => entries.into_keys().cloned().collect(),
        None => unreachable!("contains module not in graph data"),
      };
    for specifier in specifiers {
      if let ModuleEntry::Module { checked_libs, .. } =
        self.modules.get_mut(&specifier).unwrap()
      {
        checked_libs.insert(lib);
      }
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: &TsTypeLib,
  ) -> bool {
    roots.iter().all(|r| {
      let found = self.follow_redirect(r);
      match self.modules.get(&found) {
        Some(ModuleEntry::Module { checked_libs, .. }) => {
          checked_libs.contains(lib)
        }
        _ => false,
      }
    })
  }

  /// If `specifier` is known and a redirect, return the found specifier.
  /// Otherwise return `specifier`.
  pub fn follow_redirect(
    &self,
    specifier: &ModuleSpecifier,
  ) -> ModuleSpecifier {
    match self.modules.get(specifier) {
      Some(ModuleEntry::Redirect(s)) => s.clone(),
      _ => specifier.clone(),
    }
  }

  pub fn get<'a>(
    &'a self,
    specifier: &ModuleSpecifier,
  ) -> Option<&'a ModuleEntry> {
    self.modules.get(specifier)
  }

  /// Get the dependencies of a module or graph import.
  pub fn get_dependencies<'a>(
    &'a self,
    specifier: &ModuleSpecifier,
  ) -> Option<&'a BTreeMap<String, Dependency>> {
    let specifier = self.follow_redirect(specifier);
    if let Some(ModuleEntry::Module { dependencies, .. }) = self.get(&specifier)
    {
      return Some(dependencies);
    }
    if let Some(graph_import) =
      self.graph_imports.iter().find(|i| i.referrer == specifier)
    {
      return Some(&graph_import.dependencies);
    }
    None
  }

  pub fn get_cjs_esm_translation<'a>(
    &'a self,
    specifier: &ModuleSpecifier,
  ) -> Option<&'a String> {
    self.cjs_esm_translations.get(specifier)
  }
}

impl From<&ModuleGraph> for GraphData {
  fn from(graph: &ModuleGraph) -> Self {
    let mut graph_data = GraphData::default();
    graph_data.add_graph(graph, false);
    graph_data
  }
}

/// Like `graph.valid()`, but enhanced with referrer info.
pub fn graph_valid(
  graph: &ModuleGraph,
  follow_type_only: bool,
  check_js: bool,
) -> Result<(), AnyError> {
  GraphData::from(graph)
    .check(&graph.roots, follow_type_only, check_js)
    .unwrap()
}

/// Checks the lockfile against the graph and and exits on errors.
pub fn graph_lock_or_exit(graph: &ModuleGraph, lockfile: &mut Lockfile) {
  for module in graph.modules() {
    if let Some(source) = &module.maybe_source {
      if !lockfile.check_or_insert_remote(module.specifier.as_str(), source) {
        let err = format!(
          concat!(
            "The source code is invalid, as it does not match the expected hash in the lock file.\n",
            "  Specifier: {}\n",
            "  Lock file: {}",
          ),
          module.specifier,
          lockfile.filename.display(),
        );
        log::error!("{} {}", colors::red("error:"), err);
        std::process::exit(10);
      }
    }
  }
}

pub async fn create_graph_and_maybe_check(
  root: ModuleSpecifier,
  ps: &ProcState,
) -> Result<Arc<deno_graph::ModuleGraph>, AnyError> {
  let mut cache = cache::FetchCacher::new(
    ps.emit_cache.clone(),
    ps.file_fetcher.clone(),
    PermissionsContainer::allow_all(),
    PermissionsContainer::allow_all(),
  );
  let maybe_imports = ps.options.to_maybe_imports()?;
  let maybe_cli_resolver = CliResolver::maybe_new(
    ps.options.to_maybe_jsx_import_source_config(),
    ps.maybe_import_map.clone(),
  );
  let maybe_graph_resolver =
    maybe_cli_resolver.as_ref().map(|r| r.as_graph_resolver());
  let analyzer = ps.parsed_source_cache.as_analyzer();
  let graph = Arc::new(
    deno_graph::create_graph(
      vec![root],
      &mut cache,
      deno_graph::GraphOptions {
        is_dynamic: false,
        imports: maybe_imports,
        resolver: maybe_graph_resolver,
        module_analyzer: Some(&*analyzer),
        reporter: None,
      },
    )
    .await,
  );

  let check_js = ps.options.check_js();
  let mut graph_data = GraphData::default();
  graph_data.add_graph(&graph, false);
  graph_data
    .check(
      &graph.roots,
      ps.options.type_check_mode() != TypeCheckMode::None,
      check_js,
    )
    .unwrap()?;
  ps.npm_resolver
    .add_package_reqs(graph_data.npm_package_reqs().clone())
    .await?;
  if let Some(lockfile) = &ps.lockfile {
    graph_lock_or_exit(&graph, &mut lockfile.lock());
  }

  if ps.options.type_check_mode() != TypeCheckMode::None {
    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now after the lockfile has been written
    if graph_data.has_node_builtin_specifier() {
      ps.npm_resolver
        .inject_synthetic_types_node_package()
        .await?;
    }

    let ts_config_result =
      ps.options.resolve_ts_config_for_emit(TsConfigType::Check {
        lib: ps.options.ts_type_lib_window(),
      })?;
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }
    let maybe_config_specifier = ps.options.maybe_config_file_specifier();
    let cache = TypeCheckCache::new(&ps.dir.type_checking_cache_db_file_path());
    let check_result = check::check(
      &graph.roots,
      Arc::new(RwLock::new(graph_data)),
      &cache,
      &ps.npm_resolver,
      check::CheckOptions {
        type_check_mode: ps.options.type_check_mode(),
        debug: ps.options.log_level() == Some(log::Level::Debug),
        maybe_config_specifier,
        ts_config: ts_config_result.ts_config,
        log_checks: true,
        reload: ps.options.reload_flag(),
      },
    )?;
    log::debug!("{}", check_result.stats);
    if !check_result.diagnostics.is_empty() {
      return Err(check_result.diagnostics.into());
    }
  }

  Ok(graph)
}

pub fn error_for_any_npm_specifier(
  graph: &deno_graph::ModuleGraph,
) -> Result<(), AnyError> {
  let first_npm_specifier = graph
    .specifiers()
    .filter_map(|(_, r)| match r {
      Ok((specifier, kind, _)) if kind == deno_graph::ModuleKind::External => {
        Some(specifier)
      }
      _ => None,
    })
    .next();
  if let Some(npm_specifier) = first_npm_specifier {
    bail!("npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: {}", npm_specifier)
  } else {
    Ok(())
  }
}

fn handle_check_error(
  error: AnyError,
  maybe_range: Option<&deno_graph::Range>,
) -> Result<(), AnyError> {
  let mut message = if let Some(err) = error.downcast_ref::<ResolutionError>() {
    enhanced_resolution_error_message(err)
  } else {
    format!("{error}")
  };

  if let Some(range) = maybe_range {
    if !range.specifier.as_str().contains("$deno") {
      message.push_str(&format!("\n    at {range}"));
    }
  }

  Err(custom_error(get_error_class_name(&error), message))
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format!("{error}");

  if let Some(specifier) = get_resolution_error_bare_node_specifier(error) {
    message.push_str(&format!(
        "\nIf you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."
      ));
  }

  message
}

pub fn get_resolution_error_bare_node_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  get_resolution_error_bare_specifier(error).filter(|specifier| {
    crate::node::resolve_builtin_node_module(specifier).is_ok()
  })
}

fn get_resolution_error_bare_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  if let ResolutionError::InvalidSpecifier {
    error: SpecifierError::ImportPrefixMissing(specifier, _),
    ..
  } = error
  {
    Some(specifier.as_str())
  } else if let ResolutionError::ResolverError { error, .. } = error {
    if let Some(ImportMapError::UnmappedBareSpecifier(specifier, _)) =
      error.downcast_ref::<ImportMapError>()
    {
      Some(specifier.as_str())
    } else {
      None
    }
  } else {
    None
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_ast::ModuleSpecifier;
  use deno_graph::Position;
  use deno_graph::Range;
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;

  use crate::graph_util::get_resolution_error_bare_node_specifier;

  #[test]
  fn import_map_node_resolution_error() {
    let cases = vec![("fs", Some("fs")), ("other", None)];
    for (input, output) in cases {
      let import_map = import_map::ImportMap::new(
        ModuleSpecifier::parse("file:///deno.json").unwrap(),
      );
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
      let err = import_map.resolve(input, &specifier).err().unwrap();
      let err = ResolutionError::ResolverError {
        error: Arc::new(err.into()),
        specifier: input.to_string(),
        range: Range {
          specifier,
          start: Position::zeroed(),
          end: Position::zeroed(),
        },
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output);
    }
  }

  #[test]
  fn bare_specifier_node_resolution_error() {
    let cases = vec![("process", Some("process")), ("other", None)];
    for (input, output) in cases {
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
      let err = ResolutionError::InvalidSpecifier {
        range: Range {
          specifier,
          start: Position::zeroed(),
          end: Position::zeroed(),
        },
        error: SpecifierError::ImportPrefixMissing(input.to_string(), None),
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output,);
    }
  }
}
