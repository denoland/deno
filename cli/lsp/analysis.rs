// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_npm::NpmPackageId;
use deno_path_util::url_to_file_path;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_runtime::deno_node::PathClean;
use deno_semver::jsr::JsrPackageNvReference;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageNvReference;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use import_map::ImportMap;
use lsp_types::Uri;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;

use super::diagnostics::DenoDiagnostic;
use super::diagnostics::DiagnosticSource;
use super::documents::DocumentModule;
use super::documents::DocumentModules;
use super::language_server;
use super::resolver::LspResolver;
use super::tsc;
use crate::args::jsr_url;
use crate::tools::lint::CliLinter;
use crate::util::path::relative_specifier;

/// Diagnostic error codes which actually are the same, and so when grouping
/// fixes we treat them the same.
static FIX_ALL_ERROR_CODES: Lazy<HashMap<&'static str, &'static str>> =
  Lazy::new(|| ([("2339", "2339"), ("2345", "2339")]).into_iter().collect());

/// Fixes which help determine if there is a preferred fix when there are
/// multiple fixes available.
static PREFERRED_FIXES: Lazy<HashMap<&'static str, (u32, bool)>> =
  Lazy::new(|| {
    ([
      ("annotateWithTypeFromJSDoc", (1, false)),
      ("constructorForDerivedNeedSuperCall", (1, false)),
      ("extendsInterfaceBecomesImplements", (1, false)),
      ("awaitInSyncFunction", (1, false)),
      ("classIncorrectlyImplementsInterface", (3, false)),
      ("classDoesntImplementInheritedAbstractMember", (3, false)),
      ("unreachableCode", (1, false)),
      ("unusedIdentifier", (1, false)),
      ("forgottenThisPropertyAccess", (1, false)),
      ("spelling", (2, false)),
      ("addMissingAwait", (1, false)),
      ("fixImport", (0, true)),
    ])
    .into_iter()
    .collect()
  });

static IMPORT_SPECIFIER_RE: Lazy<Regex> = lazy_regex::lazy_regex!(
  r#"\sfrom\s+["']([^"']*)["']|import\s*\(\s*["']([^"']*)["']\s*\)"#
);

const SUPPORTED_EXTENSIONS: &[&str] = &[
  ".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts", ".cjs", ".cts", ".d.ts",
  ".d.mts", ".d.cts",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataQuickFixChange {
  pub range: Range,
  pub new_text: String,
}

/// A quick fix that's stored in the diagnostic's data field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataQuickFix {
  pub description: String,
  pub changes: Vec<DataQuickFixChange>,
}

/// Category of self-generated diagnostic messages (those not coming from)
/// TypeScript.
#[derive(Debug, PartialEq, Eq)]
pub enum Category {
  /// A lint diagnostic, where the first element is the message.
  Lint {
    message: String,
    code: String,
    hint: Option<String>,
    quick_fixes: Vec<DataQuickFix>,
  },
}

/// A structure to hold a reference to a diagnostic message.
#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
  category: Category,
  range: Range,
}

impl Reference {
  pub fn to_diagnostic(&self) -> lsp::Diagnostic {
    match &self.category {
      Category::Lint {
        message,
        code,
        hint,
        quick_fixes,
      } => lsp::Diagnostic {
        range: self.range,
        severity: Some(lsp::DiagnosticSeverity::WARNING),
        code: Some(lsp::NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some(DiagnosticSource::Lint.as_lsp_source().to_string()),
        message: {
          let mut msg = message.to_string();
          if let Some(hint) = hint {
            msg.push('\n');
            msg.push_str(hint);
          }
          msg
        },
        related_information: None,
        tags: None, // we should tag unused code
        data: if quick_fixes.is_empty() {
          None
        } else {
          serde_json::to_value(quick_fixes).ok()
        },
      },
    }
  }
}

fn as_lsp_range_from_lint_diagnostic(
  diagnostic_range: &LintDiagnosticRange,
) -> Range {
  as_lsp_range(diagnostic_range.range, &diagnostic_range.text_info)
}

fn as_lsp_range(
  source_range: SourceRange,
  text_info: &SourceTextInfo,
) -> Range {
  let start_lc = text_info.line_and_column_index(source_range.start);
  let end_lc = text_info.line_and_column_index(source_range.end);
  Range {
    start: Position {
      line: start_lc.line_index as u32,
      character: start_lc.column_index as u32,
    },
    end: Position {
      line: end_lc.line_index as u32,
      character: end_lc.column_index as u32,
    },
  }
}

pub fn get_lint_references(
  parsed_source: &deno_ast::ParsedSource,
  linter: &CliLinter,
  token: CancellationToken,
) -> Result<Vec<Reference>, AnyError> {
  let lint_diagnostics = linter.lint_with_ast(parsed_source, token)?;

  Ok(
    lint_diagnostics
      .into_iter()
      .filter_map(|d| {
        let range = d.range.as_ref()?;
        Some(Reference {
          range: as_lsp_range_from_lint_diagnostic(range),
          category: Category::Lint {
            message: d.details.message,
            code: d.details.code.to_string(),
            hint: d.details.hint,
            quick_fixes: d
              .details
              .fixes
              .into_iter()
              .map(|f| DataQuickFix {
                description: f.description.to_string(),
                changes: f
                  .changes
                  .into_iter()
                  .map(|change| DataQuickFixChange {
                    range: as_lsp_range(change.range, &range.text_info),
                    new_text: change.new_text.to_string(),
                  })
                  .collect(),
              })
              .collect(),
          },
        })
      })
      .collect(),
  )
}

fn code_as_string(code: &Option<lsp::NumberOrString>) -> String {
  match code {
    Some(lsp::NumberOrString::String(str)) => str.clone(),
    Some(lsp::NumberOrString::Number(num)) => num.to_string(),
    _ => "".to_string(),
  }
}

/// Rewrites imports in quick fixes and code changes to be Deno specific.
pub struct TsResponseImportMapper<'a> {
  document_modules: &'a DocumentModules,
  scope: Option<Arc<ModuleSpecifier>>,
  maybe_import_map: Option<&'a ImportMap>,
  resolver: &'a LspResolver,
  tsc_specifier_map: &'a tsc::TscSpecifierMap,
}

