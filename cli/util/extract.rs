// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use deno_ast::swc::ast;
use deno_ast::swc::atoms::Atom;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::swc::common::DUMMY_SP;
use deno_ast::swc::ecma_visit::visit_mut_pass;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitMut;
use deno_ast::swc::ecma_visit::VisitWith as _;
use deno_ast::swc::utils as swc_utils;
use deno_ast::MediaType;
use deno_ast::SourceRangedForSpanned as _;
use deno_cache_dir::file_fetcher::File;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use regex::Regex;

use crate::file_fetcher::TextDecodedFile;
use crate::util::path::mapped_specifier_for_tsc;

/// Extracts doc tests from a given file, transforms them into pseudo test
/// files by wrapping the content of the doc tests in a `Deno.test` call, and
/// returns a list of the pseudo test files.
///
/// The difference from [`extract_snippet_files`] is that this function wraps
/// extracted code snippets in a `Deno.test` call.
pub fn extract_doc_tests(file: File) -> Result<Vec<File>, AnyError> {
  extract_inner(file, WrapKind::DenoTest)
}

/// Extracts code snippets from a given file and returns a list of the extracted
/// files.
///
/// The difference from [`extract_doc_tests`] is that this function does *not*
/// wrap extracted code snippets in a `Deno.test` call.
pub fn extract_snippet_files(file: File) -> Result<Vec<File>, AnyError> {
  extract_inner(file, WrapKind::NoWrap)
}

#[derive(Clone, Copy)]
enum WrapKind {
  DenoTest,
  NoWrap,
}

fn extract_inner(
  file: File,
  wrap_kind: WrapKind,
) -> Result<Vec<File>, AnyError> {
  let file = TextDecodedFile::decode(file)?;

  let exports = match deno_ast::parse_program(deno_ast::ParseParams {
    specifier: file.specifier.clone(),
    text: file.source.clone(),
    media_type: file.media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) {
    Ok(parsed) => {
      let mut c = ExportCollector::default();
      c.visit_program(parsed.program().as_ref());
      c
    }
    Err(_) => ExportCollector::default(),
  };

  let extracted_files = if file.media_type == MediaType::Unknown {
    extract_files_from_fenced_blocks(
      &file.specifier,
      &file.source,
      file.media_type,
    )?
  } else {
    extract_files_from_source_comments(
      &file.specifier,
      file.source.clone(),
      file.media_type,
    )?
  };

  extracted_files
    .into_iter()
    .map(|extracted_file| {
      generate_pseudo_file(extracted_file, &file.specifier, &exports, wrap_kind)
    })
    .collect::<Result<_, _>>()
}

fn extract_files_from_fenced_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<Vec<File>, AnyError> {
  // The pattern matches code blocks as well as anything in HTML comment syntax,
  // but it stores the latter without any capturing groups. This way, a simple
  // check can be done to see if a block is inside a comment (and skip typechecking)
  // or not by checking for the presence of capturing groups in the matches.
  let blocks_regex =
    lazy_regex::regex!(r"(?s)<!--.*?-->|```([^\r\n]*)\r?\n([\S\s]*?)```");
  let lines_regex = lazy_regex::regex!(r"(((#!+).*)|(?:# ?)?(.*))");

  extract_files_from_regex_blocks(
    specifier,
    source,
    media_type,
    /* file line index */ 0,
    blocks_regex,
    lines_regex,
  )
}

fn extract_files_from_source_comments(
  specifier: &ModuleSpecifier,
  source: Arc<str>,
  media_type: MediaType,
) -> Result<Vec<File>, AnyError> {
  let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: source,
    media_type,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })?;
  let comments = parsed_source.comments().get_vec();
  let blocks_regex = lazy_regex::regex!(r"```([^\r\n]*)\r?\n([\S\s]*?)```");
  let lines_regex =
    lazy_regex::regex!(r"(?:\* ?)((#!+).*)|(?:\* ?)(?:\# ?)?(.*)");

  let files = comments
    .iter()
    .filter(|comment| {
      if comment.kind != CommentKind::Block || !comment.text.starts_with('*') {
        return false;
      }

      true
    })
    .flat_map(|comment| {
      extract_files_from_regex_blocks(
        specifier,
        &comment.text,
        media_type,
        parsed_source.text_info_lazy().line_index(comment.start()),
        blocks_regex,
        lines_regex,
      )
    })
    .flatten()
    .collect();

  Ok(files)
}

