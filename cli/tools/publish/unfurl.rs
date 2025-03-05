// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use deno_ast::diagnostics::Diagnostic;
use deno_ast::diagnostics::DiagnosticLevel;
use deno_ast::diagnostics::DiagnosticLocation;
use deno_ast::diagnostics::DiagnosticSnippet;
use deno_ast::diagnostics::DiagnosticSnippetHighlight;
use deno_ast::diagnostics::DiagnosticSnippetHighlightStyle;
use deno_ast::diagnostics::DiagnosticSourcePos;
use deno_ast::diagnostics::DiagnosticSourceRange;
use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_ast::SourceTextProvider;
use deno_ast::TextChange;
use deno_core::anyhow;
use deno_core::ModuleSpecifier;
use deno_graph::DependencyDescriptor;
use deno_graph::DynamicTemplatePart;
use deno_graph::StaticDependencyKind;
use deno_graph::TypeScriptReference;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepWorkspaceReq;
use deno_resolver::workspace::MappedResolution;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_semver::Version;
use deno_semver::VersionReq;
use sys_traits::FsMetadata;
use sys_traits::FsRead;

use crate::sys::CliSys;

#[derive(Debug, Clone)]
pub enum SpecifierUnfurlerDiagnostic {
  UnanalyzableDynamicImport {
    specifier: ModuleSpecifier,
    text_info: SourceTextInfo,
    range: SourceRange,
  },
  ResolvingNpmWorkspacePackage {
    specifier: ModuleSpecifier,
    package_name: String,
    text_info: SourceTextInfo,
    range: SourceRange,
    reason: String,
  },
  UnsupportedPkgJsonFileSpecifier {
    specifier: ModuleSpecifier,
    text_info: SourceTextInfo,
    range: SourceRange,
    package_name: String,
  },
}

impl Diagnostic for SpecifierUnfurlerDiagnostic {
  fn level(&self) -> DiagnosticLevel {
    match self {
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => {
        DiagnosticLevel::Warning
      }
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => {
        DiagnosticLevel::Error
      }
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier {
        ..
      } => DiagnosticLevel::Error,
    }
  }