impl<'a> TsResponseImportMapper<'a> {
  pub fn new(
    document_modules: &'a DocumentModules,
    scope: Option<Arc<ModuleSpecifier>>,
    maybe_import_map: Option<&'a ImportMap>,
    resolver: &'a LspResolver,
    tsc_specifier_map: &'a tsc::TscSpecifierMap,
  ) -> Self {
    Self {
      document_modules,
      scope,
      maybe_import_map,
      resolver,
      tsc_specifier_map,
    }
  }

  pub fn check_specifier(
    &self,
    specifier: &ModuleSpecifier,
    referrer: &ModuleSpecifier,
  ) -> Option<String> {
    fn concat_npm_specifier(
      prefix: &str,
      pkg_req: &PackageReq,
      sub_path: Option<&str>,
    ) -> String {
      let result = format!("{}{}", prefix, pkg_req);
      match sub_path {
        Some(path) => format!("{}/{}", result, path),
        None => result,
      }
    }

    if specifier.scheme() == "node" {
      return Some(specifier.to_string());
    }

    let scoped_resolver =
      self.resolver.get_scoped_resolver(self.scope.as_deref());

    if let Some(jsr_path) = specifier.as_str().strip_prefix(jsr_url().as_str())
    {
      let mut segments = jsr_path.split('/');
      let name = if jsr_path.starts_with('@') {
        let scope = segments.next()?;
        let name = segments.next()?;
        capacity_builder::StringBuilder::<StackString>::build(|builder| {
          builder.append(scope);
          builder.append("/");
          builder.append(name);
        })
        .unwrap()
      } else {
        StackString::from(segments.next()?)
      };
      let version = Version::parse_standard(segments.next()?).ok()?;
      let nv = PackageNv { name, version };
      let path = segments.collect::<Vec<_>>().join("/");
      let export = scoped_resolver.jsr_lookup_export_for_path(&nv, &path)?;
      let sub_path = (export != ".")
        .then_some(export)
        .map(SmallStackString::from_string);
      let mut req = None;
      req = req.or_else(|| {
        let import_map = self.maybe_import_map?;
        for entry in import_map.entries_for_referrer(referrer) {
          let Some(value) = entry.raw_value else {
            continue;
          };
          let Ok(req_ref) = JsrPackageReqReference::from_str(value) else {
            continue;
          };
          let req = req_ref.req();
          if req.name == nv.name
            && req.version_req.tag().is_none()
            && req.version_req.matches(&nv.version)
          {
            return Some(req.clone());
          }
        }
        None
      });
      req = req.or_else(|| scoped_resolver.jsr_lookup_req_for_nv(&nv));
      let spec_str = if let Some(req) = req {
        let req_ref = PackageReqReference { req, sub_path };
        JsrPackageReqReference::new(req_ref).to_string()
      } else {
        let nv_ref = PackageNvReference { nv, sub_path };
        JsrPackageNvReference::new(nv_ref).to_string()
      };
      let specifier = ModuleSpecifier::parse(&spec_str).ok()?;
      if let Some(import_map) = self.maybe_import_map {
        if let Some(result) = import_map.lookup(&specifier, referrer) {
          return Some(result);
        }
        if let Some(req_ref_str) = specifier.as_str().strip_prefix("jsr:") {
          if !req_ref_str.starts_with('/') {
            let specifier_str = format!("jsr:/{req_ref_str}");
            if let Ok(specifier) = ModuleSpecifier::parse(&specifier_str) {
              if let Some(result) = import_map.lookup(&specifier, referrer) {
                return Some(result);
              }
            }
          }
        }
      }
      return Some(spec_str);
    }

    if let Some(npm_resolver) = scoped_resolver.as_maybe_managed_npm_resolver()
    {
      let in_npm_pkg = scoped_resolver
        .as_in_npm_pkg_checker()
        .in_npm_package(specifier);
      if in_npm_pkg {
        if let Ok(Some(pkg_id)) =
          npm_resolver.resolve_pkg_id_from_specifier(specifier)
        {
          let pkg_reqs =
            maybe_reverse_definitely_typed(&pkg_id, npm_resolver.resolution())
              .unwrap_or_else(|| {
                npm_resolver
                  .resolution()
                  .resolve_pkg_reqs_from_pkg_id(&pkg_id)
              });
          // check if any pkg reqs match what is found in an import map
          if !pkg_reqs.is_empty() {
            let sub_path = npm_resolver
              .resolve_pkg_folder_from_pkg_id(&pkg_id)
              .ok()
              .and_then(|pkg_folder| {
                self.resolve_package_path(specifier, &pkg_folder)
              });
            if let Some(import_map) = self.maybe_import_map {
              let pkg_reqs = pkg_reqs.iter().collect::<HashSet<_>>();
              let mut matches = Vec::new();
              for entry in import_map.entries_for_referrer(referrer) {
                if let Some(value) = entry.raw_value {
                  if let Ok(package_ref) =
                    NpmPackageReqReference::from_str(value)
                  {
                    if pkg_reqs.contains(package_ref.req()) {
                      let sub_path = sub_path.as_deref().unwrap_or("");
                      let value_sub_path = package_ref.sub_path().unwrap_or("");
                      if let Some(key_sub_path) =
                        sub_path.strip_prefix(value_sub_path)
                      {
                        // keys that don't end in a slash can't be mapped to a subpath
                        if entry.raw_key.ends_with('/')
                          || key_sub_path.is_empty()
                        {
                          matches
                            .push(format!("{}{}", entry.raw_key, key_sub_path));
                        }
                      }
                    }
                  }
                }
              }
              // select the shortest match
              matches.sort_by_key(|a| a.len());
              if let Some(matched) = matches.first() {
                return Some(matched.to_string());
              }
            }

            // if not found in the import map, return the first pkg req
            if let Some(pkg_req) = pkg_reqs.first() {
              return Some(concat_npm_specifier(
                "npm:",
                pkg_req,
                sub_path.as_deref(),
              ));
            }
          }
        }
      }
    } else if let Some(dep_name) =
      scoped_resolver.file_url_to_package_json_dep(specifier)
    {
      return Some(dep_name);
    }

    // check if the import map has this specifier
    if let Some(import_map) = self.maybe_import_map {
      if let Some(result) = import_map.lookup(specifier, referrer) {
        return Some(result);
      }
    }

    None
  }

