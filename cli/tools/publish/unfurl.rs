// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::ParsedSource;
use deno_ast::SourcePos;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_ast::SourceTextProvider;
use deno_ast::TextChange;
use deno_ast::diagnostics::Diagnostic;
use deno_ast::diagnostics::DiagnosticLevel;
use deno_ast::diagnostics::DiagnosticLocation;
use deno_ast::diagnostics::DiagnosticSnippet;
use deno_ast::diagnostics::DiagnosticSnippetHighlight;
use deno_ast::diagnostics::DiagnosticSnippetHighlightStyle;
use deno_ast::diagnostics::DiagnosticSourcePos;
use deno_ast::diagnostics::DiagnosticSourceRange;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::MetaPropKind;
use deno_ast::swc::atoms::Atom;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::ecma_visit::noop_visit_type;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::ModuleSpecifier;
use deno_core::anyhow;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::DynamicTemplatePart;
use deno_graph::analysis::StaticDependencyKind;
use deno_graph::analysis::TypeScriptReference;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseErrorKind;
use deno_package_json::PackageJsonDepWorkspaceReq;
use deno_resolver::npm::NpmResolverSys;
use deno_resolver::workspace::MappedResolution;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::NodeResolverSys;

use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::resolver::CliNpmReqResolver;
use crate::sys::CliSys;

#[derive(Debug, Clone)]
pub enum SpecifierUnfurlerDiagnostic {
  UnanalyzableDynamicImport {
    specifier: ModuleSpecifier,
    text_info: SourceTextInfo,
    range: SourceRange,
  },
  UnanalyzableImportMetaResolve {
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
  UnsupportedPkgJsonSpecifier {
    specifier: ModuleSpecifier,
    scheme: String,
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
      SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve { .. } => {
        DiagnosticLevel::Warning
      }
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => {
        DiagnosticLevel::Error
      }
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier { .. } => {
        DiagnosticLevel::Error
      }
    }
  }

  fn code(&self) -> Cow<'_, str> {
    match self {
      Self::UnanalyzableDynamicImport { .. } => "unanalyzable-dynamic-import",
      Self::UnanalyzableImportMetaResolve { .. } => {
        "unanalyzable-import-meta-resolve"
      }
      Self::ResolvingNpmWorkspacePackage { .. } => "npm-workspace-package",
      Self::UnsupportedPkgJsonSpecifier { .. } => "unsupported-specifier",
    }
    .into()
  }

  fn message(&self) -> Cow<'_, str> {
    match self {
      Self::UnanalyzableDynamicImport { .. } => {
        "unable to analyze dynamic import".into()
      }
      Self::UnanalyzableImportMetaResolve { .. } => {
        "unable to analyze import.meta.resolve".into()
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
      Self::UnsupportedPkgJsonSpecifier {
        package_name,
        scheme,
        ..
      } => format!(
        "unsupported package.json '{}:' specifier for '{}'",
        scheme, package_name
      )
      .into(),
    }
  }

  fn location(&self) -> deno_ast::diagnostics::DiagnosticLocation<'_> {
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
      SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve {
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
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier {
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
      SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve {
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
          description: Some("the unanalyzable import.meta.resolve call".into()),
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
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier {
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
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } | SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve { .. } => {
        None
      }
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => Some(
        "make sure the npm workspace package is resolvable and has a version field in its package.json".into()
      ),
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier { .. } => Some(
        "change the package dependency to point to something on npm or JSR instead".into()
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
      SpecifierUnfurlerDiagnostic::UnanalyzableDynamicImport { .. } => {
        Cow::Borrowed(&[
          Cow::Borrowed(
            "after publishing this package, imports from the local import map / package.json do not work",
          ),
          Cow::Borrowed(
            "dynamic imports that can not be analyzed at publish time will not be rewritten automatically",
          ),
          Cow::Borrowed(
            "make sure the dynamic import is resolvable at runtime without an import map / package.json",
          ),
        ])
      }
      SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve { .. } => {
        Cow::Borrowed(&[
          Cow::Borrowed(
            "after publishing this package, import.meta.resolve calls from the local import map / package.json do not work",
          ),
          Cow::Borrowed(
            "import.meta.resolve calls that can not be analyzed at publish time will not be rewritten automatically",
          ),
          Cow::Borrowed(
            "make sure the import.meta.resolve call is resolvable at runtime without an import map / package.json",
          ),
        ])
      }
      SpecifierUnfurlerDiagnostic::ResolvingNpmWorkspacePackage { .. } => {
        Cow::Borrowed(&[])
      }
      SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier { .. } => {
        Cow::Borrowed(&[])
      }
    }
  }

  fn docs_url(&self) -> Option<Cow<'_, str>> {
    None
  }
}