  fn code(&self) -> Cow<'_, str> {
    match self {
      Self::UnanalyzableDynamicImport { .. } => "unanalyzable-dynamic-import",
      Self::ResolvingNpmWorkspacePackage { .. } => "npm-workspace-package",
      Self::UnsupportedPkgJsonFileSpecifier { .. } => {
        "unsupported-file-specifier"
      }
    }
    .into()
  }

  fn message(&self) -> Cow<'_, str> {
    match self {
      Self::UnanalyzableDynamicImport { .. } => {
        "unable to analyze dynamic import".into()
      }
      Self::ResolvingNpmWorkspacePackage {
        package_name,
        reason,
        ..
      } => format!(
        "failed resolving npm workspace package '{}': {}",
        package_name, reason
      )
      .into(),
      Self::UnsupportedPkgJsonFileSpecifier { package_name, .. } => format!(
        "unsupported package.json file specifier for '{}'",
        package_name
      )
      .into(),
    }
  }

  fn location(&self) -> deno_ast::diagnostics::DiagnosticLocation {
    match self {
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
        specifier,
        text_info,
        range,
      } => DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(specifier),
        text_info: Cow::Borrowed(text_info),
        source_pos: DiagnosticSourcePos::SourcePos(range.start),
      },
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage {
        specifier,
        text_info,
        range,
        ..
      } => DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(specifier),
        text_info: Cow::Borrowed(text_info),
        source_pos: DiagnosticSourcePos::SourcePos(range.start),
      },
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier {
        specifier,
        text_info,
        range,
        ..
      } => DiagnosticLocation::ModulePosition {
        specifier: Cow::Borrowed(specifier),
        text_info: Cow::Borrowed(text_info),
        source_pos: DiagnosticSourcePos::SourcePos(range.start),
      },
    }
  }

  fn snippet(&self) -> Option<deno_ast::diagnostics::DiagnosticSnippet<'_>> {
    match self {
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
        text_info,
        range,
        ..
      } => Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
        highlights: vec![DiagnosticSnippetHighlight {
          style: DiagnosticSnippetHighlightStyle::Warning,
          range: DiagnosticSourceRange {
            start: DiagnosticSourcePos::SourcePos(range.start),
            end: DiagnosticSourcePos::SourcePos(range.end),
          },
          description: Some("the unanalyzable dynamic import".into()),
        }],
      }),
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage {
        text_info,
        range,
        ..
      } => Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
        highlights: vec![DiagnosticSnippetHighlight {
          style: DiagnosticSnippetHighlightStyle::Warning,
          range: DiagnosticSourceRange {
            start: DiagnosticSourcePos::SourcePos(range.start),
            end: DiagnosticSourcePos::SourcePos(range.end),
          },
          description: Some("the unresolved import".into()),
        }],
      }),
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier {
        text_info,
        range,
        ..
      } => Some(DiagnosticSnippet {
        source: Cow::Borrowed(text_info),
        highlights: vec![DiagnosticSnippetHighlight {
          style: DiagnosticSnippetHighlightStyle::Warning,
          range: DiagnosticSourceRange {
            start: DiagnosticSourcePos::SourcePos(range.start),
            end: DiagnosticSourcePos::SourcePos(range.end),
          },
          description: Some("the import".into()),
        }],
      }),
    }
  }

  fn hint(&self) -> Option<Cow<'_, str>> {
    match self {
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => {
        None
      }
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => Some(
        "make sure the npm workspace package is resolvable and has a version field in its package.json".into()
      ),
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier { .. } => Some(
        "change the package dependency to point to something on npm instead".into()
      ),
    }
  }

  fn snippet_fixed(
    &self,
  ) -> Option<deno_ast::diagnostics::DiagnosticSnippet<'_>> {
    None
  }

  fn info(&self) -> Cow<'_, [Cow<'_, str>]> {
    match self {
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => Cow::Borrowed(&[
        Cow::Borrowed("after publishing this package, imports from the local import map / package.json do not work"),
        Cow::Borrowed("dynamic imports that can not be analyzed at publish time will not be rewritten automatically"),
        Cow::Borrowed("make sure the dynamic import is resolvable at runtime without an import map / package.json")
      ]),
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => {
        Cow::Borrowed(&[])
      },
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier { .. } => {
        Cow::Borrowed(&[])
      },
    }
  }

  fn docs_url(&self) -> Option<Cow<'_, str>> {
    None
  }
}

enum UnfurlSpecifierError {
  UnsupportedPkgJsonFileSpecifier {
    package_name: String,
  },
  Workspace {
    package_name: String,
    reason: String,
  },
}

pub struct SpecifierUnfurler<TSys: FsMetadata + FsRead = CliSys> {
  workspace_resolver: Arc<WorkspaceResolver<TSys>>,
  bare_node_builtins: bool,
}

impl<TSys: FsMetadata + FsRead> SpecifierUnfurler<TSys> {
  pub fn new(
    workspace_resolver: Arc<WorkspaceResolver<TSys>>,
    bare_node_builtins: bool,
  ) -> Self {
    debug_assert_eq!(
      workspace_resolver.pkg_json_dep_resolution(),
      PackageJsonDepResolution::Enabled
    );
    Self {
      workspace_resolver,
      bare_node_builtins,
    }
  }