  fn resolve_package_path(
    &self,
    specifier: &ModuleSpecifier,
    package_root_folder: &Path,
  ) -> Option<String> {
    let scoped_resolver = self.resolver.get_scoped_resolver(Some(specifier));
    let package_json = scoped_resolver
      .as_pkg_json_resolver()
      // the specifier might have a closer package.json, but we
      // want the root of the package's package.json
      .get_closest_package_json(&package_root_folder.join("package.json"))
      .ok()
      .flatten()?;
    let root_folder = package_json.path.parent()?;

    let specifier_path = url_to_file_path(specifier).ok()?;
    let mut search_paths = vec![specifier_path.clone()];
    // TypeScript will provide a .js extension for quick fixes, so do
    // a search for the .d.ts file instead
    if specifier_path.extension().and_then(|e| e.to_str()) == Some("js") {
      search_paths.insert(0, specifier_path.with_extension("d.ts"));
    } else if let Some(file_name) =
      specifier_path.file_name().and_then(|f| f.to_str())
    {
      // In some other cases, typescript will provide the .d.ts extension, but the
      // export might not have a .d.ts defined. In that case, look for the corresponding
      // JavaScript file after not being able to find the .d.ts file.
      if let Some(file_stem) = file_name.strip_suffix(".d.ts") {
        search_paths
          .push(specifier_path.with_file_name(format!("{}.js", file_stem)));
      } else if let Some(file_stem) = file_name.strip_suffix(".d.cts") {
        search_paths
          .push(specifier_path.with_file_name(format!("{}.cjs", file_stem)));
      } else if let Some(file_stem) = file_name.strip_suffix(".d.mts") {
        search_paths
          .push(specifier_path.with_file_name(format!("{}.mjs", file_stem)));
      }
    }

    for search_path in search_paths {
      if let Some(exports) = &package_json.exports {
        if let Some(result) = try_reverse_map_package_json_exports(
          root_folder,
          &search_path,
          exports,
        ) {
          return Some(result);
        }
      }
    }

    None
  }