fn extract_files_from_regex_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  file_line_index: usize,
  blocks_regex: &Regex,
  lines_regex: &Regex,
) -> Result<Vec<File>, AnyError> {
  let files = blocks_regex
    .captures_iter(source)
    .filter_map(|block| {
      block.get(1)?;

      let maybe_attributes: Option<Vec<_>> = block
        .get(1)
        .map(|attributes| attributes.as_str().split(' ').collect());

      let file_media_type = if let Some(attributes) = maybe_attributes {
        if attributes.contains(&"ignore") {
          return None;
        }

        match attributes.first() {
          Some(&"js") => MediaType::JavaScript,
          Some(&"javascript") => MediaType::JavaScript,
          Some(&"mjs") => MediaType::Mjs,
          Some(&"cjs") => MediaType::Cjs,
          Some(&"jsx") => MediaType::Jsx,
          Some(&"ts") => MediaType::TypeScript,
          Some(&"typescript") => MediaType::TypeScript,
          Some(&"mts") => MediaType::Mts,
          Some(&"cts") => MediaType::Cts,
          Some(&"tsx") => MediaType::Tsx,
          _ => MediaType::Unknown,
        }
      } else {
        media_type
      };

      if file_media_type == MediaType::Unknown {
        return None;
      }

      let line_offset = source[0..block.get(0).unwrap().start()]
        .chars()
        .filter(|c| *c == '\n')
        .count();

      let line_count = block.get(0).unwrap().as_str().split('\n').count();

      let body = block.get(2).unwrap();
      let text = body.as_str();

      // TODO(caspervonb) generate an inline source map
      let mut file_source = String::new();
      for line in lines_regex.captures_iter(text) {
        let text = line.get(1).or_else(|| line.get(3)).unwrap();
        writeln!(file_source, "{}", text.as_str()).unwrap();
      }

      let file_specifier = ModuleSpecifier::parse(&format!(
        "{}${}-{}",
        specifier,
        file_line_index + line_offset + 1,
        file_line_index + line_offset + line_count + 1,
      ))
      .unwrap();
      let file_specifier =
        mapped_specifier_for_tsc(&file_specifier, file_media_type)
          .map(|s| ModuleSpecifier::parse(&s).unwrap())
          .unwrap_or(file_specifier);

      Some(File {
        url: file_specifier,
        maybe_headers: None,
        source: file_source.into_bytes().into(),
      })
    })
    .collect();

  Ok(files)
}

#[derive(Default)]
struct ExportCollector {
  named_exports: BTreeSet<Atom>,
  default_export: Option<Atom>,
}

impl ExportCollector {
  fn to_import_specifiers(
    &self,
    symbols_to_exclude: &rustc_hash::FxHashSet<Atom>,
  ) -> Vec<ast::ImportSpecifier> {
    let mut import_specifiers = vec![];

    if let Some(default_export) = &self.default_export {
      // If the default export conflicts with a named export, a named one
      // takes precedence.
      if !symbols_to_exclude.contains(default_export)
        && !self.named_exports.contains(default_export)
      {
        import_specifiers.push(ast::ImportSpecifier::Default(
          ast::ImportDefaultSpecifier {
            span: DUMMY_SP,
            local: ast::Ident {
              span: DUMMY_SP,
              ctxt: Default::default(),
              sym: default_export.clone(),
              optional: false,
            },
          },
        ));
      }
    }

    for named_export in &self.named_exports {
      if symbols_to_exclude.contains(named_export) {
        continue;
      }

      import_specifiers.push(ast::ImportSpecifier::Named(
        ast::ImportNamedSpecifier {
          span: DUMMY_SP,
          local: ast::Ident {
            span: DUMMY_SP,
            ctxt: Default::default(),
            sym: named_export.clone(),
            optional: false,
          },
          imported: None,
          is_type_only: false,
        },
      ));
    }

    import_specifiers
  }
}

impl Visit for ExportCollector {
  fn visit_ts_module_decl(&mut self, ts_module_decl: &ast::TsModuleDecl) {
    if ts_module_decl.declare {
      return;
    }

    ts_module_decl.visit_children_with(self);
  }

  fn visit_export_decl(&mut self, export_decl: &ast::ExportDecl) {
    match &export_decl.decl {
      ast::Decl::Class(class) => {
        self.named_exports.insert(class.ident.sym.clone());
      }
      ast::Decl::Fn(func) => {
        self.named_exports.insert(func.ident.sym.clone());
      }
      ast::Decl::Var(var) => {
        for var_decl in &var.decls {
          let atoms = extract_sym_from_pat(&var_decl.name);
          self.named_exports.extend(atoms);
        }
      }
      ast::Decl::TsEnum(ts_enum) => {
        self.named_exports.insert(ts_enum.id.sym.clone());
      }
      ast::Decl::TsModule(ts_module) => {
        if ts_module.declare {
          return;
        }

        match &ts_module.id {
          ast::TsModuleName::Ident(ident) => {
            self.named_exports.insert(ident.sym.clone());
          }
          ast::TsModuleName::Str(s) => {
            self.named_exports.insert(s.value.clone());
          }
        }
      }
      ast::Decl::TsTypeAlias(ts_type_alias) => {
        self.named_exports.insert(ts_type_alias.id.sym.clone());
      }
      ast::Decl::TsInterface(ts_interface) => {
        self.named_exports.insert(ts_interface.id.sym.clone());
      }
      ast::Decl::Using(_) => {}
    }
  }

