// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::emit::TsTypeLib;
use crate::errors::get_error_class_name;

use deno_ast::ParsedSource;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::Dependency;
use deno_graph::GraphImport;
use deno_graph::MediaType;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::Range;
use deno_graph::Resolved;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

/// Matches the `@ts-check` pragma.
static TS_CHECK_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"(?i)^\s*@ts-check(?:\s+|$)"#).unwrap());

pub fn contains_specifier(
  v: &[(ModuleSpecifier, ModuleKind)],
  specifier: &ModuleSpecifier,
) -> bool {
  v.iter().any(|(s, _)| s == specifier)
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ModuleEntry {
  Module {
    code: Arc<str>,
    maybe_parsed_source: Option<ParsedSource>,
    dependencies: BTreeMap<String, Dependency>,
    media_type: MediaType,
    /// Whether or not this is a JS/JSX module with a `@ts-check` directive.
    ts_check: bool,
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
  /// Map of first known referrer locations for each module. Used to enhance
  /// error messages.
  referrer_map: HashMap<ModuleSpecifier, Range>,
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
      // TODO(nayeemrmn): Implement `Clone` on `GraphImport`.
      self.graph_imports.push(GraphImport {
        referrer: graph_import.referrer.clone(),
        dependencies: graph_import.dependencies.clone(),
      });
    }

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
        Ok((_, _, media_type)) => {
          let module = graph.get(&specifier).unwrap();
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
          let ts_check = match &media_type {
            MediaType::JavaScript
            | MediaType::Mjs
            | MediaType::Cjs
            | MediaType::Jsx => {
              let parsed_source = module.maybe_parsed_source.as_ref().unwrap();
              parsed_source
                .get_leading_comments()
                .iter()
                .any(|c| TS_CHECK_RE.is_match(&c.text))
            }
            _ => false,
          };
          let module_entry = ModuleEntry::Module {
            code,
            maybe_parsed_source: module.maybe_parsed_source.clone(),
            dependencies: module.dependencies.clone(),
            ts_check,
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

  pub fn entries(
    &self,
  ) -> impl Iterator<Item = (&ModuleSpecifier, &ModuleEntry)> {
    self.modules.iter()
  }

  /// Walk dependencies from `roots` and return every encountered specifier.
  /// Return `None` if any modules are not known.
  pub fn walk<'a>(
    &'a self,
    roots: &[(ModuleSpecifier, ModuleKind)],
    follow_dynamic: bool,
    follow_type_only: bool,
    check_js: bool,
  ) -> Option<HashMap<&'a ModuleSpecifier, &'a ModuleEntry>> {
    let mut result = HashMap::<&'a ModuleSpecifier, &'a ModuleEntry>::new();
    let mut seen = HashSet::<&ModuleSpecifier>::new();
    let mut visiting = VecDeque::<&ModuleSpecifier>::new();
    for (root, _) in roots {
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
            if let Some(Resolved::Ok { specifier, .. }) = maybe_types {
              if !seen.contains(specifier) {
                seen.insert(specifier);
                visiting.push_front(specifier);
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
  pub fn graph_segment(
    &self,
    roots: &[(ModuleSpecifier, ModuleKind)],
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
      // TODO(nayeemrmn): Implement `Clone` on `GraphImport`.
      graph_imports: self
        .graph_imports
        .iter()
        .map(|i| GraphImport {
          referrer: i.referrer.clone(),
          dependencies: i.dependencies.clone(),
        })
        .collect(),
      cjs_esm_translations: Default::default(),
    })
  }

  /// Check if `roots` and their deps are available. Returns `Some(Ok(()))` if
  /// so. Returns `Some(Err(_))` if there is a known module graph or resolution
  /// error statically reachable from `roots`. Returns `None` if any modules are
  /// not known.
  pub fn check(
    &self,
    roots: &[(ModuleSpecifier, ModuleKind)],
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
              if !range.specifier.as_str().contains("$deno") {
                return Some(Err(custom_error(
                  get_error_class_name(&error.clone().into()),
                  format!("{}\n    at {}", error, range),
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
                if let Resolved::Err(error) = resolved {
                  let range = error.range();
                  if !range.specifier.as_str().contains("$deno") {
                    return Some(Err(custom_error(
                      get_error_class_name(&error.clone().into()),
                      format!("{}\n    at {}", error, range),
                    )));
                  }
                  return Some(Err(error.clone().into()));
                }
              }
            }
          }
        }
        ModuleEntry::Error(error) => {
          if !contains_specifier(roots, specifier) {
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
  pub fn set_type_checked(
    &mut self,
    roots: &[(ModuleSpecifier, ModuleKind)],
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
    roots: &[(ModuleSpecifier, ModuleKind)],
    lib: &TsTypeLib,
  ) -> bool {
    roots.iter().all(|(r, _)| {
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

  // TODO(bartlomieju): after saving translated source
  // it's never removed, potentially leading to excessive
  // memory consumption
  pub fn add_cjs_esm_translation(
    &mut self,
    specifier: &ModuleSpecifier,
    source: String,
  ) {
    let prev = self
      .cjs_esm_translations
      .insert(specifier.to_owned(), source);
    assert!(prev.is_none());
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

/// Calls `graph.lock()` and exits on errors.
pub fn graph_lock_or_exit(graph: &ModuleGraph) {
  if let Err(err) = graph.lock() {
    log::error!("{} {}", colors::red("error:"), err);
    std::process::exit(10);
  }
}