  /// Iterate over the supported extensions, concatenating the extension on the
  /// specifier, returning the first specifier that is resolve-able, otherwise
  /// None if none match.
  pub fn check_unresolved_specifier(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    resolution_mode: ResolutionMode,
  ) -> Option<String> {
    let specifier_stem = specifier.strip_suffix(".js").unwrap_or(specifier);
    let specifiers = std::iter::once(Cow::Borrowed(specifier)).chain(
      SUPPORTED_EXTENSIONS
        .iter()
        .map(|ext| Cow::Owned(format!("{specifier_stem}{ext}"))),
    );
    let scoped_resolver =
      self.resolver.get_scoped_resolver(self.scope.as_deref());
    for specifier in specifiers {
      if let Some(specifier) = scoped_resolver
        .as_cli_resolver()
        .resolve(
          &specifier,
          referrer,
          deno_graph::Position::zeroed(),
          resolution_mode,
          NodeResolutionKind::Types,
        )
        .ok()
        .and_then(|s| self.tsc_specifier_map.normalize(s.as_str()).ok())
        .filter(|s| {
          self
            .document_modules
            .specifier_exists(s, self.scope.as_deref())
        })
      {
        if let Some(specifier) = self
          .check_specifier(&specifier, referrer)
          .or_else(|| relative_specifier(referrer, &specifier))
          .filter(|s| !s.contains("/node_modules/"))
        {
          return Some(specifier);
        }
      }
    }
    None
  }

  pub fn is_valid_import(
    &self,
    specifier_text: &str,
    referrer: &ModuleSpecifier,
    resolution_mode: ResolutionMode,
  ) -> bool {
    self
      .resolver
      .get_scoped_resolver(self.scope.as_deref())
      .as_cli_resolver()
      .resolve(
        specifier_text,
        referrer,
        deno_graph::Position::zeroed(),
        resolution_mode,
        NodeResolutionKind::Types,
      )
      .is_ok()
  }
}

fn maybe_reverse_definitely_typed(
  pkg_id: &NpmPackageId,
  resolution: &NpmResolutionCell,
) -> Option<Vec<PackageReq>> {
  let rest = pkg_id.nv.name.strip_prefix("@types/")?;
  let package_name = if rest.contains("__") {
    Cow::Owned(format!("@{}", rest.replace("__", "/")))
  } else {
    Cow::Borrowed(rest)
  };

  let reqs = resolution
    .package_reqs()
    .into_iter()
    .filter_map(|(req, nv)| (*nv.name == package_name).then_some(req))
    .collect::<Vec<_>>();

  if reqs.is_empty() {
    None
  } else {
    Some(reqs)
  }
}

fn try_reverse_map_package_json_exports(
  root_path: &Path,
  target_path: &Path,
  exports: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
  use deno_core::serde_json::Value;

  fn try_reverse_map_package_json_exports_inner(
    root_path: &Path,
    target_path: &Path,
    exports: &serde_json::Map<String, Value>,
  ) -> Option<String> {
    for (key, value) in exports {
      match value {
        Value::String(str) => {
          if root_path.join(str).clean() == target_path {
            return Some(if let Some(suffix) = key.strip_prefix("./") {
              suffix.to_string()
            } else {
              String::new() // condition (ex. "types"), ignore
            });
          }
        }
        Value::Object(obj) => {
          if let Some(result) = try_reverse_map_package_json_exports_inner(
            root_path,
            target_path,
            obj,
          ) {
            return Some(if let Some(suffix) = key.strip_prefix("./") {
              if result.is_empty() {
                suffix.to_string()
              } else {
                format!("{}/{}", suffix, result)
              }
            } else {
              result // condition (ex. "types"), ignore
            });
          }
        }
        _ => {}
      }
    }
    None
  }

  let result = try_reverse_map_package_json_exports_inner(
    root_path,
    target_path,
    exports,
  )?;
  if result.is_empty() {
    None
  } else {
    Some(result)
  }
}

/// For a set of tsc changes, can them for any that contain something that looks
/// like an import and rewrite the import specifier to include the extension
pub fn fix_ts_import_changes(
  changes: &[tsc::FileTextChanges],
  module: &DocumentModule,
  language_server: &language_server::Inner,
  token: &CancellationToken,
) -> Result<Vec<tsc::FileTextChanges>, AnyError> {
  let mut r = Vec::new();
  for change in changes {
    if token.is_cancelled() {
      return Err(anyhow!("request cancelled"));
    }
    let is_new_file = change.is_new_file.unwrap_or(false);
    let Ok(target_specifier) = resolve_url(&change.file_name) else {
      continue;
    };
    let target_module = if is_new_file {
      None
    } else {
      let Some(target_module) = language_server
        .document_modules
        .inspect_module_for_specifier(
          &target_specifier,
          module.scope.as_deref(),
        )
      else {
        continue;
      };
      Some(target_module)
    };
    let resolution_mode = target_module
      .as_ref()
      .map(|m| m.resolution_mode)
      .unwrap_or(ResolutionMode::Import);
    let import_mapper = language_server.get_ts_response_import_mapper(module);
    let mut text_changes = Vec::new();
    for text_change in &change.text_changes {
      let lines = text_change.new_text.split('\n');
      let new_lines: Vec<String> = lines
        .map(|line| {
          // This assumes that there's only one import per line.
          if let Some(captures) = IMPORT_SPECIFIER_RE.captures(line) {
            let specifier =
              captures.iter().skip(1).find_map(|s| s).unwrap().as_str();
            if let Some(new_specifier) = import_mapper
              .check_unresolved_specifier(
                specifier,
                &target_specifier,
                resolution_mode,
              )
            {
              line.replace(specifier, &new_specifier)
            } else {
              line.to_string()
            }
          } else {
            line.to_string()
          }
        })
        .collect();

      text_changes.push(tsc::TextChange {
        span: text_change.span.clone(),
        new_text: new_lines.join("\n").to_string(),
      });
    }
    r.push(tsc::FileTextChanges {
      file_name: change.file_name.clone(),
      text_changes,
      is_new_file: change.is_new_file,
    });
  }
  Ok(r)
}