  fn visit_export_default_decl(
    &mut self,
    export_default_decl: &ast::ExportDefaultDecl,
  ) {
    match &export_default_decl.decl {
      ast::DefaultDecl::Class(class) => {
        if let Some(ident) = &class.ident {
          self.default_export = Some(ident.sym.clone());
        }
      }
      ast::DefaultDecl::Fn(func) => {
        if let Some(ident) = &func.ident {
          self.default_export = Some(ident.sym.clone());
        }
      }
      ast::DefaultDecl::TsInterfaceDecl(iface_decl) => {
        self.default_export = Some(iface_decl.id.sym.clone());
      }
    }
  }

  fn visit_export_default_expr(
    &mut self,
    export_default_expr: &ast::ExportDefaultExpr,
  ) {
    if let ast::Expr::Ident(ident) = &*export_default_expr.expr {
      self.default_export = Some(ident.sym.clone());
    }
  }

  fn visit_export_named_specifier(
    &mut self,
    export_named_specifier: &ast::ExportNamedSpecifier,
  ) {
    fn get_atom(export_name: &ast::ModuleExportName) -> Atom {
      match export_name {
        ast::ModuleExportName::Ident(ident) => ident.sym.clone(),
        ast::ModuleExportName::Str(s) => s.value.clone(),
      }
    }

    match &export_named_specifier.exported {
      Some(exported) => {
        self.named_exports.insert(get_atom(exported));
      }
      None => {
        self
          .named_exports
          .insert(get_atom(&export_named_specifier.orig));
      }
    }
  }

  fn visit_named_export(&mut self, named_export: &ast::NamedExport) {
    // ExportCollector does not handle re-exports
    if named_export.src.is_some() {
      return;
    }

    named_export.visit_children_with(self);
  }
}

fn extract_sym_from_pat(pat: &ast::Pat) -> Vec<Atom> {
  fn rec(pat: &ast::Pat, atoms: &mut Vec<Atom>) {
    match pat {
      ast::Pat::Ident(binding_ident) => {
        atoms.push(binding_ident.sym.clone());
      }
      ast::Pat::Array(array_pat) => {
        for elem in array_pat.elems.iter().flatten() {
          rec(elem, atoms);
        }
      }
      ast::Pat::Rest(rest_pat) => {
        rec(&rest_pat.arg, atoms);
      }
      ast::Pat::Object(object_pat) => {
        for prop in &object_pat.props {
          match prop {
            ast::ObjectPatProp::Assign(assign_pat_prop) => {
              atoms.push(assign_pat_prop.key.sym.clone());
            }
            ast::ObjectPatProp::KeyValue(key_value_pat_prop) => {
              rec(&key_value_pat_prop.value, atoms);
            }
            ast::ObjectPatProp::Rest(rest_pat) => {
              rec(&rest_pat.arg, atoms);
            }
          }
        }
      }
      ast::Pat::Assign(assign_pat) => {
        rec(&assign_pat.left, atoms);
      }
      ast::Pat::Invalid(_) | ast::Pat::Expr(_) => {}
    }
  }

  let mut atoms = vec![];
  rec(pat, &mut atoms);
  atoms
}

