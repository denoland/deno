// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::emit::TypeLib;
use crate::errors::get_error_class_name;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::Dependency;
use deno_graph::MediaType;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::Range;
use deno_graph::ResolutionError;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum ModuleEntry {
  Module {
    code: Arc<String>,
    dependencies: BTreeMap<String, Dependency>,
    media_type: MediaType,
    /// A set of type libs that the module has passed a type check with this
    /// session. This would consist of window, worker or both.
    checked_libs: HashSet<TypeLib>,
    maybe_types: Option<Result<(ModuleSpecifier, Range), ResolutionError>>,
  },
  Configuration {
    dependencies:
      BTreeMap<String, Result<(ModuleSpecifier, Range), ResolutionError>>,
  },
  Error(ModuleGraphError),
  Redirect(ModuleSpecifier),
}

/// Composes data from potentially many `ModuleGraph`s.
#[derive(Debug, Default)]
pub(crate) struct GraphData {
  modules: HashMap<ModuleSpecifier, ModuleEntry>,
  /// Map of first known referrer locations for each module. Used to enhance
  /// error messages.
  referrer_map: HashMap<ModuleSpecifier, Range>,
  configurations: HashSet<ModuleSpecifier>,
}

impl GraphData {
  /// Store data from `graph` into `self`.
  pub(crate) fn add_graph(&mut self, graph: &ModuleGraph, reload: bool) {
    for (specifier, result) in graph.specifiers() {
      if !reload && self.modules.contains_key(&specifier) {
        continue;
      }
      if let Some(found) = graph.redirects.get(&specifier) {
        let module_entry = ModuleEntry::Redirect(found.clone());
        self.modules.insert(specifier.clone(), module_entry);
        continue;
      }
      match result {
        Ok((_, media_type)) => {
          let module = graph.get(&specifier).unwrap();
          let (code, dependencies, maybe_types) = match module {
            Module::Es(es_module) => (
              es_module.source.clone(),
              es_module.dependencies.clone(),
              es_module
                .maybe_types_dependency
                .as_ref()
                .and_then(|(_, r)| r.clone()),
            ),
            Module::Synthetic(synthetic_module) => match &synthetic_module
              .maybe_source
            {
              // Synthetic modules with a source are actually JSON modules.
              Some(source) => (source.clone(), Default::default(), None),
              // Synthetic modules without a source are config roots.
              None => {
                let mut dependencies = BTreeMap::new();
                for (specifier, resolved) in &synthetic_module.dependencies {
                  if let Some(dep_result) = resolved {
                    dependencies.insert(specifier.clone(), dep_result.clone());
                    if let Ok((specifier, referrer_range)) = dep_result {
                      let entry = self.referrer_map.entry(specifier.clone());
                      entry.or_insert_with(|| referrer_range.clone());
                    }
                  }
                }
                self.modules.insert(
                  synthetic_module.specifier.clone(),
                  ModuleEntry::Configuration { dependencies },
                );
                self
                  .configurations
                  .insert(synthetic_module.specifier.clone());
                continue;
              }
            },
          };
          if let Some(Ok((specifier, referrer_range))) = &maybe_types {
            let specifier = graph.redirects.get(specifier).unwrap_or(specifier);
            let entry = self.referrer_map.entry(specifier.clone());
            entry.or_insert_with(|| referrer_range.clone());
          }
          for dep in dependencies.values() {
            #[allow(clippy::manual_flatten)]
            for resolved in [&dep.maybe_code, &dep.maybe_type] {
              if let Some(Ok((specifier, referrer_range))) = resolved {
                let specifier =
                  graph.redirects.get(specifier).unwrap_or(specifier);
                let entry = self.referrer_map.entry(specifier.clone());
                entry.or_insert_with(|| referrer_range.clone());
              }
            }
          }
          let module_entry = ModuleEntry::Module {
            code,
            dependencies,
            media_type,
            checked_libs: Default::default(),
            maybe_types,
          };
          self.modules.insert(specifier, module_entry);
        }
        Err(error) => {
          let module_entry = ModuleEntry::Error(error);
          self.modules.insert(specifier, module_entry);
        }
      }
    }
  }

  pub(crate) fn entries(&self) -> HashMap<&ModuleSpecifier, &ModuleEntry> {
    self.modules.iter().collect()
  }

