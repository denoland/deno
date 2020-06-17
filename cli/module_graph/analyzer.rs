// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::doc::Location;
use crate::msg::MediaType;
use crate::swc_common::comments::CommentKind;
use crate::swc_common::Span;
use crate::swc_ecma_ast;
use crate::swc_util::AstParser;
use crate::swc_util::SwcDiagnosticBuffer;
use swc_ecma_visit::Node;
use swc_ecma_visit::Visit;

struct DependencyVisitor {
  dependencies: Vec<String>,
  analyze_dynamic_imports: bool,
}

impl Visit for DependencyVisitor {
  fn visit_import_decl(
    &mut self,
    import_decl: &swc_ecma_ast::ImportDecl,
    _parent: &dyn Node,
  ) {
    let src_str = import_decl.src.value.to_string();
    self.dependencies.push(src_str);
  }

  fn visit_named_export(
    &mut self,
    named_export: &swc_ecma_ast::NamedExport,
    _parent: &dyn Node,
  ) {
    if let Some(src) = &named_export.src {
      let src_str = src.value.to_string();
      self.dependencies.push(src_str);
    }
  }

  fn visit_export_all(
    &mut self,
    export_all: &swc_ecma_ast::ExportAll,
    _parent: &dyn Node,
  ) {
    let src_str = export_all.src.value.to_string();
    self.dependencies.push(src_str);
  }

  fn visit_call_expr(
    &mut self,
    call_expr: &swc_ecma_ast::CallExpr,
    _parent: &dyn Node,
  ) {
    if !self.analyze_dynamic_imports {
      return;
    }

    use swc_ecma_ast::Expr::*;
    use swc_ecma_ast::ExprOrSuper::*;

    let boxed_expr = match call_expr.callee.clone() {
      Super(_) => return,
      Expr(boxed) => boxed,
    };

    match &*boxed_expr {
      Ident(ident) => {
        if &ident.sym.to_string() != "import" {
          return;
        }
      }
      _ => return,
    };

    if let Some(arg) = call_expr.args.get(0) {
      match &*arg.expr {
        Lit(lit) => {
          if let swc_ecma_ast::Lit::Str(str_) = lit {
            let src_str = str_.value.to_string();
            self.dependencies.push(src_str);
          }
        }
        _ => return,
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
enum DependencyKind {
  Import,
  DynamicImport,
  Export,
}

#[derive(Clone, Debug, PartialEq)]
struct DependencyDescriptor {
  span: Span,
  specifier: String,
  kind: DependencyKind,
}

struct NewDependencyVisitor {
  dependencies: Vec<DependencyDescriptor>,
}

impl Visit for NewDependencyVisitor {
  fn visit_import_decl(
    &mut self,
    import_decl: &swc_ecma_ast::ImportDecl,
    _parent: &dyn Node,
  ) {
    let src_str = import_decl.src.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Import,
      span: import_decl.span,
    });
  }

  fn visit_named_export(
    &mut self,
    named_export: &swc_ecma_ast::NamedExport,
    _parent: &dyn Node,
  ) {
    if let Some(src) = &named_export.src {
      let src_str = src.value.to_string();
      self.dependencies.push(DependencyDescriptor {
        specifier: src_str,
        kind: DependencyKind::Export,
        span: named_export.span,
      });
    }
  }

  fn visit_export_all(
    &mut self,
    export_all: &swc_ecma_ast::ExportAll,
    _parent: &dyn Node,
  ) {
    let src_str = export_all.src.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Export,
      span: export_all.span,
    });
  }

  fn visit_ts_import_type(
    &mut self,
    ts_import_type: &swc_ecma_ast::TsImportType,
    _parent: &dyn Node,
  ) {
    // TODO(bartlomieju): possibly add separate DependencyKind
    let src_str = ts_import_type.arg.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Import,
      span: ts_import_type.arg.span,
    });
  }

  fn visit_call_expr(
    &mut self,
    call_expr: &swc_ecma_ast::CallExpr,
    parent: &dyn Node,
  ) {
    use swc_ecma_ast::Expr::*;
    use swc_ecma_ast::ExprOrSuper::*;

    swc_ecma_visit::visit_call_expr(self, call_expr, parent);
    let boxed_expr = match call_expr.callee.clone() {
      Super(_) => return,
      Expr(boxed) => boxed,
    };

    match &*boxed_expr {
      Ident(ident) => {
        if &ident.sym.to_string() != "import" {
          return;
        }
      }
      _ => return,
    };

    if let Some(arg) = call_expr.args.get(0) {
      match &*arg.expr {
        Lit(lit) => {
          if let swc_ecma_ast::Lit::Str(str_) = lit {
            let src_str = str_.value.to_string();
            self.dependencies.push(DependencyDescriptor {
              specifier: src_str,
              kind: DependencyKind::DynamicImport,
              span: call_expr.span,
            });
          }
        }
        _ => return,
      }
    }
  }
}