/// Generates a "pseudo" file from a given file by applying the following
/// transformations:
///
/// 1. Injects `import` statements for expoted items from the base file
/// 2. If `wrap_kind` is [`WrapKind::DenoTest`], wraps the content of the file
///    in a `Deno.test` call.
///
/// For example, given a file that looks like:
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
///
/// assertEquals(increment(1), 2);
/// ```
///
/// and the base file (from which the above snippet was extracted):
///
/// ```ts
/// export function increment(n: number): number {
///   return n + 1;
/// }
///
/// export const SOME_CONST = "HELLO";
/// ```
///
/// The generated pseudo test file would look like (if `wrap_in_deno_test` is enabled):
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
/// import { increment, SOME_CONST } from "./base.ts";
///
/// Deno.test("./base.ts$1-3.ts", async () => {
///   assertEquals(increment(1), 2);
/// });
/// ```
///
/// # Edge case 1 - duplicate identifier
///
/// If a given file imports, say, `doSomething` from an external module while
/// the base file exports `doSomething` as well, the generated pseudo test file
/// would end up having two duplciate imports for `doSomething`, causing the
/// duplicate identifier error.
///
/// To avoid this issue, when a given file imports `doSomething`, this takes
/// precedence over the automatic import injection for the base file's
/// `doSomething`. So the generated pseudo test file would look like:
///
/// ```ts
/// import { assertEquals } from "@std/assert/equals";
/// import { doSomething } from "./some_external_module.ts";
///
/// Deno.test("./base.ts$1-3.ts", async () => {
///   assertEquals(doSomething(1), 2);
/// });
/// ```
///
/// # Edge case 2 - exports can't be put inside `Deno.test` blocks
///
/// All exports like `export const foo = 42` must be at the top level of the
/// module, making it impossible to wrap exports in `Deno.test` blocks. For
/// example, when the following code snippet is provided:
///
/// ```ts
/// const logger = createLogger("my-awesome-module");
///
/// export function sum(a: number, b: number): number {
///   logger.debug("sum called");
///   return a + b;
/// }
/// ```
///
/// If we applied the naive transformation to this, the generated pseudo test
/// file would look like:
///
/// ```ts
/// Deno.test("./base.ts$1-7.ts", async () => {
///   const logger = createLogger("my-awesome-module");
///
///   export function sum(a: number, b: number): number {
///     logger.debug("sum called");
///     return a + b;
///   }
/// });
/// ```
///
/// But obviously this violates the rule because `export function sum` is not
/// at the top level of the module.
///
/// To address this issue, the `export` keyword is removed so that the item can
/// stay in the `Deno.test` block's scope:
///
/// ```ts
/// Deno.test("./base.ts$1-7.ts", async () => {
///   const logger = createLogger("my-awesome-module");
///
///   function sum(a: number, b: number): number {
///     logger.debug("sum called");
///     return a + b;
///   }
/// });
/// ```
fn generate_pseudo_file(
  file: File,
  base_file_specifier: &ModuleSpecifier,
  exports: &ExportCollector,
  wrap_kind: WrapKind,
) -> Result<File, AnyError> {
  let file = TextDecodedFile::decode(file)?;

  let parsed = deno_ast::parse_program(deno_ast::ParseParams {
    specifier: file.specifier.clone(),
    text: file.source,
    media_type: file.media_type,
    capture_tokens: false,
    scope_analysis: true,
    maybe_syntax: None,
  })?;

  let top_level_atoms = swc_utils::collect_decls_with_ctxt::<Atom, _>(
    &parsed.program_ref(),
    parsed.top_level_context(),
  );

  let transformed =
    parsed
      .program_ref()
      .to_owned()
      .apply(&mut visit_mut_pass(Transform {
        specifier: &file.specifier,
        base_file_specifier,
        exports_from_base: exports,
        atoms_to_be_excluded_from_import: top_level_atoms,
        wrap_kind,
      }));

  let source = deno_ast::swc::codegen::to_code_with_comments(
    Some(&parsed.comments().as_single_threaded()),
    &transformed,
  );

  log::debug!("{}:\n{}", file.specifier, source);

  Ok(File {
    url: file.specifier,
    maybe_headers: None,
    source: source.into_bytes().into(),
  })
}

struct Transform<'a> {
  specifier: &'a ModuleSpecifier,
  base_file_specifier: &'a ModuleSpecifier,
  exports_from_base: &'a ExportCollector,
  atoms_to_be_excluded_from_import: rustc_hash::FxHashSet<Atom>,
  wrap_kind: WrapKind,
}