/// Fix tsc import code actions so that the module specifier is correct for
/// resolution by Deno (includes the extension).
fn fix_ts_import_action<'a>(
  action: &'a tsc::CodeFixAction,
  module: &DocumentModule,
  language_server: &language_server::Inner,
) -> Option<Cow<'a, tsc::CodeFixAction>> {
  if !matches!(
    action.fix_name.as_str(),
    "import" | "fixMissingFunctionDeclaration"
  ) {
    return Some(Cow::Borrowed(action));
  }
  let specifier = (|| {
    let text_change = action.changes.first()?.text_changes.first()?;
    let captures = IMPORT_SPECIFIER_RE.captures(&text_change.new_text)?;
    Some(captures.get(1)?.as_str())
  })();
  let Some(specifier) = specifier else {
    return Some(Cow::Borrowed(action));
  };
  let import_mapper = language_server.get_ts_response_import_mapper(module);
  if let Some(new_specifier) = import_mapper.check_unresolved_specifier(
    specifier,
    &module.specifier,
    module.resolution_mode,
  ) {
    let description = action.description.replace(specifier, &new_specifier);
    let changes = action
      .changes
      .iter()
      .map(|c| {
        let text_changes = c
          .text_changes
          .iter()
          .map(|tc| tsc::TextChange {
            span: tc.span.clone(),
            new_text: tc.new_text.replace(specifier, &new_specifier),
          })
          .collect();
        tsc::FileTextChanges {
          file_name: c.file_name.clone(),
          text_changes,
          is_new_file: c.is_new_file,
        }
      })
      .collect();

    Some(Cow::Owned(tsc::CodeFixAction {
      description,
      changes,
      commands: None,
      fix_name: action.fix_name.clone(),
      fix_id: None,
      fix_all_description: None,
    }))
  } else if !import_mapper.is_valid_import(
    specifier,
    &module.specifier,
    module.resolution_mode,
  ) {
    None
  } else {
    Some(Cow::Borrowed(action))
  }
}

/// Determines if two TypeScript diagnostic codes are effectively equivalent.
fn is_equivalent_code(
  a: &Option<lsp::NumberOrString>,
  b: &Option<lsp::NumberOrString>,
) -> bool {
  let a_code = code_as_string(a);
  let b_code = code_as_string(b);
  FIX_ALL_ERROR_CODES.get(a_code.as_str())
    == FIX_ALL_ERROR_CODES.get(b_code.as_str())
}

/// Return a boolean flag to indicate if the specified action is the preferred
/// action for a given set of actions.
fn is_preferred(
  action: &tsc::CodeFixAction,
  actions: &[CodeActionKind],
  fix_priority: u32,
  only_one: bool,
) -> bool {
  actions.iter().all(|i| {
    if let CodeActionKind::Tsc(_, a) = i {
      if action == a {
        return true;
      }
      if a.fix_id.is_some() {
        return true;
      }
      if let Some((other_fix_priority, _)) =
        PREFERRED_FIXES.get(a.fix_name.as_str())
      {
        match other_fix_priority.cmp(&fix_priority) {
          Ordering::Less => return true,
          Ordering::Greater => return false,
          Ordering::Equal => (),
        }
        if only_one && action.fix_name == a.fix_name {
          return false;
        }
      }
      true
    } else if let CodeActionKind::Deno(_) = i {
      // This is to make sure 'Remove import' isn't preferred over 'Cache
      // dependencies'.
      return false;
    } else {
      true
    }
  })
}

/// Convert changes returned from a TypeScript quick fix action into edits
/// for an LSP CodeAction.
pub fn ts_changes_to_edit(
  changes: &[tsc::FileTextChanges],
  module: &DocumentModule,
  language_server: &language_server::Inner,
) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
  let mut text_document_edits = Vec::new();
  for change in changes {
    let Some(edit) = change.to_text_document_edit(module, language_server)
    else {
      continue;
    };
    text_document_edits.push(edit);
  }
  Ok(Some(lsp::WorkspaceEdit {
    changes: None,
    document_changes: Some(lsp::DocumentChanges::Edits(text_document_edits)),
    change_annotations: None,
  }))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionData {
  pub uri: Uri,
  pub fix_id: String,
}

#[derive(Debug, Clone)]
enum CodeActionKind {
  Deno(lsp::CodeAction),
  DenoLint(lsp::CodeAction),
  Tsc(lsp::CodeAction, tsc::CodeFixAction),
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum FixAllKind {
  Tsc(String),
}

#[derive(Debug, Default)]
pub struct CodeActionCollection {
  actions: Vec<CodeActionKind>,
  fix_all_actions: HashMap<FixAllKind, CodeActionKind>,
}

impl CodeActionCollection {
  pub fn add_deno_fix_action(
    &mut self,
    uri: &Uri,
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    let code_action =
      DenoDiagnostic::get_code_action(uri, specifier, diagnostic)?;
    self.actions.push(CodeActionKind::Deno(code_action));
    Ok(())
  }

