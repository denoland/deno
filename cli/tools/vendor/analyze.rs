// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_ast::swc::ast::ExportDefaultDecl;
use deno_ast::swc::ast::ExportSpecifier;
use deno_ast::swc::ast::ModuleExportName;
use deno_ast::swc::ast::NamedExport;
use deno_ast::swc::ast::Program;
use deno_ast::swc::visit::noop_visit_type;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::ParsedSource;

/// Gets if the parsed source has a default export.
pub fn has_default_export(source: &ParsedSource) -> bool {
  let mut visitor = DefaultExportFinder {
    has_default_export: false,
  };
  let program = source.program();
  let program: &Program = &program;
  program.visit_with(&mut visitor);
  visitor.has_default_export
}

struct DefaultExportFinder {
  has_default_export: bool,
}

impl<'a> Visit for DefaultExportFinder {
  noop_visit_type!();

  fn visit_export_default_decl(&mut self, _: &ExportDefaultDecl) {
    self.has_default_export = true;
  }

  fn visit_named_export(&mut self, named_export: &NamedExport) {
    if named_export
      .specifiers
      .iter()
      .any(export_specifier_has_default)
    {
      self.has_default_export = true;
    }
  }
}

fn export_specifier_has_default(s: &ExportSpecifier) -> bool {
  match s {
    ExportSpecifier::Default(_) => true,
    ExportSpecifier::Namespace(_) => false,
    ExportSpecifier::Named(named) => {
      let export_name = named.exported.as_ref().unwrap_or(&named.orig);

      match export_name {
        ModuleExportName::Str(_) => false,
        ModuleExportName::Ident(ident) => &*ident.sym == "default",
      }
    }
  }
}

#[cfg(test)]
mod test {
  use deno_ast::MediaType;
  use deno_ast::ParseParams;
  use deno_ast::ParsedSource;
  use deno_ast::SourceTextInfo;

  use super::has_default_export;

  #[test]
  fn has_default_when_export_default_decl() {
    let parsed_source = parse_module("export default class Class {}");
    assert!(has_default_export(&parsed_source));
  }

  #[test]
  fn has_default_when_named_export() {
    let parsed_source = parse_module("export {default} from './test.ts';");
    assert!(has_default_export(&parsed_source));
  }

  #[test]
  fn has_default_when_named_export_alias() {
    let parsed_source =
      parse_module("export {test as default} from './test.ts';");
    assert!(has_default_export(&parsed_source));
  }

  #[test]
  fn not_has_default_when_named_export_not_exported() {
    let parsed_source =
      parse_module("export {default as test} from './test.ts';");
    assert!(!has_default_export(&parsed_source));
  }

  #[test]
  fn not_has_default_when_not() {
    let parsed_source = parse_module("export {test} from './test.ts'; export class Test{} export * from './test';");
    assert!(!has_default_export(&parsed_source));
  }

  fn parse_module(text: &str) -> ParsedSource {
    deno_ast::parse_module(ParseParams {
      specifier: "file:///mod.ts".to_string(),
      capture_tokens: false,
      maybe_syntax: None,
      media_type: MediaType::TypeScript,
      scope_analysis: false,
      source: SourceTextInfo::from_string(text.to_string()),
    })
    .unwrap()
  }
}