impl VisitMut for Transform<'_> {
  fn visit_mut_program(&mut self, node: &mut ast::Program) {
    let new_module_items = match node {
      ast::Program::Module(module) => {
        let mut module_decls = vec![];
        let mut stmts = vec![];

        for item in &module.body {
          match item {
            ast::ModuleItem::ModuleDecl(decl) => match self.wrap_kind {
              WrapKind::NoWrap => {
                module_decls.push(decl.clone());
              }
              // We remove `export` keywords so that they can be put inside
              // `Deno.test` block scope.
              WrapKind::DenoTest => match decl {
                ast::ModuleDecl::ExportDecl(export_decl) => {
                  stmts.push(ast::Stmt::Decl(export_decl.decl.clone()));
                }
                ast::ModuleDecl::ExportDefaultDecl(export_default_decl) => {
                  let stmt = match &export_default_decl.decl {
                    ast::DefaultDecl::Class(class) => {
                      let expr = ast::Expr::Class(class.clone());
                      ast::Stmt::Expr(ast::ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(expr),
                      })
                    }
                    ast::DefaultDecl::Fn(func) => {
                      let expr = ast::Expr::Fn(func.clone());
                      ast::Stmt::Expr(ast::ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(expr),
                      })
                    }
                    ast::DefaultDecl::TsInterfaceDecl(ts_interface_decl) => {
                      ast::Stmt::Decl(ast::Decl::TsInterface(
                        ts_interface_decl.clone(),
                      ))
                    }
                  };
                  stmts.push(stmt);
                }
                ast::ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                  stmts.push(ast::Stmt::Expr(ast::ExprStmt {
                    span: DUMMY_SP,
                    expr: export_default_expr.expr.clone(),
                  }));
                }
                _ => {
                  module_decls.push(decl.clone());
                }
              },
            },
            ast::ModuleItem::Stmt(stmt) => {
              stmts.push(stmt.clone());
            }
          }
        }

        let mut transformed_items = vec![];
        transformed_items
          .extend(module_decls.into_iter().map(ast::ModuleItem::ModuleDecl));
        let import_specifiers = self
          .exports_from_base
          .to_import_specifiers(&self.atoms_to_be_excluded_from_import);
        if !import_specifiers.is_empty() {
          transformed_items.push(ast::ModuleItem::ModuleDecl(
            ast::ModuleDecl::Import(ast::ImportDecl {
              span: DUMMY_SP,
              specifiers: import_specifiers,
              src: Box::new(ast::Str {
                span: DUMMY_SP,
                value: self.base_file_specifier.to_string().into(),
                raw: None,
              }),
              type_only: false,
              with: None,
              phase: ast::ImportPhase::Evaluation,
            }),
          ));
        }
        match self.wrap_kind {
          WrapKind::DenoTest => {
            transformed_items.push(ast::ModuleItem::Stmt(wrap_in_deno_test(
              stmts,
              self.specifier.to_string().into(),
            )));
          }
          WrapKind::NoWrap => {
            transformed_items
              .extend(stmts.into_iter().map(ast::ModuleItem::Stmt));
          }
        }

        transformed_items
      }
      ast::Program::Script(script) => {
        let mut transformed_items = vec![];

        let import_specifiers = self
          .exports_from_base
          .to_import_specifiers(&self.atoms_to_be_excluded_from_import);
        if !import_specifiers.is_empty() {
          transformed_items.push(ast::ModuleItem::ModuleDecl(
            ast::ModuleDecl::Import(ast::ImportDecl {
              span: DUMMY_SP,
              specifiers: import_specifiers,
              src: Box::new(ast::Str {
                span: DUMMY_SP,
                value: self.base_file_specifier.to_string().into(),
                raw: None,
              }),
              type_only: false,
              with: None,
              phase: ast::ImportPhase::Evaluation,
            }),
          ));
        }

        match self.wrap_kind {
          WrapKind::DenoTest => {
            transformed_items.push(ast::ModuleItem::Stmt(wrap_in_deno_test(
              script.body.clone(),
              self.specifier.to_string().into(),
            )));
          }
          WrapKind::NoWrap => {
            transformed_items.extend(
              script.body.clone().into_iter().map(ast::ModuleItem::Stmt),
            );
          }
        }

        transformed_items
      }
    };

    *node = ast::Program::Module(ast::Module {
      span: DUMMY_SP,
      body: new_module_items,
      shebang: None,
    });
  }
}

fn wrap_in_deno_test(stmts: Vec<ast::Stmt>, test_name: Atom) -> ast::Stmt {
  ast::Stmt::Expr(ast::ExprStmt {
    span: DUMMY_SP,
    expr: Box::new(ast::Expr::Call(ast::CallExpr {
      span: DUMMY_SP,
      callee: ast::Callee::Expr(Box::new(ast::Expr::Member(ast::MemberExpr {
        span: DUMMY_SP,
        obj: Box::new(ast::Expr::Ident(ast::Ident {
          span: DUMMY_SP,
          sym: "Deno".into(),
          optional: false,
          ..Default::default()
        })),
        prop: ast::MemberProp::Ident(ast::IdentName {
          span: DUMMY_SP,
          sym: "test".into(),
        }),
      }))),
      args: vec![
        ast::ExprOrSpread {
          spread: None,
          expr: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
            span: DUMMY_SP,
            value: test_name,
            raw: None,
          }))),
        },
        ast::ExprOrSpread {
          spread: None,
          expr: Box::new(ast::Expr::Arrow(ast::ArrowExpr {
            span: DUMMY_SP,
            params: vec![],
            body: Box::new(ast::BlockStmtOrExpr::BlockStmt(ast::BlockStmt {
              span: DUMMY_SP,
              stmts,
              ..Default::default()
            })),
            is_async: true,
            is_generator: false,
            type_params: None,
            return_type: None,
            ..Default::default()
          })),
        },
      ],
      type_args: None,
      ..Default::default()
    })),
  })
}

#[cfg(test)]
mod tests {
  use deno_ast::swc::atoms::Atom;
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::file_fetcher::TextDecodedFile;