fn get_deno_types(parser: &AstParser, span: Span) -> Option<String> {
  let comments = parser.get_span_comments(span);

  if comments.is_empty() {
    return None;
  }

  // @deno-types must directly prepend import statement - hence
  // checking last comment for span
  let last = comments.last().unwrap();
  let comment = last.text.trim_start();

  if comment.starts_with("@deno-types") {
    let split: Vec<String> =
      comment.split('=').map(|s| s.to_string()).collect();
    assert_eq!(split.len(), 2);
    let specifier_in_quotes = split.get(1).unwrap().to_string();
    let specifier = specifier_in_quotes
      .trim_start_matches('\"')
      .trim_start_matches('\'')
      .trim_end_matches('\"')
      .trim_end_matches('\'')
      .to_string();
    return Some(specifier);
  }

  None
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportDescriptor {
  pub specifier: String,
  pub deno_types: Option<String>,
  pub location: Location,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TsReferenceKind {
  Lib,
  Types,
  Path,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TsReferenceDescriptor {
  pub kind: TsReferenceKind,
  pub specifier: String,
  pub location: Location,
}

pub fn analyze_dependencies_and_references(
  file_name: &str,
  media_type: MediaType,
  source_code: &str,
  analyze_dynamic_imports: bool,
) -> Result<
  (Vec<ImportDescriptor>, Vec<TsReferenceDescriptor>),
  SwcDiagnosticBuffer,
> {
  let parser = AstParser::new();
  parser.parse_module(file_name, media_type, source_code, |parse_result| {
    let module = parse_result?;
    let mut collector = NewDependencyVisitor {
      dependencies: vec![],
    };
    let module_span = module.span;
    collector.visit_module(&module, &module);

    let dependency_descriptors = collector.dependencies;

    // for each import check if there's relevant @deno-types directive
    let imports = dependency_descriptors
      .iter()
      .filter(|desc| {
        if analyze_dynamic_imports {
          return true;
        }

        desc.kind != DependencyKind::DynamicImport
      })
      .map(|desc| {
        let location = parser.get_span_location(desc.span);
        let deno_types = get_deno_types(&parser, desc.span);
        ImportDescriptor {
          specifier: desc.specifier.to_string(),
          deno_types,
          location: location.into(),
        }
      })
      .collect();

    // analyze comment from beginning of the file and find TS directives
    let comments = parser
      .comments
      .take_leading_comments(module_span.lo())
      .unwrap_or_else(Vec::new);

    let mut references = vec![];
    for comment in comments {
      if comment.kind != CommentKind::Line {
        continue;
      }

      // TODO(bartlomieju): you can do better than that...
      let text = comment.text.to_string();
      let (kind, specifier_in_quotes) =
        if text.starts_with("/ <reference path=") {
          (
            TsReferenceKind::Path,
            text.trim_start_matches("/ <reference path="),
          )
        } else if text.starts_with("/ <reference lib=") {
          (
            TsReferenceKind::Lib,
            text.trim_start_matches("/ <reference lib="),
          )
        } else if text.starts_with("/ <reference types=") {
          (
            TsReferenceKind::Types,
            text.trim_start_matches("/ <reference types="),
          )
        } else {
          continue;
        };
      let specifier = specifier_in_quotes
        .trim_end_matches("/>")
        .trim_end()
        .trim_start_matches('\"')
        .trim_start_matches('\'')
        .trim_end_matches('\"')
        .trim_end_matches('\'')
        .to_string();

      let location = parser.get_span_location(comment.span);
      references.push(TsReferenceDescriptor {
        kind,
        specifier,
        location: location.into(),
      });
    }
    Ok((imports, references))
  })
}

#[test]
fn test_analyze_dependencies_and_directives() {
  let source = r#"
// This comment is placed to make sure that directives are parsed
// even when they start on non-first line
  
/// <reference lib="dom" />
/// <reference types="./type_reference.d.ts" />
/// <reference path="./type_reference/dep.ts" />
// @deno-types="./type_definitions/foo.d.ts"
import { foo } from "./type_definitions/foo.js";
// @deno-types="./type_definitions/fizz.d.ts"
import "./type_definitions/fizz.js";

/// <reference path="./type_reference/dep2.ts" />

import * as qat from "./type_definitions/qat.ts";

console.log(foo);
console.log(fizz);
console.log(qat.qat);  
"#;

  let (imports, references) = analyze_dependencies_and_references(
    "some/file.ts",
    MediaType::TypeScript,
    source,
    true,
  )
  .expect("Failed to parse");

  assert_eq!(
    imports,
    vec![
      ImportDescriptor {
        specifier: "./type_definitions/foo.js".to_string(),
        deno_types: Some("./type_definitions/foo.d.ts".to_string()),
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 9,
          col: 0,
        },
      },
      ImportDescriptor {
        specifier: "./type_definitions/fizz.js".to_string(),
        deno_types: Some("./type_definitions/fizz.d.ts".to_string()),
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 11,
          col: 0,
        },
      },
      ImportDescriptor {
        specifier: "./type_definitions/qat.ts".to_string(),
        deno_types: None,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 15,
          col: 0,
        },
      },
    ]
  );

  // According to TS docs (https://www.typescriptlang.org/docs/handbook/triple-slash-directives.html)
  // directives that are not at the top of the file are ignored, so only
  // 3 references should be captured instead of 4.
  assert_eq!(
    references,
    vec![
      TsReferenceDescriptor {
        specifier: "dom".to_string(),
        kind: TsReferenceKind::Lib,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 5,
          col: 0,
        },
      },
      TsReferenceDescriptor {
        specifier: "./type_reference.d.ts".to_string(),
        kind: TsReferenceKind::Types,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 6,
          col: 0,
        },
      },
      TsReferenceDescriptor {
        specifier: "./type_reference/dep.ts".to_string(),
        kind: TsReferenceKind::Path,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 7,
          col: 0,
        },
      },
    ]
  );
}