  pub fn unfurl_specifier_reporting_diagnostic(
    &self,
    referrer: &ModuleSpecifier,
    specifier: &str,
    resolution_kind: deno_resolver::workspace::ResolutionKind,
    text_info: &SourceTextInfo,
    range: &deno_graph::PositionRange,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> Option<String> {
    match self.unfurl_specifier(referrer, specifier, resolution_kind) {
      Ok(maybe_unfurled) => maybe_unfurled,
      Err(diagnostic) => {
        let range = to_range(text_info, range);
        let range = SourceRange::new(
          text_info.start_pos() + range.start,
          text_info.start_pos() + range.end,
        );
        match diagnostic {
          UnfurlSpecifierError::UnsupportedPkgJsonFileSpecifier {
            package_name,
          } => {
            diagnostic_reporter(
              SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonFileSpecifier {
                specifier: referrer.clone(),
                package_name,
                text_info: text_info.clone(),
                range,
              },
            );
            None
          }
          UnfurlSpecifierError::Workspace {
            package_name,
            reason,
          } => {
            diagnostic_reporter(
              SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage {
                specifier: referrer.clone(),
                package_name,
                text_info: text_info.clone(),
                range,
                reason,
              },
            );
            None
          }
        }
      }
    }
  }

  fn unfurl_specifier(
    &self,
    referrer: &ModuleSpecifier,
    specifier: &str,
    resolution_kind: deno_resolver::workspace::ResolutionKind,
  ) -> Result<Option<String>, UnfurlSpecifierError> {
    let resolved = if let Ok(resolved) =
      self
        .workspace_resolver
        .resolve(specifier, referrer, resolution_kind)
    {
      match resolved {
        MappedResolution::Normal { specifier, .. } => Some(specifier),
        MappedResolution::WorkspaceJsrPackage { pkg_req_ref, .. } => {
          Some(ModuleSpecifier::parse(&pkg_req_ref.to_string()).unwrap())
        }
        MappedResolution::WorkspaceNpmPackage {
          target_pkg_json: pkg_json,
          pkg_name,
          sub_path,
        } => {
          // todo(#24612): consider warning or error when this is also a jsr package?
          ModuleSpecifier::parse(&format!(
            "npm:{}{}{}",
            pkg_name,
            pkg_json
              .version
              .as_ref()
              .map(|v| format!("@^{}", v))
              .unwrap_or_default(),
            sub_path
              .as_ref()
              .map(|s| format!("/{}", s))
              .unwrap_or_default()
          ))
          .ok()
        }
        MappedResolution::PackageJson {
          alias,
          sub_path,
          dep_result,
          ..
        } => match dep_result {
          Ok(dep) => match dep {
            PackageJsonDepValue::File(_) => {
              return Err(
                UnfurlSpecifierError::UnsupportedPkgJsonFileSpecifier {
                  package_name: alias.to_string(),
                },
              );
            }
            PackageJsonDepValue::Req(pkg_req) => {
              // todo(#24612): consider warning or error when this is an npm workspace
              // member that's also a jsr package?
              ModuleSpecifier::parse(&format!(
                "npm:{}{}",
                pkg_req,
                sub_path
                  .as_ref()
                  .map(|s| format!("/{}", s))
                  .unwrap_or_default()
              ))
              .ok()
            }
            PackageJsonDepValue::Workspace(workspace_version_req) => {
              let version_req = match workspace_version_req {
                PackageJsonDepWorkspaceReq::VersionReq(version_req) => {
                  Cow::Borrowed(version_req)
                }
                PackageJsonDepWorkspaceReq::Caret => {
                  let version = self
                    .find_workspace_npm_dep_version(alias)
                    .map_err(|err| UnfurlSpecifierError::Workspace {
                      package_name: alias.to_string(),
                      reason: err.to_string(),
                    })?;
                  // version was validated, so ok to unwrap
                  Cow::Owned(
                    VersionReq::parse_from_npm(&format!("^{}", version))
                      .unwrap(),
                  )
                }
                PackageJsonDepWorkspaceReq::Tilde => {
                  let version = self
                    .find_workspace_npm_dep_version(alias)
                    .map_err(|err| UnfurlSpecifierError::Workspace {
                      package_name: alias.to_string(),
                      reason: err.to_string(),
                    })?;
                  // version was validated, so ok to unwrap
                  Cow::Owned(
                    VersionReq::parse_from_npm(&format!("~{}", version))
                      .unwrap(),
                  )
                }
              };
              // todo(#24612): warn when this is also a jsr package telling
              // people to map the specifiers in the import map
              ModuleSpecifier::parse(&format!(
                "npm:{}@{}{}",
                alias,
                version_req,
                sub_path
                  .as_ref()
                  .map(|s| format!("/{}", s))
                  .unwrap_or_default()
              ))
              .ok()
            }
          },
          Err(err) => {
            log::warn!(
              "Ignoring failed to resolve package.json dependency. {:#}",
              err
            );
            None
          }
        },
      }
    } else {
      None
    };
    let resolved = match resolved {
      Some(resolved) => resolved,
      None if self.bare_node_builtins && is_builtin_node_module(specifier) => {
        format!("node:{specifier}").parse().unwrap()
      }
      None => match ModuleSpecifier::options()
        .base_url(Some(referrer))
        .parse(specifier)
        .ok()
      {
        Some(value) => value,
        None => return Ok(None),
      },
    };
    // TODO(lucacasonato): this requires integration in deno_graph first
    // let resolved = if let Ok(specifier) =
    //   NpmPackageReqReference::from_specifier(&resolved)
    // {
    //   if let Some(scope_name) = specifier.req().name.strip_prefix("@jsr/") {
    //     let (scope, name) = scope_name.split_once("__")?;
    //     let new_specifier = JsrPackageReqReference::new(PackageReqReference {
    //       req: PackageReq {
    //         name: format!("@{scope}/{name}"),
    //         version_req: specifier.req().version_req.clone(),
    //       },
    //       sub_path: specifier.sub_path().map(ToOwned::to_owned),
    //     })
    //     .to_string();
    //     ModuleSpecifier::parse(&new_specifier).unwrap()
    //   } else {
    //     resolved
    //   }
    // } else {
    //   resolved
    // };
    let relative_resolved = relative_url(&resolved, referrer);
    if relative_resolved == specifier {
      Ok(None) // nothing to unfurl
    } else {
      log::debug!(
        "Unfurled specifier: {} from {} -> {}",
        specifier,
        referrer,
        relative_resolved
      );
      Ok(Some(relative_resolved))
    }
  }

  fn find_workspace_npm_dep_version(
    &self,
    pkg_name: &str,
  ) -> Result<Version, anyhow::Error> {
    // todo(#24612): warn when this is also a jsr package telling
    // people to map the specifiers in the import map
    let pkg_json = self
      .workspace_resolver
      .package_jsons()
      .find(|pkg| pkg.name.as_deref() == Some(pkg_name))
      .ok_or_else(|| {
        anyhow::anyhow!("unable to find npm package in workspace")
      })?;
    if let Some(version) = &pkg_json.version {
      Ok(Version::parse_from_npm(version)?)
    } else {
      Err(anyhow::anyhow!(
        "missing version in package.json of npm package",
      ))
    }
  }

  /// Attempts to unfurl the dynamic dependency returning `true` on success
  /// or `false` when the import was not analyzable.
  fn try_unfurl_dynamic_dep(
    &self,
    module_url: &ModuleSpecifier,
    text_info: &SourceTextInfo,
    dep: &deno_graph::DynamicDependencyDescriptor,
    text_changes: &mut Vec<deno_ast::TextChange>,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> bool {
    match &dep.argument {
      deno_graph::DynamicArgument::String(specifier) => {
        let range = to_range(text_info, &dep.argument_range);
        let maybe_relative_index =
          text_info.text_str()[range.start..range.end].find(specifier);
        let Some(relative_index) = maybe_relative_index else {
          return true; // always say it's analyzable for a string
        };
        let maybe_unfurled = self.unfurl_specifier_reporting_diagnostic(
          module_url,
          specifier,
          deno_resolver::workspace::ResolutionKind::Execution, // dynamic imports are always execution
          text_info,
          &dep.argument_range,
          diagnostic_reporter,
        );
        if let Some(unfurled) = maybe_unfurled {
          let start = range.start + relative_index;
          text_changes.push(deno_ast::TextChange {
            range: start..start + specifier.len(),
            new_text: unfurled,
          });
        }
        true
      }
      deno_graph::DynamicArgument::Template(parts) => match parts.first() {
        Some(DynamicTemplatePart::String { value: specifier }) => {
          // relative doesn't need to be modified
          let is_relative =
            specifier.starts_with("./") || specifier.starts_with("../");
          if is_relative {
            return true;
          }
          if !specifier.ends_with('/') {
            return false;
          }
          let unfurled = self.unfurl_specifier_reporting_diagnostic(
            module_url,
            specifier,
            deno_resolver::workspace::ResolutionKind::Execution, // dynamic imports are always execution
            text_info,
            &dep.argument_range,
            diagnostic_reporter,
          );
          let Some(unfurled) = unfurled else {
            return true; // nothing to unfurl
          };
          let range = to_range(text_info, &dep.argument_range);
          let maybe_relative_index =
            text_info.text_str()[range.start..].find(specifier);
          let Some(relative_index) = maybe_relative_index else {
            return false;
          };
          let start = range.start + relative_index;
          text_changes.push(deno_ast::TextChange {
            range: start..start + specifier.len(),
            new_text: unfurled,
          });
          true
        }
        Some(DynamicTemplatePart::Expr) => {
          false // failed analyzing
        }
        None => {
          true // ignore
        }
      },
      deno_graph::DynamicArgument::Expr => {
        false // failed analyzing
      }
    }
  }

  pub fn unfurl_to_changes(
    &self,
    url: &ModuleSpecifier,
    parsed_source: &ParsedSource,
    module_info: &deno_graph::ModuleInfo,
    text_changes: &mut Vec<TextChange>,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) {
    let text_info = parsed_source.text_info_lazy();
    let analyze_specifier =
      |specifier: &str,
       range: &deno_graph::PositionRange,
       resolution_kind: deno_resolver::workspace::ResolutionKind,
       text_changes: &mut Vec<deno_ast::TextChange>,
       diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic)| {
        if let Some(unfurled) = self.unfurl_specifier_reporting_diagnostic(
          url,
          specifier,
          resolution_kind,
          text_info,
          range,
          diagnostic_reporter,
        ) {
          text_changes.push(deno_ast::TextChange {
            range: to_range(text_info, range),
            new_text: unfurled,
          });
        }
      };
    for dep in &module_info.dependencies {
      match dep {
        DependencyDescriptor::Static(dep) => {
          let resolution_kind = if parsed_source.media_type().is_declaration() {
            deno_resolver::workspace::ResolutionKind::Types
          } else {
            match dep.kind {
              StaticDependencyKind::Export
              | StaticDependencyKind::Import
              | StaticDependencyKind::ExportEquals
              | StaticDependencyKind::ImportEquals => {
                deno_resolver::workspace::ResolutionKind::Execution
              }
              StaticDependencyKind::ExportType
              | StaticDependencyKind::ImportType => {
                deno_resolver::workspace::ResolutionKind::Types
              }
            }
          };

          analyze_specifier(
            &dep.specifier,
            &dep.specifier_range,
            resolution_kind,
            text_changes,
            diagnostic_reporter,
          );
        }
        DependencyDescriptor::Dynamic(dep) => {
          let success = self.try_unfurl_dynamic_dep(
            url,
            text_info,
            dep,
            text_changes,
            diagnostic_reporter,
          );

          if !success {
            let start_pos = text_info.line_start(dep.argument_range.start.line)
              + dep.argument_range.start.character;
            let end_pos = text_info.line_start(dep.argument_range.end.line)
              + dep.argument_range.end.character;
            diagnostic_reporter(
              SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport {
                specifier: url.to_owned(),
                range: SourceRange::new(start_pos, end_pos),
                text_info: text_info.clone(),
              },
            );
          }
        }
      }
    }
    for ts_ref in &module_info.ts_references {
      let specifier_with_range = match ts_ref {
        TypeScriptReference::Path(s) => s,
        TypeScriptReference::Types { specifier, .. } => specifier,
      };
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }
    for jsdoc in &module_info.jsdoc_imports {
      analyze_specifier(
        &jsdoc.specifier.text,
        &jsdoc.specifier.range,
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        deno_resolver::workspace::ResolutionKind::Execution,
        text_changes,
        diagnostic_reporter,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source_types {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }
  }
}

fn relative_url(
  resolved: &ModuleSpecifier,
  referrer: &ModuleSpecifier,
) -> String {
  if resolved.scheme() == "file" {
    let relative = referrer.make_relative(resolved).unwrap();
    if relative.is_empty() {
      let last = resolved.path_segments().unwrap().last().unwrap();
      format!("./{last}")
    } else if relative.starts_with("../") {
      relative
    } else {
      format!("./{relative}")
    }
  } else {
    resolved.to_string()
  }
}

fn to_range(
  text_info: &SourceTextInfo,
  range: &deno_graph::PositionRange,
) -> std::ops::Range<usize> {
  let mut range = range
    .as_source_range(text_info)
    .as_byte_range(text_info.range().start);
  let text = &text_info.text_str()[range.clone()];
  if text.starts_with('"') || text.starts_with('\'') {
    range.start += 1;
  }
  if text.ends_with('"') || text.ends_with('\'') {
    range.end -= 1;
  }
  range
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_config::workspace::ResolverWorkspaceJsrPackage;
  use deno_core::serde_json::json;
  use deno_core::url::Url;
  use deno_graph::ParserModuleAnalyzer;
  use deno_resolver::workspace::SloppyImportsOptions;
  use deno_runtime::deno_node::PackageJson;
  use deno_semver::Version;
  use import_map::ImportMapWithDiagnostics;
  use indexmap::IndexMap;
  use pretty_assertions::assert_eq;
  use test_util::testdata_path;

  use super::*;
  use crate::sys::CliSys;

  fn parse_ast(specifier: &Url, source_code: &str) -> ParsedSource {
    let media_type = MediaType::from_specifier(specifier);
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      media_type,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
      text: source_code.into(),
    })
    .unwrap()
  }

  #[test]
  fn test_unfurling() {
    let cwd = testdata_path().join("unfurl").to_path_buf();

    let deno_json_url =
      ModuleSpecifier::from_file_path(cwd.join("deno.json")).unwrap();
    let value = json!({
      "imports": {
        "express": "npm:express@5",
        "lib/": "./lib/",
        "fizz": "./fizz/mod.ts",
        "@std/fs": "npm:@jsr/std__fs@1",
      }
    });
    let ImportMapWithDiagnostics { import_map, .. } =
      import_map::parse_from_value(deno_json_url, value).unwrap();
    let package_json = PackageJson::load_from_value(
      cwd.join("package.json"),
      json!({
        "dependencies": {
          "chalk": 5
        }
      }),
    )
    .unwrap();
    let workspace_resolver = WorkspaceResolver::new_raw(
      Arc::new(ModuleSpecifier::from_directory_path(&cwd).unwrap()),
      Some(import_map),
      vec![ResolverWorkspaceJsrPackage {
        is_patch: false,
        base: ModuleSpecifier::from_directory_path(cwd.join("jsr-package"))
          .unwrap(),
        name: "@denotest/example".to_string(),
        version: Some(Version::parse_standard("1.0.0").unwrap()),
        exports: IndexMap::from([(".".to_string(), "mod.ts".to_string())]),
      }],
      vec![Arc::new(package_json)],
      deno_resolver::workspace::PackageJsonDepResolution::Enabled,
      SloppyImportsOptions::Enabled,
      Default::default(),
      Default::default(),
      Default::default(),
      CliSys::default(),
    );
    let unfurler = SpecifierUnfurler::new(Arc::new(workspace_resolver), true);

    // Unfurling TS file should apply changes.
    {
      let source_code = r#"import express from "express";"
import foo from "lib/foo.ts";
import bar from "lib/bar.ts";
import fizz from "fizz";
import chalk from "chalk";
import baz from "./baz";
import b from "./b.js";
import b2 from "./b";
import "./mod.ts";
import { } from "./c";
import type { } from "./c";
import url from "url";
import "@denotest/example";
// TODO: unfurl these to jsr
// import "npm:@jsr/std__fs@1/file";
// import "npm:@jsr/std__fs@1";
// import "npm:@jsr/std__fs";
// import "@std/fs";

const test1 = await import("lib/foo.ts");
const test2 = await import(`lib/foo.ts`);
const test3 = await import(`lib/${expr}`);
const test4 = await import(`./lib/${expr}`);
const test5 = await import("./lib/something.ts");
const test6 = await import(`./lib/something.ts`);
// will warn
const warn1 = await import(`lib${expr}`);
const warn2 = await import(`${expr}`);
"#;
      let specifier =
        ModuleSpecifier::from_file_path(cwd.join("mod.ts")).unwrap();
      let source = parse_ast(&specifier, source_code);
      let mut d = Vec::new();
      let mut reporter = |diagnostic| d.push(diagnostic);
      let unfurled_source =
        unfurl(&unfurler, &specifier, &source, &mut reporter);
      assert_eq!(d.len(), 2);
      assert!(
        matches!(
          d[0],
          SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. }
        ),
        "{:?}",
        d[0]
      );
      assert!(
        matches!(
          d[1],
          SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. }
        ),
        "{:?}",
        d[1]
      );
      let expected_source = r#"import express from "npm:express@5";"
import foo from "./lib/foo.ts";
import bar from "./lib/bar.ts";
import fizz from "./fizz/mod.ts";
import chalk from "npm:chalk@5";
import baz from "./baz/index.js";
import b from "./b.ts";
import b2 from "./b.ts";
import "./mod.ts";
import { } from "./c.js";
import type { } from "./c.d.ts";
import url from "node:url";
import "jsr:@denotest/example@^1.0.0";
// TODO: unfurl these to jsr
// import "npm:@jsr/std__fs@1/file";
// import "npm:@jsr/std__fs@1";
// import "npm:@jsr/std__fs";
// import "@std/fs";

const test1 = await import("./lib/foo.ts");
const test2 = await import(`./lib/foo.ts`);
const test3 = await import(`./lib/${expr}`);
const test4 = await import(`./lib/${expr}`);
const test5 = await import("./lib/something.ts");
const test6 = await import(`./lib/something.ts`);
// will warn
const warn1 = await import(`lib${expr}`);
const warn2 = await import(`${expr}`);
"#;
      assert_eq!(unfurled_source, expected_source);
    }