  #[test]
  fn test_extract_doc_tests() {
    struct Input {
      source: &'static str,
      specifier: &'static str,
    }
    struct Expected {
      source: &'static str,
      specifier: &'static str,
      media_type: MediaType,
    }
    struct Test {
      input: Input,
      expected: Vec<Expected>,
    }

    let tests = [
      Test {
        input: Input {
          source: r#""#,
          specifier: "file:///main.ts",
        },
        expected: vec![],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equal";
 * 
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "file:///main.ts";
Deno.test("file:///main.ts$3-8.ts", async ()=>{
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///main.ts$3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export default class Bar {}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import Bar, { foo } from "file:///main.ts";
Deno.test("file:///main.ts$3-6.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts$3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const input = { a: 42 } satisfies Args;
 * foo(input);
 * ```
 */
export function foo(args: Args) {}

export type Args = { a: number };
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Args, foo } from "file:///main.ts";
Deno.test("file:///main.ts$3-7.ts", async ()=>{
    const input = {
        a: 42
    } satisfies Args;
    foo(input);
});
"#,
          specifier: "file:///main.ts$3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * This is a module-level doc.
 *
 * ```ts
 * foo();
 * ```
 *
 * @module doc
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"Deno.test("file:///main.ts$5-8.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts$5-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * This is a module-level doc.
 *
 * ```js
 * const cls = new MyClass();
 * ```
 *
 * @module doc
 */

/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export default class MyClass {}

export * from "./other.ts";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![
          Expected {
            source: r#"import MyClass, { foo } from "file:///main.ts";
Deno.test("file:///main.ts$5-8.js", async ()=>{
    const cls = new MyClass();
});
"#,
            specifier: "file:///main.ts$5-8.js",
            media_type: MediaType::JavaScript,
          },
          Expected {
            source: r#"import MyClass, { foo } from "file:///main.ts";
Deno.test("file:///main.ts$13-16.ts", async ()=>{
    foo();
});
"#,
            specifier: "file:///main.ts$13-16.ts",
            media_type: MediaType::TypeScript,
          },
        ],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * foo();
 * ```
 */
export function foo() {}

export const ONE = 1;
const TWO = 2;
export default TWO;
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import TWO, { ONE, foo } from "file:///main.ts";
Deno.test("file:///main.ts$3-6.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts$3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Avoid duplicate imports
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { DUPLICATE1 } from "./other1.ts";
 * import * as DUPLICATE2 from "./other2.js";
 * import { foo as DUPLICATE3 } from "./other3.tsx";
 *
 * foo();
 * ```
 */
export function foo() {}

export const DUPLICATE1 = "dup1";
const DUPLICATE2 = "dup2";
export default DUPLICATE2;
const DUPLICATE3 = "dup3";
export { DUPLICATE3 };
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { DUPLICATE1 } from "./other1.ts";
import * as DUPLICATE2 from "./other2.js";
import { foo as DUPLICATE3 } from "./other3.tsx";
import { foo } from "file:///main.ts";
Deno.test("file:///main.ts$3-10.ts", async ()=>{
    foo();
});
"#,
          specifier: "file:///main.ts$3-10.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // duplication of imported identifier and local identifier is fine
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const foo = createFoo();
 * foo();
 * ```
 */
export function createFoo() {
  return () => "created foo";
}

export const foo = () => "foo";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { createFoo } from "file:///main.ts";
Deno.test("file:///main.ts$3-7.ts", async ()=>{
    const foo = createFoo();
    foo();
});
"#,
          specifier: "file:///main.ts$3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/25718
      // A case where the example code has an exported item which references
      // a variable from one upper scope.
      // Naive application of `Deno.test` wrap would cause a reference error
      // because the variable would go inside the `Deno.test` block while the
      // exported item would be moved to the top level. To suppress the auto
      // move of the exported item to the top level, the `export` keyword is
      // removed so that the item stays in the same scope as the variable.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { getLogger } from "@std/log";
 *
 * const logger = getLogger("my-awesome-module");
 *
 * export function foo() {
 *   logger.debug("hello");
 * }
 * ```
 *
 * @module
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { getLogger } from "@std/log";
Deno.test("file:///main.ts$3-12.ts", async ()=>{
    const logger = getLogger("my-awesome-module");
    function foo() {
        logger.debug("hello");
    }
});
"#,
          specifier: "file:///main.ts$3-12.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
# Header

This is a *markdown*.

```js
import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";

assertEquals(add(1, 2), 3);
```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";
Deno.test("file:///README.md$6-12.js", async ()=>{
    assertEquals(add(1, 2), 3);
});
"#,
          specifier: "file:///README.md$6-12.js",
          media_type: MediaType::JavaScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26009
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * console.log(Foo)
 * ```
 */
export class Foo {}
export default Foo
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Foo } from "file:///main.ts";
Deno.test("file:///main.ts$3-6.ts", async ()=>{
    console.log(Foo);
});
"#,
          specifier: "file:///main.ts$3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26728
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * // @ts-expect-error: can only add numbers
 * add('1', '2');
 * ```
 */
export function add(first: number, second: number) {
  return first + second;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { add } from "file:///main.ts";
Deno.test("file:///main.ts$3-7.ts", async ()=>{
    // @ts-expect-error: can only add numbers
    add('1', '2');
});
"#,
          specifier: "file:///main.ts$3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
    ];

    for test in tests {
      let file = File {
        url: ModuleSpecifier::parse(test.input.specifier).unwrap(),
        maybe_headers: None,
        source: test.input.source.as_bytes().into(),
      };
      let got_decoded = extract_doc_tests(file)
        .unwrap()
        .into_iter()
        .map(|f| TextDecodedFile::decode(f).unwrap())
        .collect::<Vec<_>>();
      let expected = test
        .expected
        .iter()
        .map(|e| TextDecodedFile {
          specifier: ModuleSpecifier::parse(e.specifier).unwrap(),
          media_type: e.media_type,
          source: e.source.into(),
        })
        .collect::<Vec<_>>();
      assert_eq!(got_decoded, expected);
    }
  }

  #[test]
  fn test_extract_snippet_files() {
    struct Input {
      source: &'static str,
      specifier: &'static str,
    }
    struct Expected {
      source: &'static str,
      specifier: &'static str,
      media_type: MediaType,
    }
    struct Test {
      input: Input,
      expected: Vec<Expected>,
    }

    let tests = [
      Test {
        input: Input {
          source: r#""#,
          specifier: "file:///main.ts",
        },
        expected: vec![],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { add } from "file:///main.ts";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///main.ts$3-8.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 * import { DUPLICATE } from "./other.ts";
 *
 * assertEquals(add(1, 2), 3);
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}

export const DUPLICATE = "dup";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equals";
import { DUPLICATE } from "./other.ts";
import { add } from "file:///main.ts";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///main.ts$3-9.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // If the snippet has a local variable with the same name as an exported
      // item, the local variable takes precedence.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * const foo = createFoo();
 * foo();
 * ```
 */
export function createFoo() {
  return () => "created foo";
}

export const foo = () => "foo";
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { createFoo } from "file:///main.ts";
const foo = createFoo();
foo();
"#,
          specifier: "file:///main.ts$3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // Unlike `extract_doc_tests`, `extract_snippet_files` does not remove
      // the `export` keyword from the exported items.
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * import { getLogger } from "@std/log";
 *
 * const logger = getLogger("my-awesome-module");
 *
 * export function foo() {
 *   logger.debug("hello");
 * }
 * ```
 *
 * @module
 */
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { getLogger } from "@std/log";
export function foo() {
    logger.debug("hello");
}
const logger = getLogger("my-awesome-module");
"#,
          specifier: "file:///main.ts$3-12.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      Test {
        input: Input {
          source: r#"
# Header

This is a *markdown*.

```js
import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";

assertEquals(add(1, 2), 3);
```
"#,
          specifier: "file:///README.md",
        },
        expected: vec![Expected {
          source: r#"import { assertEquals } from "@std/assert/equal";
import { add } from "jsr:@deno/non-existent";
assertEquals(add(1, 2), 3);
"#,
          specifier: "file:///README.md$6-12.js",
          media_type: MediaType::JavaScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26009
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * console.log(Foo)
 * ```
 */
export class Foo {}
export default Foo
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { Foo } from "file:///main.ts";
console.log(Foo);
"#,
          specifier: "file:///main.ts$3-6.ts",
          media_type: MediaType::TypeScript,
        }],
      },
      // https://github.com/denoland/deno/issues/26728
      Test {
        input: Input {
          source: r#"
/**
 * ```ts
 * // @ts-expect-error: can only add numbers
 * add('1', '2');
 * ```
 */
export function add(first: number, second: number) {
  return first + second;
}
"#,
          specifier: "file:///main.ts",
        },
        expected: vec![Expected {
          source: r#"import { add } from "file:///main.ts";
// @ts-expect-error: can only add numbers
add('1', '2');
"#,
          specifier: "file:///main.ts$3-7.ts",
          media_type: MediaType::TypeScript,
        }],
      },
    ];

    for test in tests {
      let file = File {
        url: ModuleSpecifier::parse(test.input.specifier).unwrap(),
        maybe_headers: None,
        source: test.input.source.as_bytes().into(),
      };
      let got_decoded = extract_snippet_files(file)
        .unwrap()
        .into_iter()
        .map(|f| TextDecodedFile::decode(f).unwrap())
        .collect::<Vec<_>>();
      let expected = test
        .expected
        .iter()
        .map(|e| TextDecodedFile {
          specifier: ModuleSpecifier::parse(e.specifier).unwrap(),
          media_type: e.media_type,
          source: e.source.into(),
        })
        .collect::<Vec<_>>();
      assert_eq!(got_decoded, expected);
    }
  }

  #[test]
  fn test_export_collector() {
    fn helper(input: &'static str) -> ExportCollector {
      let mut collector = ExportCollector::default();
      let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: deno_ast::ModuleSpecifier::parse("file:///main.ts").unwrap(),
        text: input.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })
      .unwrap();

      parsed.program_ref().visit_with(&mut collector);
      collector
    }

