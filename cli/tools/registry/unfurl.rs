// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::WorkspaceResolver;
use deno_core::ModuleSpecifier;
use deno_graph::DependencyDescriptor;
use deno_graph::DynamicTemplatePart;
use deno_graph::ParserModuleAnalyzer;
use deno_graph::TypeScriptReference;
use deno_package_json::PackageJsonDepValue;
use deno_resolver::sloppy_imports::SloppyImportsResolutionMode;
use deno_runtime::deno_node::is_builtin_node_module;

use crate::resolver::CliSloppyImportsResolver;

#[derive(Debug, Clone)]
pub enum SpecifierUnfurlerDiagnostic {
  UnanalyzableDynamicImport {
    specifier: ModuleSpecifier,
    text_info: SourceTextInfo,
    range: SourceRange,
  },
}

impl SpecifierUnfurlerDiagnostic {
  pub fn code(&self) -> &'static str {
    match self {
      Self::UnanalyzableDynamicImport { .. } => "unanalyzable-dynamic-import",
    }
  }

  pub fn message(&self) -> &'static str {
    match self {
      Self::UnanalyzableDynamicImport { .. } => {
        "unable to analyze dynamic import"
      }
    }
  }
}

pub struct SpecifierUnfurler {
  sloppy_imports_resolver: Option<CliSloppyImportsResolver>,
  workspace_resolver: WorkspaceResolver,
  bare_node_builtins: bool,
}

impl SpecifierUnfurler {
  pub fn new(
    sloppy_imports_resolver: Option<CliSloppyImportsResolver>,
    workspace_resolver: WorkspaceResolver,
    bare_node_builtins: bool,
  ) -> Self {
    debug_assert_eq!(
      workspace_resolver.pkg_json_dep_resolution(),
      PackageJsonDepResolution::Enabled
    );
    Self {
      sloppy_imports_resolver,
      workspace_resolver,
      bare_node_builtins,
    }
  }

  fn unfurl_specifier(
    &self,
    referrer: &ModuleSpecifier,
    specifier: &str,
  ) -> Option<String> {
    let resolved = if let Ok(resolved) =
      self.workspace_resolver.resolve(specifier, referrer)
    {
      match resolved {
        MappedResolution::Normal { specifier, .. }
        | MappedResolution::ImportMap { specifier, .. } => Some(specifier),
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
            PackageJsonDepValue::Workspace(version_req) => {
              // todo(#24612): consider warning or error when this is also a jsr package?
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
      None => ModuleSpecifier::options()
        .base_url(Some(referrer))
        .parse(specifier)
        .ok()?,
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
    let resolved =
      if let Some(sloppy_imports_resolver) = &self.sloppy_imports_resolver {
        sloppy_imports_resolver
          .resolve(&resolved, SloppyImportsResolutionMode::Execution)
          .map(|res| res.into_specifier())
          .unwrap_or(resolved)
      } else {
        resolved
      };
    let relative_resolved = relative_url(&resolved, referrer);
    if relative_resolved == specifier {
      None // nothing to unfurl
    } else {
      log::debug!(
        "Unfurled specifier: {} from {} -> {}",
        specifier,
        referrer,
        relative_resolved
      );
      Some(relative_resolved)
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
  ) -> bool {
    match &dep.argument {
      deno_graph::DynamicArgument::String(specifier) => {
        let range = to_range(text_info, &dep.argument_range);
        let maybe_relative_index =
          text_info.text_str()[range.start..range.end].find(specifier);
        let Some(relative_index) = maybe_relative_index else {
          return true; // always say it's analyzable for a string
        };
        let unfurled = self.unfurl_specifier(module_url, specifier);
        if let Some(unfurled) = unfurled {
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
          let unfurled = self.unfurl_specifier(module_url, specifier);
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

  pub fn unfurl(
    &self,
    url: &ModuleSpecifier,
    parsed_source: &ParsedSource,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> String {
    let mut text_changes = Vec::new();
    let text_info = parsed_source.text_info_lazy();
    let module_info = ParserModuleAnalyzer::module_info(parsed_source);
    let analyze_specifier =
      |specifier: &str,
       range: &deno_graph::PositionRange,
       text_changes: &mut Vec<deno_ast::TextChange>| {
        if let Some(unfurled) = self.unfurl_specifier(url, specifier) {
          text_changes.push(deno_ast::TextChange {
            range: to_range(text_info, range),
            new_text: unfurled,
          });
        }
      };
    for dep in &module_info.dependencies {
      match dep {
        DependencyDescriptor::Static(dep) => {
          analyze_specifier(
            &dep.specifier,
            &dep.specifier_range,
            &mut text_changes,
          );
        }
        DependencyDescriptor::Dynamic(dep) => {
          let success =
            self.try_unfurl_dynamic_dep(url, text_info, dep, &mut text_changes);

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
        TypeScriptReference::Path(range) => range,
        TypeScriptReference::Types(range) => range,
      };
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }
    for specifier_with_range in &module_info.jsdoc_imports {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }

    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);
    rewritten_text
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

  use crate::resolver::SloppyImportsCachedFs;

  use super::*;
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_config::workspace::ResolverWorkspaceJsrPackage;
  use deno_core::serde_json::json;
  use deno_core::url::Url;
  use deno_runtime::deno_fs::RealFs;
  use deno_runtime::deno_node::PackageJson;
  use deno_semver::Version;
  use import_map::ImportMapWithDiagnostics;
  use indexmap::IndexMap;
  use pretty_assertions::assert_eq;
  use test_util::testdata_path;

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
    );
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
      deno_config::workspace::PackageJsonDepResolution::Enabled,
    );
    let fs = Arc::new(RealFs);
    let unfurler = SpecifierUnfurler::new(
      Some(CliSloppyImportsResolver::new(SloppyImportsCachedFs::new(
        fs,
      ))),
      workspace_resolver,
      true,
    );

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
      let unfurled_source = unfurler.unfurl(&specifier, &source, &mut reporter);
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
  }
}