  pub fn add_deno_lint_actions(
    &mut self,
    uri: &Uri,
    module: &DocumentModule,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    if let Some(data_quick_fixes) = diagnostic
      .data
      .as_ref()
      .and_then(|d| serde_json::from_value::<Vec<DataQuickFix>>(d.clone()).ok())
    {
      for quick_fix in data_quick_fixes {
        let mut changes = HashMap::new();
        changes.insert(
          uri.clone(),
          quick_fix
            .changes
            .into_iter()
            .map(|change| lsp::TextEdit {
              new_text: change.new_text.clone(),
              range: change.range,
            })
            .collect(),
        );
        let code_action = lsp::CodeAction {
          title: quick_fix.description.to_string(),
          kind: Some(lsp::CodeActionKind::QUICKFIX),
          diagnostics: Some(vec![diagnostic.clone()]),
          command: None,
          is_preferred: None,
          disabled: None,
          data: None,
          edit: Some(lsp::WorkspaceEdit {
            changes: Some(changes),
            change_annotations: None,
            document_changes: None,
          }),
        };
        self.actions.push(CodeActionKind::DenoLint(code_action));
      }
    }
    self.add_deno_lint_ignore_action(uri, module, diagnostic)
  }

  fn add_deno_lint_ignore_action(
    &mut self,
    uri: &Uri,
    module: &DocumentModule,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    let code = diagnostic
      .code
      .as_ref()
      .map(|v| match v {
        lsp::NumberOrString::String(v) => v.to_owned(),
        _ => "".to_string(),
      })
      .unwrap();
    let text_info = module.text_info();

    let line_content = text_info
      .line_text(diagnostic.range.start.line as usize)
      .to_string();

    let mut changes = HashMap::new();
    changes.insert(
      uri.clone(),
      vec![lsp::TextEdit {
        new_text: prepend_whitespace(
          format!("// deno-lint-ignore {code}\n"),
          Some(line_content),
        ),
        range: lsp::Range {
          start: lsp::Position {
            line: diagnostic.range.start.line,
            character: 0,
          },
          end: lsp::Position {
            line: diagnostic.range.start.line,
            character: 0,
          },
        },
      }],
    );
    let ignore_error_action = lsp::CodeAction {
      title: format!("Disable {code} for this line"),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_error_action));

    // Disable a lint error for the entire file.
    let maybe_ignore_comment = module
      .open_data
      .as_ref()
      .and_then(|d| d.parsed_source.as_ref())
      .and_then(|ps| {
        let ps = ps.as_ref().ok()?;
        // Note: we can use ps.get_leading_comments() but it doesn't
        // work when shebang is present at the top of the file.
        ps.comments().get_vec().iter().find_map(|c| {
          let comment_text = c.text.trim();
          comment_text.split_whitespace().next().and_then(|prefix| {
            if prefix == "deno-lint-ignore-file" {
              Some(c.clone())
            } else {
              None
            }
          })
        })
      });