    struct Test {
      input: &'static str,
      named_expected: BTreeSet<Atom>,
      default_expected: Option<Atom>,
    }

    macro_rules! atom_set {
      ($( $x:expr ),*) => {
        [$( Atom::from($x) ),*].into_iter().collect::<BTreeSet<_>>()
      };
    }

    let tests = [
      Test {
        input: r#"export const foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export let foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export var foo = 42;"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export const foo = () => {};"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export function foo() {}"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export class Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export enum Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export module Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export module "foo" {}"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      Test {
        input: r#"export namespace Foo {}"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export type Foo = string;"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export interface Foo {};"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"export let name1, name2;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export const name1 = 1, name2 = 2;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export function* generatorFunc() {}"#,
        named_expected: atom_set!("generatorFunc"),
        default_expected: None,
      },
      Test {
        input: r#"export const { name1, name2: bar } = obj;"#,
        named_expected: atom_set!("name1", "bar"),
        default_expected: None,
      },
      Test {
        input: r#"export const [name1, name2] = arr;"#,
        named_expected: atom_set!("name1", "name2"),
        default_expected: None,
      },
      Test {
        input: r#"export const { name1 = 42 } = arr;"#,
        named_expected: atom_set!("name1"),
        default_expected: None,
      },
      Test {
        input: r#"export default function foo() {}"#,
        named_expected: atom_set!(),
        default_expected: Some("foo".into()),
      },
      Test {
        input: r#"export default class Foo {}"#,
        named_expected: atom_set!(),
        default_expected: Some("Foo".into()),
      },
      Test {
        input: r#"export default interface Foo {}"#,
        named_expected: atom_set!(),
        default_expected: Some("Foo".into()),
      },
      Test {
        input: r#"const foo = 42; export default foo;"#,
        named_expected: atom_set!(),
        default_expected: Some("foo".into()),
      },
      Test {
        input: r#"export { foo, bar as barAlias };"#,
        named_expected: atom_set!("foo", "barAlias"),
        default_expected: None,
      },
      Test {
        input: r#"
export default class Foo {}
export let value1 = 42;
const value2 = "Hello";
const value3 = "World";
export { value2 };
"#,
        named_expected: atom_set!("value1", "value2"),
        default_expected: Some("Foo".into()),
      },
      // overloaded function
      Test {
        input: r#"
export function foo(a: number): boolean;
export function foo(a: boolean): string;
export function foo(a: number | boolean): boolean | string {
  return typeof a === "number" ? true : "hello";
}
"#,
        named_expected: atom_set!("foo"),
        default_expected: None,
      },
      // The collector deliberately does not handle re-exports, because from
      // doc reader's perspective, an example code would become hard to follow
      // if it uses re-exported items (as opposed to normal, non-re-exported
      // items that would look verbose if an example code explicitly imports
      // them).
      Test {
        input: r#"
export * from "./module1.ts";
export * as name1 from "./module2.ts";
export { name2, name3 as N3 } from "./module3.js";
export { default } from "./module4.ts";
export { default as myDefault } from "./module5.ts";
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      Test {
        input: r#"
export namespace Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!("Foo"),
        default_expected: None,
      },
      Test {
        input: r#"
declare namespace Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      Test {
        input: r#"
declare module Foo {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      Test {
        input: r#"
declare global {
  export type MyType = string;
  export const myValue = 42;
  export function myFunc(): boolean;
}
"#,
        named_expected: atom_set!(),
        default_expected: None,
      },
      // The identifier `Foo` conflicts, but `ExportCollector` doesn't do
      // anything about it. It is handled by `to_import_specifiers` method.
      Test {
        input: r#"
export class Foo {}
export default Foo
"#,
        named_expected: atom_set!("Foo"),
        default_expected: Some("Foo".into()),
      },
    ];

    for test in tests {
      let got = helper(test.input);
      assert_eq!(got.named_exports, test.named_expected);
      assert_eq!(got.default_export, test.default_expected);
    }
  }
}