    // Unfurling .d.ts file should use types resolution.
    {
      let source_code = r#"import express from "express";"
export type * from "./c";
"#;
      let specifier =
        ModuleSpecifier::from_file_path(cwd.join("mod.d.ts")).unwrap();
      let source = parse_ast(&specifier, source_code);
      let mut d = Vec::new();
      let mut reporter = |diagnostic| d.push(diagnostic);
      let unfurled_source =
        unfurl(&unfurler, &specifier, &source, &mut reporter);
      assert_eq!(d.len(), 0);
      let expected_source = r#"import express from "npm:express@5";"
export type * from "./c.d.ts";
"#;
      assert_eq!(unfurled_source, expected_source);
    }
  }

  #[test]
  fn test_unfurling_npm_dep_workspace_specifier() {
    let cwd = testdata_path().join("unfurl").to_path_buf();

    let pkg_json_add = PackageJson::load_from_value(
      cwd.join("add/package.json"),
      json!({ "name": "add", "version": "0.1.0", }),
    )
    .unwrap();
    let pkg_json_subtract = PackageJson::load_from_value(
      cwd.join("subtract/package.json"),
      json!({ "name": "subtract", "version": "0.2.0", }),
    )
    .unwrap();
    let pkg_json_publishing = PackageJson::load_from_value(
      cwd.join("publish/package.json"),
      json!({
        "name": "@denotest/main",
        "version": "1.0.0",
        "dependencies": {
          "add": "workspace:~",
          "subtract": "workspace:^",
          "non-existent": "workspace:~",
        }
      }),
    )
    .unwrap();
    let root_pkg_json = PackageJson::load_from_value(
      cwd.join("package.json"),
      json!({ "workspaces": ["./publish", "./subtract", "./add"] }),
    )
    .unwrap();
    let sys = CliSys::default();
    let workspace_resolver = WorkspaceResolver::new_raw(
      Arc::new(ModuleSpecifier::from_directory_path(&cwd).unwrap()),
      None,
      vec![ResolverWorkspaceJsrPackage {
        is_patch: false,
        base: ModuleSpecifier::from_directory_path(
          cwd.join("publish/jsr.json"),
        )
        .unwrap(),
        name: "@denotest/main".to_string(),
        version: Some(Version::parse_standard("1.0.0").unwrap()),
        exports: IndexMap::from([(".".to_string(), "mod.ts".to_string())]),
      }],
      vec![
        Arc::new(root_pkg_json),
        Arc::new(pkg_json_add),
        Arc::new(pkg_json_subtract),
        Arc::new(pkg_json_publishing),
      ],
      deno_resolver::workspace::PackageJsonDepResolution::Enabled,
      Default::default(),
      Default::default(),
      Default::default(),
      Default::default(),
      sys.clone(),
    );
    let unfurler = SpecifierUnfurler::new(Arc::new(workspace_resolver), true);

    {
      let source_code = r#"import add from "add";
import subtract from "subtract";

console.log(add, subtract);
"#;
      let specifier =
        ModuleSpecifier::from_file_path(cwd.join("publish").join("mod.ts"))
          .unwrap();
      let source = parse_ast(&specifier, source_code);
      let mut d = Vec::new();
      let mut reporter = |diagnostic| d.push(diagnostic);
      let unfurled_source =
        unfurl(&unfurler, &specifier, &source, &mut reporter);
      assert_eq!(d.len(), 0);
      // it will inline the version
      let expected_source = r#"import add from "npm:add@~0.1.0";
import subtract from "npm:subtract@^0.2.0";

console.log(add, subtract);
"#;
      assert_eq!(unfurled_source, expected_source);
    }

    {
      let source_code = r#"import nonExistent from "non-existent";
console.log(nonExistent);
"#;
      let specifier =
        ModuleSpecifier::from_file_path(cwd.join("publish").join("other.ts"))
          .unwrap();
      let source = parse_ast(&specifier, source_code);
      let mut d = Vec::new();
      let mut reporter = |diagnostic| d.push(diagnostic);
      let unfurled_source =
        unfurl(&unfurler, &specifier, &source, &mut reporter);
      assert_eq!(d.len(), 1);
      match &d[0] {
        SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage {
          package_name,
          reason,
          ..
        } => {
          assert_eq!(package_name, "non-existent");
          assert_eq!(reason, "unable to find npm package in workspace");
        }
        _ => unreachable!(),
      }
      // won't make any changes, but the above will be a fatal error
      assert!(matches!(d[0].level(), DiagnosticLevel::Error));
      assert_eq!(unfurled_source, source_code);
    }
  }

  fn unfurl(
    unfurler: &SpecifierUnfurler,
    url: &ModuleSpecifier,
    parsed_source: &ParsedSource,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> String {
    let text_info = parsed_source.text_info_lazy();
    let mut text_changes = Vec::new();
    let module_info = ParserModuleAnalyzer::module_info(parsed_source);
    unfurler.unfurl_to_changes(
      url,
      parsed_source,
      &module_info,
      &mut text_changes,
      diagnostic_reporter,
    );

    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);
    rewritten_text
  }
}