    let mut new_text = format!("// deno-lint-ignore-file {code}\n");
    let mut range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 0,
      },
      end: lsp::Position {
        line: 0,
        character: 0,
      },
    };
    // If ignore file comment already exists, append the lint code
    // to the existing comment.
    if let Some(ignore_comment) = maybe_ignore_comment {
      new_text = format!(" {code}");
      // Get the end position of the comment.
      let line = text_info.line_and_column_index(ignore_comment.end());
      let position = lsp::Position {
        line: line.line_index as u32,
        character: line.column_index as u32,
      };
      // Set the edit range to the end of the comment.
      range.start = position;
      range.end = position;
    }

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![lsp::TextEdit { new_text, range }]);
    let ignore_file_action = lsp::CodeAction {
      title: format!("Disable {code} for the entire file"),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_file_action));

    let mut changes = HashMap::new();
    changes.insert(
      uri.clone(),
      vec![lsp::TextEdit {
        new_text: "// deno-lint-ignore-file\n".to_string(),
        range: lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 0,
          },
          end: lsp::Position {
            line: 0,
            character: 0,
          },
        },
      }],
    );
    let ignore_file_action = lsp::CodeAction {
      title: "Ignore lint errors for the entire file".to_string(),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_file_action));

    Ok(())
  }

  /// Add a TypeScript code fix action to the code actions collection.
  pub fn add_ts_fix_action(
    &mut self,
    action: &tsc::CodeFixAction,
    diagnostic: &lsp::Diagnostic,
    module: &DocumentModule,
    language_server: &language_server::Inner,
  ) -> Result<(), AnyError> {
    if action.commands.is_some() {
      // In theory, tsc can return actions that require "commands" to be applied
      // back into TypeScript.  Currently there is only one command, `install
      // package` but Deno doesn't support that.  The problem is that the
      // `.applyCodeActionCommand()` returns a promise, and with the current way
      // we wrap tsc, we can't handle the asynchronous response, so it is
      // actually easier to return errors if we ever encounter one of these,
      // which we really wouldn't expect from the Deno lsp.
      return Err(
        JsErrorBox::new(
          "UnsupportedFix",
          "The action returned from TypeScript is unsupported.",
        )
        .into(),
      );
    }
    let Some(action) = fix_ts_import_action(action, module, language_server)
    else {
      return Ok(());
    };
    let edit = ts_changes_to_edit(&action.changes, module, language_server)?;
    let code_action = lsp::CodeAction {
      title: action.description.clone(),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      edit,
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
    };
    self.actions.retain(|i| match i {
      CodeActionKind::Tsc(c, a) => {
        !(action.fix_name == a.fix_name && code_action.edit == c.edit)
      }
      _ => true,
    });
    self
      .actions
      .push(CodeActionKind::Tsc(code_action, action.as_ref().clone()));

    if let Some(fix_id) = &action.fix_id {
      if let Some(CodeActionKind::Tsc(existing_fix_all, existing_action)) =
        self.fix_all_actions.get(&FixAllKind::Tsc(fix_id.clone()))
      {
        self.actions.retain(|i| match i {
          CodeActionKind::Tsc(c, _) => c != existing_fix_all,
          _ => true,
        });
        self.actions.push(CodeActionKind::Tsc(
          existing_fix_all.clone(),
          existing_action.clone(),
        ));
      }
    }
    Ok(())
  }

  /// Add a TypeScript action to the actions as a "fix all" action, where it
  /// will fix all occurrences of the diagnostic in the file.
  pub fn add_ts_fix_all_action(
    &mut self,
    action: &tsc::CodeFixAction,
    module: &DocumentModule,
    diagnostic: &lsp::Diagnostic,
  ) {
    let data = action.fix_id.as_ref().map(|fix_id| {
      json!(CodeActionData {
        uri: module.uri.as_ref().clone(),
        fix_id: fix_id.clone(),
      })
    });
    let title = if let Some(description) = &action.fix_all_description {
      description.clone()
    } else {
      format!("{} (Fix all in file)", action.description)
    };

    let code_action = lsp::CodeAction {
      title,
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      edit: None,
      command: None,
      is_preferred: None,
      disabled: None,
      data,
    };
    if let Some(CodeActionKind::Tsc(existing, _)) = self
      .fix_all_actions
      .get(&FixAllKind::Tsc(action.fix_id.clone().unwrap()))
    {
      self.actions.retain(|i| match i {
        CodeActionKind::Tsc(c, _) => c != existing,
        _ => true,
      });
    }
    self
      .actions
      .push(CodeActionKind::Tsc(code_action.clone(), action.clone()));
    self.fix_all_actions.insert(
      FixAllKind::Tsc(action.fix_id.clone().unwrap()),
      CodeActionKind::Tsc(code_action, action.clone()),
    );
  }

  /// Move out the code actions and return them as a `CodeActionResponse`.
  pub fn get_response(self) -> lsp::CodeActionResponse {
    // Prefer Deno fixes first, then TSC fixes, then Deno lint fixes.
    let (deno, rest): (Vec<_>, Vec<_>) = self
      .actions
      .into_iter()
      .partition(|a| matches!(a, CodeActionKind::Deno(_)));
    let (tsc, deno_lint): (Vec<_>, Vec<_>) = rest
      .into_iter()
      .partition(|a| matches!(a, CodeActionKind::Tsc(..)));

    deno
      .into_iter()
      .chain(tsc)
      .chain(deno_lint)
      .map(|k| match k {
        CodeActionKind::Deno(c) => lsp::CodeActionOrCommand::CodeAction(c),
        CodeActionKind::DenoLint(c) => lsp::CodeActionOrCommand::CodeAction(c),
        CodeActionKind::Tsc(c, _) => lsp::CodeActionOrCommand::CodeAction(c),
      })
      .collect()
  }

  /// Determine if a action can be converted into a "fix all" action.
  pub fn is_fix_all_action(
    &self,
    action: &tsc::CodeFixAction,
    diagnostic: &lsp::Diagnostic,
    file_diagnostics: &[lsp::Diagnostic],
  ) -> bool {
    // If the action does not have a fix id (indicating it can be "bundled up")
    // or if the collection already contains a "bundled" action return false
    if action.fix_id.is_none()
      || self
        .fix_all_actions
        .contains_key(&FixAllKind::Tsc(action.fix_id.clone().unwrap()))
    {
      false
    } else {
      // else iterate over the diagnostic in the file and see if there are any
      // other diagnostics that could be bundled together in a "fix all" code
      // action
      file_diagnostics.iter().any(|d| {
        if d == diagnostic || d.code.is_none() || diagnostic.code.is_none() {
          false
        } else {
          d.code == diagnostic.code
            || is_equivalent_code(&d.code, &diagnostic.code)
        }
      })
    }
  }

  /// Set the `.is_preferred` flag on code actions, this should be only executed
  /// when all actions are added to the collection.
  pub fn set_preferred_fixes(&mut self) {
    let actions = self.actions.clone();
    for entry in self.actions.iter_mut() {
      if let CodeActionKind::Tsc(code_action, action) = entry {
        if action.fix_id.is_some() {
          continue;
        }
        if let Some((fix_priority, only_one)) =
          PREFERRED_FIXES.get(action.fix_name.as_str())
        {
          code_action.is_preferred =
            Some(is_preferred(action, &actions, *fix_priority, *only_one));
        }
      }
    }
  }

  pub fn add_cache_all_action(
    &mut self,
    specifier: &ModuleSpecifier,
    diagnostics: Vec<lsp::Diagnostic>,
  ) {
    self.actions.push(CodeActionKind::Deno(lsp::CodeAction {
      title: "Cache all dependencies of this module.".to_string(),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(diagnostics),
      command: Some(lsp::Command {
        title: "".to_string(),
        command: "deno.cache".to_string(),
        arguments: Some(vec![json!([]), json!(&specifier)]),
      }),
      ..Default::default()
    }));
  }
}