  /// Walk dependencies from `roots` and return every encountered specifier.
  /// Return `None` if any modules are not known.
  pub(crate) fn walk<'a>(
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
    for root in &self.configurations {
      seen.insert(root);
      visiting.push_back(root);
    }
    while let Some(specifier) = visiting.pop_front() {
      let (specifier, entry) = match self.modules.get_key_value(specifier) {
        Some(pair) => pair,
        None => return None,
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
            if let Some(Ok((types, _))) = maybe_types {
              if !seen.contains(types) {
                seen.insert(types);
                visiting.push_front(types);
              }
            }
          }
          for (_, dep) in dependencies.iter().rev() {
            if !dep.is_dynamic || follow_dynamic {
              let mut resolutions = vec![&dep.maybe_code];
              if check_types {
                resolutions.push(&dep.maybe_type);
              }
              #[allow(clippy::manual_flatten)]
              for resolved in resolutions {
                if let Some(Ok((dep_specifier, _))) = resolved {
                  if !seen.contains(dep_specifier) {
                    seen.insert(dep_specifier);
                    visiting.push_front(dep_specifier);
                  }
                }
              }
            }
          }
        }
        ModuleEntry::Configuration { dependencies } => {
          for (dep_specifier, _) in dependencies.values().flatten() {
            if !seen.contains(dep_specifier) {
              seen.insert(dep_specifier);
              visiting.push_front(dep_specifier);
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
  pub(crate) fn graph_segment(
    &self,
    roots: &[ModuleSpecifier],
  ) -> Option<Self> {
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
      referrer_map,
      configurations: self.configurations.clone(),
    })
  }

  /// Check if `roots` and their deps are available. Returns `Some(Ok(()))` if
  /// so. Returns `Some(Err(_))` if there is a known module graph or resolution
  /// error statically reachable from `roots`. Returns `None` if any modules are
  /// not known.
  pub(crate) fn check(
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
            if let Some(Err(error)) = maybe_types {
              let range = error.range();
              if !range.specifier.as_str().contains("$deno") {
                return Some(Err(custom_error(
                  get_error_class_name(&error.clone().into()),
                  format!("{}\n    at {}", error.to_string(), range),
                )));
              }
              return Some(Err(error.clone().into()));
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
                if let Some(Err(error)) = resolved {
                  let range = error.range();
                  if !range.specifier.as_str().contains("$deno") {
                    return Some(Err(custom_error(
                      get_error_class_name(&error.clone().into()),
                      format!("{}\n    at {}", error.to_string(), range),
                    )));
                  }
                  return Some(Err(error.clone().into()));
                }
              }
            }
          }
        }
        ModuleEntry::Configuration { dependencies } => {
          for resolved_result in dependencies.values() {
            if let Err(error) = resolved_result {
              let range = error.range();
              if !range.specifier.as_str().contains("$deno") {
                return Some(Err(custom_error(
                  get_error_class_name(&error.clone().into()),
                  format!("{}\n    at {}", error.to_string(), range),
                )));
              }
              return Some(Err(error.clone().into()));
            }
          }
        }
        ModuleEntry::Error(error) => {
          if !roots.contains(specifier) {
            if let Some(range) = self.referrer_map.get(specifier) {
              if !range.specifier.as_str().contains("$deno") {
                let message = error.to_string();
                return Some(Err(custom_error(
                  get_error_class_name(&error.clone().into()),
                  format!("{}\n    at {}", message, range),
                )));
              }
            }
          }
          return Some(Err(error.clone().into()));
        }
        _ => {}
      }
    }
    Some(Ok(()))
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub(crate) fn set_type_checked(
    &mut self,
    roots: &[ModuleSpecifier],
    lib: &TypeLib,
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
        checked_libs.insert(lib.clone());
      }
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub(crate) fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: &TypeLib,
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
  pub(crate) fn follow_redirect(
    &self,
    specifier: &ModuleSpecifier,
  ) -> ModuleSpecifier {
    match self.modules.get(specifier) {
      Some(ModuleEntry::Redirect(s)) => s.clone(),
      _ => specifier.clone(),
    }
  }

  pub(crate) fn get<'a>(
    &'a self,
    specifier: &ModuleSpecifier,
  ) -> Option<&'a ModuleEntry> {
    self.modules.get(specifier)
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
pub(crate) fn graph_valid(
  graph: &ModuleGraph,
  follow_type_only: bool,
  check_js: bool,
) -> Result<(), AnyError> {
  GraphData::from(graph)
    .check(&graph.roots, follow_type_only, check_js)
    .unwrap()
}

/// Calls `graph.lock()` and exits on errors.
pub(crate) fn graph_lock_or_exit(graph: &ModuleGraph) {
  if let Err(err) = graph.lock() {
    log::error!("{} {}", colors::red("error:"), err);
    std::process::exit(10);
  }
}