enum UnfurlSpecifierError {
  UnsupportedPkgJsonSpecifier {
    scheme: String,
    package_name: String,
  },
  Workspace {
    package_name: String,
    reason: String,
  },
}

#[derive(Copy, Clone)]
pub enum PositionOrSourceRangeRef<'a> {
  PositionRange(&'a deno_graph::PositionRange),
  SourceRange(SourceRange<SourcePos>),
}

#[sys_traits::auto_impl]
pub trait SpecifierUnfurlerSys: NodeResolverSys + NpmResolverSys {}

pub struct SpecifierUnfurler<TSys: SpecifierUnfurlerSys = CliSys> {
  node_resolver: Arc<CliNodeResolver<TSys>>,
  npm_req_resolver: Arc<CliNpmReqResolver<TSys>>,
  pkg_json_resolver: Arc<CliPackageJsonResolver<TSys>>,
  workspace_dir: Arc<WorkspaceDirectory>,
  workspace_resolver: Arc<WorkspaceResolver<TSys>>,
  bare_node_builtins: bool,
}

impl<TSys: SpecifierUnfurlerSys> SpecifierUnfurler<TSys> {
  pub fn new(
    node_resolver: Arc<CliNodeResolver<TSys>>,
    npm_req_resolver: Arc<CliNpmReqResolver<TSys>>,
    pkg_json_resolver: Arc<CliPackageJsonResolver<TSys>>,
    workspace_dir: Arc<WorkspaceDirectory>,
    workspace_resolver: Arc<WorkspaceResolver<TSys>>,
    bare_node_builtins: bool,
  ) -> Self {
    debug_assert_eq!(
      workspace_resolver.pkg_json_dep_resolution(),
      PackageJsonDepResolution::Enabled
    );
    Self {
      node_resolver,
      npm_req_resolver,
      pkg_json_resolver,
      workspace_dir,
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
    range: PositionOrSourceRangeRef<'_>,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> Option<String> {
    match self.unfurl_specifier(referrer, specifier, resolution_kind) {
      Ok(maybe_unfurled) => maybe_unfurled,
      Err(diagnostic) => {
        let range = match range {
          PositionOrSourceRangeRef::PositionRange(position_range) => {
            let range = to_range(text_info, position_range);
            SourceRange::new(
              text_info.start_pos() + range.start,
              text_info.start_pos() + range.end,
            )
          }
          PositionOrSourceRangeRef::SourceRange(source_range) => source_range,
        };
        match diagnostic {
          UnfurlSpecifierError::UnsupportedPkgJsonSpecifier {
            package_name,
            scheme,
          } => {
            diagnostic_reporter(
              SpecifierUnfurlerDiagnostic::UnsupportedPkgJsonSpecifier {
                specifier: referrer.clone(),
                package_name,
                scheme,
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
    let resolved = match self.workspace_resolver.resolve(
      specifier,
      referrer,
      resolution_kind,
    ) {
      Ok(resolved) => {
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
                  UnfurlSpecifierError::UnsupportedPkgJsonSpecifier {
                    package_name: alias.to_string(),
                    scheme: "file".to_string(),
                  },
                );
              }
              PackageJsonDepValue::Req(pkg_req) => {
                // todo(#24612): consider warning or error when this is an npm workspace
                // member that's also a jsr package?
                if let Some((scope, name)) = pkg_req
                  .name
                  .strip_prefix("@jsr/")
                  .and_then(|name| name.split_once("__"))
                {
                  ModuleSpecifier::parse(&format!(
                    "jsr:@{}/{}@{}{}",
                    scope,
                    name,
                    pkg_req.version_req,
                    sub_path
                      .as_ref()
                      .map(|s| format!("/{}", s))
                      .unwrap_or_default()
                  ))
                  .ok()
                } else {
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
            Err(err) => match err.as_kind() {
              PackageJsonDepValueParseErrorKind::Unsupported { scheme } => {
                return Err(
                  UnfurlSpecifierError::UnsupportedPkgJsonSpecifier {
                    package_name: alias.to_string(),
                    scheme: scheme.to_string(),
                  },
                );
              }
              PackageJsonDepValueParseErrorKind::VersionReq { .. }
              | PackageJsonDepValueParseErrorKind::JsrRequiresScope {
                ..
              } => {
                log::warn!(
                  "Ignoring failed to resolve package.json dependency. {:#}",
                  err
                );
                None
              }
            },
          },
          MappedResolution::PackageJsonImport { pkg_json } => self
            .node_resolver
            .resolve_package_import(
              specifier,
              Some(&node_resolver::UrlOrPathRef::from_url(referrer)),
              Some(pkg_json),
              node_resolver::ResolutionMode::Import,
              node_resolver::NodeResolutionKind::Execution,
            )
            .ok()
            .and_then(|s| s.into_url().ok()),
        }
      }
      Err(_) => None,
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

  /// Look up the version constraint for a @types/* package from package.json
  /// dependencies (including devDependencies) or from the import map.
  fn find_types_package_version_req(
    &self,
    types_package_name: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<VersionReq> {
    // check package.json dependencies
    let referrer_path = deno_path_util::url_to_file_path(referrer).ok();
    let referrer_pkg_jsons = referrer_path
      .as_ref()
      .map(|path| self.pkg_json_resolver.get_closest_package_jsons(path))
      .into_iter()
      .flatten()
      .filter_map(|i| i.ok());
    for pkg_json in referrer_pkg_jsons
      .chain(self.workspace_dir.workspace.package_jsons().cloned())
    {
      let deps = pkg_json.resolve_local_package_json_deps();
      if let Some(Ok(PackageJsonDepValue::Req(pkg_req))) =
        deps.get(types_package_name)
      {
        return Some(pkg_req.version_req.clone());
      }
    }

    let check_dep = |dep: JsrDepPackageReq| {
      if dep.kind == deno_semver::package::PackageKind::Npm
        && dep.req.name == types_package_name
      {
        Some(dep.req.version_req)
      } else {
        None
      }
    };

    // now look in the member and root deno json
    let deno_jsons = [
      self.workspace_dir.member_deno_json(),
      self.workspace_dir.workspace.root_deno_json(),
    ];
    for deno_json in deno_jsons.iter().flatten() {
      let deps = deno_json
        .dependencies()
        .into_iter()
        .collect::<BTreeSet<_>>();
      for dep in deps {
        if let Some(version_req) = check_dep(dep) {
          return Some(version_req);
        }
      }
    }

    // check the import map for the types package
    if let Some(import_map) = self.workspace_resolver.maybe_import_map() {
      let deps = deno_config::import_map::import_map_deps(import_map);
      for dep in deps {
        if let Some(version_req) = check_dep(dep) {
          return Some(version_req);
        }
      }
    }

    None
  }

  /// Check if a resolved npm specifier (like "npm:express@^4" or
  /// "npm:express@^4/subpath") has types coming from a separate @types/*
  /// package. If so, return the types package specifier to use for @ts-types
  /// comment.
  fn get_types_package_specifier(
    &self,
    unfurled_specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<String> {
    let npm_req_ref =
      NpmPackageReqReference::from_str(unfurled_specifier).ok()?;
    if npm_req_ref.req().name.starts_with("@types/") {
      return None;
    }
    let types_resolution = self.npm_req_resolver.resolve_req_reference(
      &npm_req_ref,
      referrer,
      node_resolver::ResolutionMode::Import,
      node_resolver::NodeResolutionKind::Types,
    );
    let resolved_path = types_resolution.ok()?.into_path().ok()?;
    let types_pkg_json = self
      .pkg_json_resolver
      .get_closest_package_jsons(&resolved_path)
      .filter_map(|pkg_json| pkg_json.ok())
      .find(|p| p.name.is_some())?;
    let types_pkg_name = types_pkg_json.name.as_ref()?;
    let types_pkg_version = types_pkg_json
      .version
      .as_ref()
      .and_then(|v| Version::parse_from_npm(v).ok());

    if !types_pkg_name.starts_with("@types/") {
      return None;
    }

    // determine version constraint
    let version_req = if let Some(req) =
      self.find_types_package_version_req(types_pkg_name, referrer)
    {
      req.to_string()
    } else {
      // fall back to using the package's version
      types_pkg_version
        .map(|v| format!("^{}", v))
        .unwrap_or_else(|| "*".to_string())
    };

    // construct the types specifier with subpath if present
    Some(format!(
      "npm:{}@{}{}",
      types_pkg_name,
      version_req,
      npm_req_ref
        .sub_path()
        .map(|path| format!("/{}", path))
        .unwrap_or_default()
    ))
  }

  /// Attempts to unfurl the dynamic dependency returning `true` on success
  /// or `false` when the import was not analyzable.
  fn try_unfurl_dynamic_dep(
    &self,
    module_url: &ModuleSpecifier,
    text_info: &SourceTextInfo,
    dep: &deno_graph::analysis::DynamicDependencyDescriptor,
    text_changes: &mut Vec<deno_ast::TextChange>,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> bool {
    match &dep.argument {
      deno_graph::analysis::DynamicArgument::String(specifier) => {
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
          PositionOrSourceRangeRef::PositionRange(&dep.argument_range),
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
      deno_graph::analysis::DynamicArgument::Template(parts) => {
        match parts.first() {
          Some(DynamicTemplatePart::String { value: specifier }) => {
            // relative doesn't need to be modified
            if deno_path_util::is_relative_specifier(specifier) {
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
              PositionOrSourceRangeRef::PositionRange(&dep.argument_range),
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
        }
      }
      deno_graph::analysis::DynamicArgument::Expr => {
        false // failed analyzing
      }
    }
  }

  pub fn unfurl_to_changes(
    &self,
    url: &ModuleSpecifier,
    parsed_source: &ParsedSource,
    module_info: &deno_graph::analysis::ModuleInfo,
    text_changes: &mut Vec<TextChange>,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) {
    let text_info = parsed_source.text_info_lazy();
    let analyze_specifier =
      |specifier: &str,
       range: PositionOrSourceRangeRef,
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
            range: match range {
              PositionOrSourceRangeRef::PositionRange(position_range) => {
                to_range(text_info, position_range)
              }
              PositionOrSourceRangeRef::SourceRange(source_range) => {
                source_range.as_byte_range(parsed_source.start_pos())
              }
            },
            new_text: unfurled,
          });
        }
      };
    // collect import declaration start positions to correctly insert @ts-types
    // comments when multiple imports are on the same line
    let mut specifier_collector = SpecifierStartCollector::default();
    parsed_source.program().visit_with(&mut specifier_collector);

    for dep in &module_info.dependencies {
      match dep {
        DependencyDescriptor::Static(dep) => {
          let resolution_kind = if parsed_source.media_type().is_declaration() {
            deno_resolver::workspace::ResolutionKind::Types
          } else {
            match dep.kind {
              StaticDependencyKind::Export
              | StaticDependencyKind::Import
              | StaticDependencyKind::ImportSource
              | StaticDependencyKind::ExportEquals
              | StaticDependencyKind::ImportEquals => {
                deno_resolver::workspace::ResolutionKind::Execution
              }
              StaticDependencyKind::ExportType
              | StaticDependencyKind::ImportType
              | StaticDependencyKind::MaybeTsModuleAugmentation => {
                deno_resolver::workspace::ResolutionKind::Types
              }
            }
          };

          // check if we need to add @ts-types comment
          if dep.types_specifier.is_none()
            && let Ok(Some(unfurled)) = self.unfurl_specifier(
              url,
              &dep.specifier,
              deno_resolver::workspace::ResolutionKind::Types,
            )
            && let Some(types_specifier) =
              self.get_types_package_specifier(&unfurled, url)
          {
            // insert @ts-types comment before the import declaration
            let specifier_range =
              dep.specifier_range.as_source_range(text_info);
            let decl_start_byte_index = specifier_collector
              .specifier_to_decl_start
              .get(&specifier_range)
              .map(|start| start.as_byte_index(text_info.range().start))
              .unwrap_or_else(|| {
                // fallback to line start if not found (shouldn't happen)
                let line_start =
                  text_info.line_start(dep.specifier_range.start.line);
                line_start.as_byte_index(text_info.range().start)
              });
            // use block comment so it works even with multiple imports on one line
            text_changes.push(deno_ast::TextChange {
              range: decl_start_byte_index..decl_start_byte_index,
              new_text: format!("/* @ts-types=\"{}\" */ ", types_specifier),
            });
          }

          analyze_specifier(
            &dep.specifier,
            PositionOrSourceRangeRef::PositionRange(&dep.specifier_range),
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
        PositionOrSourceRangeRef::PositionRange(&specifier_with_range.range),
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }
    for jsdoc in &module_info.jsdoc_imports {
      analyze_specifier(
        &jsdoc.specifier.text,
        PositionOrSourceRangeRef::PositionRange(&jsdoc.specifier.range),
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        PositionOrSourceRangeRef::PositionRange(&specifier_with_range.range),
        deno_resolver::workspace::ResolutionKind::Execution,
        text_changes,
        diagnostic_reporter,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source_types {
      analyze_specifier(
        &specifier_with_range.text,
        PositionOrSourceRangeRef::PositionRange(&specifier_with_range.range),
        deno_resolver::workspace::ResolutionKind::Types,
        text_changes,
        diagnostic_reporter,
      );
    }

    let mut collector = ImportMetaResolveCollector::default();
    parsed_source.program().visit_with(&mut collector);
    for (range, specifier) in collector.specifiers {
      analyze_specifier(
        &specifier,
        PositionOrSourceRangeRef::SourceRange(range),
        deno_resolver::workspace::ResolutionKind::Execution,
        text_changes,
        diagnostic_reporter,
      );
    }
    for range in collector.diagnostic_ranges {
      diagnostic_reporter(
        SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve {
          specifier: url.to_owned(),
          range,
          text_info: text_info.clone(),
        },
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
      let last = resolved.path_segments().unwrap().next_back().unwrap();
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

/// Collects the start position of import/export declarations for each specifier.
/// This is needed to correctly insert @ts-types comments before import statements
/// when multiple imports appear on the same line.
#[derive(Default)]
struct SpecifierStartCollector {
  /// Maps the specifier source range to the start position of the declaration.
  specifier_to_decl_start: HashMap<SourceRange<SourcePos>, SourcePos>,
}

impl Visit for SpecifierStartCollector {
  noop_visit_type!();

  fn visit_import_decl(&mut self, node: &deno_ast::swc::ast::ImportDecl) {
    let decl_start = node.range().start;
    let src_range = node.src.range();
    self.specifier_to_decl_start.insert(src_range, decl_start);
  }

  fn visit_export_all(&mut self, node: &deno_ast::swc::ast::ExportAll) {
    let decl_start = node.range().start;
    let src_range = node.src.range();
    self.specifier_to_decl_start.insert(src_range, decl_start);
  }

  fn visit_named_export(&mut self, node: &deno_ast::swc::ast::NamedExport) {
    if let Some(src) = &node.src {
      let decl_start = node.range().start;
      let src_range = src.range();
      self.specifier_to_decl_start.insert(src_range, decl_start);
    }
  }
}

#[derive(Default)]
struct ImportMetaResolveCollector {
  specifiers: Vec<(SourceRange<SourcePos>, Atom)>,
  diagnostic_ranges: Vec<SourceRange<SourcePos>>,
}

impl Visit for ImportMetaResolveCollector {
  noop_visit_type!();

  fn visit_call_expr(&mut self, node: &deno_ast::swc::ast::CallExpr) {
    if node.args.len() == 1
      && let Some(first_arg) = node.args.first()
      && let Callee::Expr(callee) = &node.callee
      && let Expr::Member(member) = &**callee
      && let Expr::MetaProp(prop) = &*member.obj
      && prop.kind == MetaPropKind::ImportMeta
      && let MemberProp::Ident(ident) = &member.prop
      && ident.sym == "resolve"
      && first_arg.spread.is_none()
    {
      if let Expr::Lit(Lit::Str(arg)) = &*first_arg.expr {
        let range = arg.range();
        self.specifiers.push((
          // remove quotes
          SourceRange::new(range.start + 1, range.end - 1),
          arg.value.to_atom_lossy().into_owned(),
        ));
      } else {
        self.diagnostic_ranges.push(first_arg.expr.range());
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::path::Path;
  use std::path::PathBuf;
  use std::sync::Arc;

  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_core::serde_json::json;
  use deno_core::url::Url;
  use deno_graph::ast::ParserModuleAnalyzer;
  use deno_resolver::factory::ResolverFactory;
  use deno_resolver::factory::ResolverFactoryOptions;
  use deno_resolver::factory::WorkspaceFactory;
  use deno_resolver::factory::WorkspaceFactoryOptions;
  use pretty_assertions::assert_eq;
  use sys_traits::EnvCurrentDir;
  use sys_traits::impls::InMemorySys;

  use super::*;

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

  #[tokio::test]
  async fn test_unfurling() {
    let cwd = get_cwd();
    let memory_sys = InMemorySys::new_with_cwd(&cwd);
    memory_sys.fs_insert_json(
      cwd.join("deno.json"),
      json!({
        "workspace": [
          "./jsr-package"
        ],
        "imports": {
          "express": "npm:express@5",
          "lib/": "./lib/",
          "fizz": "./fizz/mod.ts",
          "@std/fs": "npm:@jsr/std__fs@1",
        }
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("package.json"),
      json!({
        "dependencies": {
          "chalk": 5
        }
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("jsr-package/deno.json"),
      json!({
        "name": "@denotest/example",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    memory_sys.fs_insert(cwd.join("b.ts"), "");
    memory_sys.fs_insert(cwd.join("c.js"), "");
    memory_sys.fs_insert(cwd.join("c.d.ts"), "");
    memory_sys.fs_insert(cwd.join("baz").join("index.js"), "");
    memory_sys.fs_insert(cwd.join("jsr-package/mod.ts"), "export default 1;");
    let unfurler = build_unfurler(memory_sys, &cwd).await;

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

import.meta.resolve("chalk");
import.meta.resolve(nonAnalyzable);
"#;
      let (unfurled_source, d) = unfurl_text_with_diagnostics(
        &cwd.join("mod.ts"),
        source_code,
        &unfurler,
      );
      assert_eq!(d.len(), 3);
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
      assert!(
        matches!(
          d[2],
          SpecifierUnfurlerDiagnostic::UnanalyzableImportMetaResolve { .. }
        ),
        "{:?}",
        d[2]
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

import.meta.resolve("npm:chalk@5");
import.meta.resolve(nonAnalyzable);
"#;
      assert_eq!(unfurled_source, expected_source);
    }

    // Unfurling .d.ts file should use types resolution.
    {
      let source_code = r#"import express from "express";"
export type * from "./c";
"#;
      let unfurled_source =
        unfurl_text(&cwd.join("mod.d.ts"), source_code, &unfurler);
      let expected_source = r#"import express from "npm:express@5";"
export type * from "./c.d.ts";
"#;
      assert_eq!(unfurled_source, expected_source);
    }
  }

  #[tokio::test]
  async fn test_unfurling_npm_dep_workspace_specifier() {
    let cwd = get_cwd();
    let memory_sys = InMemorySys::new_with_cwd(&cwd);
    memory_sys.fs_insert_json(
      cwd.join("package.json"),
      json!({ "workspaces": ["./publish", "./subtract", "./add"] }),
    );
    memory_sys.fs_insert_json(
      cwd.join("add/package.json"),
      json!({ "name": "add", "version": "0.1.0", }),
    );
    memory_sys.fs_insert_json(
      cwd.join("subtract/package.json"),
      json!({ "name": "subtract", "version": "0.2.0", }),
    );
    memory_sys.fs_insert_json(
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
    );
    memory_sys.fs_insert_json(
      cwd.join("publish/jsr.json"),
      json!({
        "name": "@denotest/main",
        "version": "1.0.0",
        "exports": "./mod.ts",
      }),
    );
    let unfurler = build_unfurler(memory_sys, &cwd).await;

    {
      let source_code = r#"import add from "add";
  import subtract from "subtract";

  console.log(add, subtract);
  "#;
      let unfurled_source = unfurl_text(
        &cwd.join("publish").join("mod.ts"),
        source_code,
        &unfurler,
      );
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
      let (unfurled_source, d) = unfurl_text_with_diagnostics(
        &cwd.join("publish").join("other.ts"),
        source_code,
        &unfurler,
      );
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

  #[tokio::test]
  async fn test_unfurl_types_package() {
    async fn run_test(memory_sys: InMemorySys) {
      #[allow(clippy::disallowed_methods)]
      let cwd = memory_sys.env_current_dir().unwrap();
      memory_sys.fs_insert_json(
        cwd.join("node_modules/package/package.json"),
        json!({
          "name": "package",
          "exports": {
            ".": "./index.js",
            "./subpath": "./subpath.js"
          }
        }),
      );
      memory_sys.fs_insert(cwd.join("node_modules/package/index.js"), "");
      memory_sys.fs_insert(cwd.join("node_modules/package/subpath.js"), "");
      memory_sys.fs_insert_json(
        cwd.join("node_modules/@types/package/package.json"),
        json!({
          "name": "@types/package",
          "types": "./index.d.ts",
          "exports": {
            ".": "./index.d.ts",
            "./subpath": "./subpath.d.ts"
          }
        }),
      );
      memory_sys
        .fs_insert(cwd.join("node_modules/@types/package/index.d.ts"), "");
      memory_sys
        .fs_insert(cwd.join("node_modules/@types/package/subpath.d.ts"), "");
      let unfurler = build_unfurler(memory_sys, &cwd).await;

      let source_code = r#"import { data } from "package"; import { a } from "package";
import { helper } from "package/subpath";
// @ts-types="npm:@types/package@^1.0"
import { other } from "package";
export { a, data, helper, other };
export { b } from "package"; export { c } from "package";
import type { d } from "package"; import type { e } from "package";
import type { f } from "@types/package";
export { d, e, f };
"#;
      let unfurled_source =
        unfurl_text(&cwd.join("mod.ts"), source_code, &unfurler);
      let expected_source = r#"/* @ts-types="npm:@types/package@^1" */ import { data } from "npm:package@^1.2.3"; /* @ts-types="npm:@types/package@^1" */ import { a } from "npm:package@^1.2.3";
/* @ts-types="npm:@types/package@^1/subpath" */ import { helper } from "npm:package@^1.2.3/subpath";
// @ts-types="npm:@types/package@^1.0"
import { other } from "npm:package@^1.2.3";
export { a, data, helper, other };
/* @ts-types="npm:@types/package@^1" */ export { b } from "npm:package@^1.2.3"; /* @ts-types="npm:@types/package@^1" */ export { c } from "npm:package@^1.2.3";
/* @ts-types="npm:@types/package@^1" */ import type { d } from "npm:package@^1.2.3"; /* @ts-types="npm:@types/package@^1" */ import type { e } from "npm:package@^1.2.3";
import type { f } from "npm:@types/package@^1";
export { d, e, f };
"#;
      // when using a deno.json or import map, it adds an extra slash at the
      // start, which is harmless, so ignore that in order to normalize to the
      // expected source
      assert_eq!(unfurled_source.replace("npm:/", "npm:"), expected_source);
    }

    // these different scenarios should all have the same outcome
    let cwd = get_cwd();
    // deno.json
    {
      let memory_sys = InMemorySys::new_with_cwd(&cwd);
      memory_sys.fs_insert_json(
        cwd.join("deno.json"),
        json!({
          "name": "@denotest/main",
          "version": "1.0.0",
          "exports": "./mod.ts",
          "nodeModulesDir": "manual",
          "imports": {
            "@types/package": "npm:@types/package@^1",
            "package": "npm:package@^1.2.3"
          }
        }),
      );
      run_test(memory_sys).await;
    }
    // package.json
    {
      let memory_sys = InMemorySys::new_with_cwd(&cwd);
      memory_sys.fs_insert_json(
        cwd.join("package.json"),
        json!({
          "dependencies": {
            "@types/package": "^1",
            "package": "^1.2.3"
          }
        }),
      );
      memory_sys.fs_insert_json(
        cwd.join("deno.json"),
        json!({
          "name": "@denotest/main",
          "version": "1.0.0",
          "exports": "./mod.ts"
        }),
      );
      run_test(memory_sys).await;
    }
    // import map
    {
      let memory_sys = InMemorySys::new_with_cwd(&cwd);
      memory_sys.fs_insert_json(
        cwd.join("deno.json"),
        json!({
          "name": "@denotest/main",
          "version": "1.0.0",
          "exports": "./mod.ts",
          "nodeModulesDir": "manual",
          "importMap": "./import_map.json",
        }),
      );
      memory_sys.fs_insert_json(
        cwd.join("import_map.json"),
        json!({
          "imports": {
            "@types/package": "npm:@types/package@^1",
            "@types/package/": "npm:/@types/package@^1/",
            "package": "npm:package@^1.2.3",
            "package/": "npm:/package@^1.2.3/",
          }
        }),
      );
      run_test(memory_sys).await;
    }
  }

  #[tokio::test]
  async fn test_unfurl_types_package_not_dep() {
    let cwd = get_cwd();
    let memory_sys = InMemorySys::new_with_cwd(&cwd);
    memory_sys.fs_insert_json(
      cwd.join("package.json"),
      json!({
        "dependencies": {
          "package": "^1.2.3"
        }
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("deno.json"),
      json!({
        "name": "@denotest/main",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("node_modules/package/package.json"),
      json!({
        "name": "package",
        "exports": {
          ".": "./index.js",
          "./subpath": "./subpath.js"
        }
      }),
    );
    memory_sys.fs_insert(cwd.join("node_modules/package/index.js"), "");
    memory_sys.fs_insert(cwd.join("node_modules/package/subpath.js"), "");
    memory_sys.fs_insert_json(
      cwd.join("node_modules/@types/package/package.json"),
      json!({
        "name": "@types/package",
        "version": "1.5.6",
        "types": "./index.d.ts",
        "exports": {
          ".": "./index.d.ts",
          "./subpath": "./subpath.d.ts"
        }
      }),
    );
    memory_sys
      .fs_insert(cwd.join("node_modules/@types/package/index.d.ts"), "");
    memory_sys
      .fs_insert(cwd.join("node_modules/@types/package/subpath.d.ts"), "");
    let unfurler = build_unfurler(memory_sys, &cwd).await;

    let source_code = r#"import { data } from "package";
import { helper } from "package/subpath";
export { data, helper };
"#;
    let unfurled_source =
      unfurl_text(&cwd.join("mod.ts"), source_code, &unfurler);
    let expected_source = r#"/* @ts-types="npm:@types/package@^1.5.6" */ import { data } from "npm:package@^1.2.3";
/* @ts-types="npm:@types/package@^1.5.6/subpath" */ import { helper } from "npm:package@^1.2.3/subpath";
export { data, helper };
"#;
    assert_eq!(unfurled_source, expected_source);
  }

  #[tokio::test]
  async fn test_unfurl_types_in_original_package() {
    let cwd = get_cwd();
    let memory_sys = InMemorySys::new_with_cwd(&cwd);
    memory_sys.fs_insert_json(
      cwd.join("package.json"),
      json!({
        "dependencies": {
          "@types/package": "^1",
          "package": "^1.2.3"
        }
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("deno.json"),
      json!({
        "name": "@denotest/main",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    memory_sys.fs_insert_json(
      cwd.join("node_modules/package/package.json"),
      json!({
        "name": "package",
        "exports": {
          ".": "./index.js",
          "./subpath": "./subpath.js"
        }
      }),
    );
    memory_sys.fs_insert(cwd.join("node_modules/package/index.js"), "");
    memory_sys.fs_insert(cwd.join("node_modules/package/index.d.ts"), ""); // types here, so no injection
    memory_sys.fs_insert(cwd.join("node_modules/package/subpath.js"), "");
    memory_sys.fs_insert_json(
      cwd.join("node_modules/@types/package/package.json"),
      json!({
        "name": "@types/package",
        "types": "./index.d.ts",
        "exports": {
          ".": "./index.d.ts",
          "./subpath": "./subpath.d.ts"
        }
      }),
    );
    memory_sys
      .fs_insert(cwd.join("node_modules/@types/package/index.d.ts"), "");
    memory_sys
      .fs_insert(cwd.join("node_modules/@types/package/subpath.d.ts"), "");
    let unfurler = build_unfurler(memory_sys, &cwd).await;

    let source_code = r#"import { data } from "package";
import { helper } from "package/subpath";
export { data, helper };
"#;
    let unfurled_source =
      unfurl_text(&cwd.join("mod.ts"), source_code, &unfurler);
    let expected_source = r#"import { data } from "npm:package@^1.2.3";
/* @ts-types="npm:@types/package@^1/subpath" */ import { helper } from "npm:package@^1.2.3/subpath";
export { data, helper };
"#;
    assert_eq!(unfurled_source, expected_source);
  }

  #[tokio::test]
  async fn test_unfurling_package_json_jsr_dep() {
    let cwd = get_cwd();
    let memory_sys = InMemorySys::new_with_cwd(&cwd);
    memory_sys.fs_insert_json(
      cwd.join("package.json"),
      json!({
        "name": "add",
        "version": "0.1.0",
        "dependencies": {
          "path": "jsr:@std/path@1",
          "@std/fs": "jsr:^1",
          "semver": "npm:@jsr/std__semver@1",
        },
        "exports": "./mod.ts"
      }),
    );
    let unfurler = build_unfurler(memory_sys, &cwd).await;

    let source_code = r#"export * from "path";
export * from "@std/fs";
export * from "semver";
"#;
    let unfurled_source =
      unfurl_text(&cwd.join("mod.ts"), source_code, &unfurler);
    // these will be unfurled to jsr: specifiers because they're jsr packages
    let expected_source = r#"export * from "jsr:@std/path@1";
export * from "jsr:@std/fs@^1";
export * from "jsr:@std/semver@1";
"#;
    assert_eq!(unfurled_source, expected_source);
  }

  fn get_cwd() -> PathBuf {
    if cfg!(windows) {
      PathBuf::from("C:\\unfurl")
    } else {
      PathBuf::from("/unfurl")
    }
  }

  async fn build_unfurler(
    sys: InMemorySys,
    cwd: &Path,
  ) -> SpecifierUnfurler<InMemorySys> {
    let workspace_factory = Arc::new(WorkspaceFactory::new(
      sys,
      cwd.to_path_buf(),
      WorkspaceFactoryOptions::default(),
    ));
    let resolver_factory = ResolverFactory::new(
      workspace_factory,
      ResolverFactoryOptions {
        package_json_dep_resolution: Some(
          deno_resolver::workspace::PackageJsonDepResolution::Enabled,
        ),
        unstable_sloppy_imports: true,
        ..Default::default()
      },
    );

    SpecifierUnfurler::new(
      resolver_factory.node_resolver().unwrap().clone(),
      resolver_factory.npm_req_resolver().unwrap().clone(),
      resolver_factory.pkg_json_resolver().clone(),
      resolver_factory
        .workspace_factory()
        .workspace_directory()
        .unwrap()
        .clone(),
      resolver_factory.workspace_resolver().await.unwrap().clone(),
      true,
    )
  }

  fn unfurl_text(
    path: &Path,
    source_code: &str,
    unfurler: &SpecifierUnfurler<InMemorySys>,
  ) -> String {
    let (unfurled_source, d) =
      unfurl_text_with_diagnostics(path, source_code, unfurler);
    assert_eq!(d.len(), 0);
    unfurled_source
  }

  fn unfurl_text_with_diagnostics(
    path: &Path,
    source_code: &str,
    unfurler: &SpecifierUnfurler<InMemorySys>,
  ) -> (String, Vec<SpecifierUnfurlerDiagnostic>) {
    let specifier = ModuleSpecifier::from_file_path(path).unwrap();
    let source = parse_ast(&specifier, source_code);
    let mut d = Vec::new();
    let mut reporter = |diagnostic| d.push(diagnostic);
    let unfurled_source = unfurl(unfurler, &specifier, &source, &mut reporter);
    (unfurled_source, d)
  }

  fn unfurl(
    unfurler: &SpecifierUnfurler<InMemorySys>,
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

    deno_ast::apply_text_changes(text_info.text_str(), text_changes)
  }
}