/// Prepend the whitespace characters found at the start of line_content to content.
fn prepend_whitespace(content: String, line_content: Option<String>) -> String {
  if let Some(line) = line_content {
    let whitespace_end = line
      .char_indices()
      .find_map(|(i, c)| (!c.is_whitespace()).then_some(i))
      .unwrap_or(0);
    let whitespace = &line[0..whitespace_end];
    format!("{}{}", &whitespace, content)
  } else {
    content
  }
}

pub fn source_range_to_lsp_range(
  range: &SourceRange,
  source_text_info: &SourceTextInfo,
) -> lsp::Range {
  let start = source_text_info.line_and_column_index(range.start);
  let end = source_text_info.line_and_column_index(range.end);
  lsp::Range {
    start: lsp::Position {
      line: start.line_index as u32,
      character: start.column_index as u32,
    },
    end: lsp::Position {
      line: end.line_index as u32,
      character: end.column_index as u32,
    },
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  #[test]
  fn test_reference_to_diagnostic() {
    let range = Range {
      start: Position {
        line: 1,
        character: 1,
      },
      end: Position {
        line: 2,
        character: 2,
      },
    };

    let test_cases = [
      (
        Reference {
          category: Category::Lint {
            message: "message1".to_string(),
            code: "code1".to_string(),
            hint: None,
            quick_fixes: Vec::new(),
          },
          range,
        },
        lsp::Diagnostic {
          range,
          severity: Some(lsp::DiagnosticSeverity::WARNING),
          code: Some(lsp::NumberOrString::String("code1".to_string())),
          source: Some("deno-lint".to_string()),
          message: "message1".to_string(),
          ..Default::default()
        },
      ),
      (
        Reference {
          category: Category::Lint {
            message: "message2".to_string(),
            code: "code2".to_string(),
            hint: Some("hint2".to_string()),
            quick_fixes: Vec::new(),
          },
          range,
        },
        lsp::Diagnostic {
          range,
          severity: Some(lsp::DiagnosticSeverity::WARNING),
          code: Some(lsp::NumberOrString::String("code2".to_string())),
          source: Some("deno-lint".to_string()),
          message: "message2\nhint2".to_string(),
          ..Default::default()
        },
      ),
    ];

    for (input, expected) in test_cases.iter() {
      let actual = input.to_diagnostic();
      assert_eq!(&actual, expected);
    }
  }

  #[test]
  fn test_try_reverse_map_package_json_exports() {
    let exports = json!({
      ".": {
        "types": "./src/index.d.ts",
        "browser": "./dist/module.js",
      },
      "./hooks": {
        "types": "./hooks/index.d.ts",
        "browser": "./dist/devtools.module.js",
      },
      "./utils": {
        "types": {
          "./sub_utils": "./utils_sub_utils.d.ts"
        }
      }
    });
    let exports = exports.as_object().unwrap();
    assert_eq!(
      try_reverse_map_package_json_exports(
        &PathBuf::from("/project/"),
        &PathBuf::from("/project/hooks/index.d.ts"),
        exports,
      )
      .unwrap(),
      "hooks"
    );
    assert_eq!(
      try_reverse_map_package_json_exports(
        &PathBuf::from("/project/"),
        &PathBuf::from("/project/dist/devtools.module.js"),
        exports,
      )
      .unwrap(),
      "hooks"
    );
    assert!(try_reverse_map_package_json_exports(
      &PathBuf::from("/project/"),
      &PathBuf::from("/project/src/index.d.ts"),
      exports,
    )
    .is_none());
    assert_eq!(
      try_reverse_map_package_json_exports(
        &PathBuf::from("/project/"),
        &PathBuf::from("/project/utils_sub_utils.d.ts"),
        exports,
      )
      .unwrap(),
      "utils/sub_utils"
    );
  }

  #[test]
  fn test_prepend_whitespace() {
    // Regression test for https://github.com/denoland/deno/issues/23361.
    assert_eq!(
      &prepend_whitespace("foo".to_string(), Some("\u{a0}bar".to_string())),
      "\u{a0}foo"
    );
  }
}
