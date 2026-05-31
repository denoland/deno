// Copyright 2018-2026 the Deno authors. MIT license.

//! Registry of documentation comments extracted from bundled `.d.ts`
//! files. Supports the REPL `.doc` command and `doc()` helper.

use std::collections::HashMap;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::ParsedSource;
use deno_ast::SourcePos;
use deno_ast::SourceRangedForSpanned;
use deno_ast::swc::ast as swc_ast;
use deno_ast::swc::common::comments::Comment;
use deno_ast::swc::common::comments::CommentKind;
use deno_core::error::AnyError;

use crate::tsc::LAZILY_LOADED_STATIC_ASSETS;

/// The .d.ts library files that are scanned for documentation. We keep the
/// list intentionally small so REPL startup stays cheap — and so we can
/// reason about the keys we produce.
const DOC_LIB_FILES: &[&str] = &[
  "lib.deno.ns.d.ts",
  "lib.deno.unstable.d.ts",
  "lib.deno.shared_globals.d.ts",
  "lib.deno.window.d.ts",
  "lib.deno_web.d.ts",
  "lib.deno_url.d.ts",
  "lib.deno_console.d.ts",
  "lib.deno_fetch.d.ts",
  "lib.deno_net.d.ts",
  "lib.deno_crypto.d.ts",
];

/// A documented symbol — keyed by its dotted path (e.g. "Deno.openSync",
/// "Deno.stdin.read", "URL.prototype.toString").
#[derive(Debug, Clone)]
pub struct DocEntry {
  /// The raw JSDoc comment text, including leading `*` markers.
  pub jsdoc: String,
}

static REGISTRY: OnceLock<HashMap<String, DocEntry>> = OnceLock::new();

/// Look up documentation for a dotted path. Builds the registry on first
/// call. Returns `None` if no doc is recorded for that path.
pub fn lookup(path: &str) -> Option<&'static DocEntry> {
  let registry = REGISTRY.get_or_init(build_registry);
  registry.get(path)
}

/// Strip a JSDoc comment of its leading `*` decorations and surrounding
/// whitespace so it can be printed back to a terminal in a readable form.
pub fn format_jsdoc(jsdoc: &str) -> String {
  let mut lines: Vec<String> = Vec::new();
  for line in jsdoc.lines() {
    let trimmed = line.trim_start();
    let trimmed = trimmed.strip_prefix('*').unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix(' ').unwrap_or(trimmed);
    lines.push(trimmed.trim_end().to_string());
  }
  // Drop leading/trailing empty lines.
  while lines.first().map(|s| s.is_empty()).unwrap_or(false) {
    lines.remove(0);
  }
  while lines.last().map(|s| s.is_empty()).unwrap_or(false) {
    lines.pop();
  }
  lines.join("\n")
}

fn build_registry() -> HashMap<String, DocEntry> {
  let mut out = HashMap::new();
  for lib in DOC_LIB_FILES {
    let Some(asset) = LAZILY_LOADED_STATIC_ASSETS.get(lib) else {
      continue;
    };
    let source = asset.source.as_str();
    let Ok(parsed) = parse_dts(lib, source) else {
      continue;
    };
    collect_from_source(&parsed, &mut out);
  }
  out
}

fn parse_dts(name: &str, source: &str) -> Result<ParsedSource, AnyError> {
  let specifier =
    ModuleSpecifier::parse(&format!("deno:repl-docs/{name}")).unwrap();
  let parsed = deno_ast::parse_module(ParseParams {
    specifier,
    text: source.to_string().into(),
    media_type: MediaType::Dts,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })?;
  Ok(parsed)
}

fn collect_from_source(
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  let module = parsed.program_ref();
  let body = match module {
    deno_ast::ProgramRef::Module(m) => &m.body[..],
    deno_ast::ProgramRef::Script(s) => {
      // Wrap script statements as module items via a separate path.
      collect_from_stmts(&s.body, "", parsed, out);
      return;
    }
  };
  for item in body {
    collect_from_module_item(item, "", parsed, out);
  }
}

fn collect_from_stmts(
  stmts: &[swc_ast::Stmt],
  path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  for stmt in stmts {
    if let swc_ast::Stmt::Decl(decl) = stmt {
      collect_from_decl(decl, None, path, parsed, out);
    }
  }
}

fn collect_from_module_item(
  item: &swc_ast::ModuleItem,
  path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  match item {
    swc_ast::ModuleItem::ModuleDecl(swc_ast::ModuleDecl::ExportDecl(e)) => {
      // JSDoc comments in d.ts files are usually attached to the outer
      // `export` keyword, not the inner declaration. Pass the export's
      // start position as a fallback.
      collect_from_decl(&e.decl, Some(e.start()), path, parsed, out);
    }
    swc_ast::ModuleItem::ModuleDecl(_) => {}
    swc_ast::ModuleItem::Stmt(stmt) => {
      if let swc_ast::Stmt::Decl(decl) = stmt {
        collect_from_decl(decl, None, path, parsed, out);
      }
    }
  }
}

fn join_path(parent: &str, child: &str) -> String {
  if parent.is_empty() {
    child.to_string()
  } else {
    format!("{parent}.{child}")
  }
}

fn collect_from_decl(
  decl: &swc_ast::Decl,
  outer_start: Option<SourcePos>,
  path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  match decl {
    swc_ast::Decl::Fn(fn_decl) => {
      let key = join_path(path, fn_decl.ident.sym.as_ref());
      record_decl_doc(&key, fn_decl.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
    }
    swc_ast::Decl::Var(var_decl) => {
      for (idx, declarator) in var_decl.decls.iter().enumerate() {
        if let swc_ast::Pat::Ident(binding) = &declarator.name {
          let name = binding.id.sym.as_ref();
          let key = join_path(path, name);
          record_decl_doc(&key, declarator.start(), parsed, out);
          // Also try the parent VarDecl span for the JSDoc, since the
          // VarDeclarator's leading comments are typically attached to
          // the parent declaration in d.ts files.
          record_decl_doc(&key, var_decl.start(), parsed, out);
          if idx == 0
            && let Some(start) = outer_start
          {
            record_decl_doc(&key, start, parsed, out);
          }
          if let Some(type_ann) = &binding.type_ann {
            walk_ts_type(&type_ann.type_ann, &key, parsed, out);
          }
        }
      }
    }
    swc_ast::Decl::Class(class_decl) => {
      let key = join_path(path, class_decl.ident.sym.as_ref());
      record_decl_doc(&key, class_decl.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
      let proto_key = format!("{key}.prototype");
      for member in &class_decl.class.body {
        walk_class_member(member, &key, &proto_key, parsed, out);
      }
    }
    swc_ast::Decl::TsInterface(iface) => {
      let key = join_path(path, iface.id.sym.as_ref());
      record_decl_doc(&key, iface.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
      let proto_key = format!("{key}.prototype");
      for elem in &iface.body.body {
        walk_ts_type_element(elem, &key, &proto_key, parsed, out);
      }
    }
    swc_ast::Decl::TsTypeAlias(alias) => {
      let key = join_path(path, alias.id.sym.as_ref());
      record_decl_doc(&key, alias.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
    }
    swc_ast::Decl::TsModule(module) => {
      let name = match &module.id {
        swc_ast::TsModuleName::Ident(i) => i.sym.as_ref().to_string(),
        swc_ast::TsModuleName::Str(s) => s.value.to_string_lossy().to_string(),
      };
      let key = join_path(path, &name);
      record_decl_doc(&key, module.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
      if let Some(body) = &module.body {
        walk_namespace_body(body, &key, parsed, out);
      }
    }
    swc_ast::Decl::TsEnum(ts_enum) => {
      let key = join_path(path, ts_enum.id.sym.as_ref());
      record_decl_doc(&key, ts_enum.start(), parsed, out);
      if let Some(start) = outer_start {
        record_decl_doc(&key, start, parsed, out);
      }
    }
    swc_ast::Decl::Using(_) => {}
  }
}

fn walk_namespace_body(
  body: &swc_ast::TsNamespaceBody,
  path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  match body {
    swc_ast::TsNamespaceBody::TsModuleBlock(block) => {
      for item in &block.body {
        collect_from_module_item(item, path, parsed, out);
      }
    }
    swc_ast::TsNamespaceBody::TsNamespaceDecl(decl) => {
      let key = join_path(path, decl.id.sym.as_ref());
      record_decl_doc(&key, decl.start(), parsed, out);
      walk_namespace_body(&decl.body, &key, parsed, out);
    }
  }
}

fn walk_ts_type(
  ts_type: &swc_ast::TsType,
  path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  // We only descend into type literals (e.g. `const stdin: { read(...) }`).
  // Walking deeper through unions/intersections gives diminishing returns.
  if let swc_ast::TsType::TsTypeLit(lit) = ts_type {
    for elem in &lit.members {
      walk_ts_type_element(elem, path, path, parsed, out);
    }
  }
}

fn walk_ts_type_element(
  elem: &swc_ast::TsTypeElement,
  instance_path: &str,
  proto_path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  let (name, span_start): (String, SourcePos) = match elem {
    swc_ast::TsTypeElement::TsPropertySignature(p) => {
      let Some(name) = expr_to_ident(&p.key) else {
        return;
      };
      (name, p.start())
    }
    swc_ast::TsTypeElement::TsGetterSignature(g) => {
      let Some(name) = expr_to_ident(&g.key) else {
        return;
      };
      (name, g.start())
    }
    swc_ast::TsTypeElement::TsSetterSignature(s) => {
      let Some(name) = expr_to_ident(&s.key) else {
        return;
      };
      (name, s.start())
    }
    swc_ast::TsTypeElement::TsMethodSignature(m) => {
      let Some(name) = expr_to_ident(&m.key) else {
        return;
      };
      (name, m.start())
    }
    _ => return,
  };

  let key = join_path(instance_path, &name);
  record_decl_doc(&key, span_start, parsed, out);
  if instance_path != proto_path {
    let proto_key = join_path(proto_path, &name);
    record_decl_doc(&proto_key, span_start, parsed, out);
  }

  if let swc_ast::TsTypeElement::TsPropertySignature(p) = elem
    && let Some(type_ann) = &p.type_ann
  {
    walk_ts_type(&type_ann.type_ann, &key, parsed, out);
  }
}

fn walk_class_member(
  member: &swc_ast::ClassMember,
  class_path: &str,
  proto_path: &str,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  let (name, is_static, span_start): (String, bool, _) = match member {
    swc_ast::ClassMember::Method(m) => {
      let Some(name) = property_name_to_ident(&m.key) else {
        return;
      };
      (name, m.is_static, m.start())
    }
    swc_ast::ClassMember::ClassProp(p) => {
      let Some(name) = property_name_to_ident(&p.key) else {
        return;
      };
      (name, p.is_static, p.start())
    }
    swc_ast::ClassMember::AutoAccessor(a) => {
      let key = match &a.key {
        swc_ast::Key::Public(prop) => property_name_to_ident(prop),
        swc_ast::Key::Private(_) => return,
      };
      let Some(name) = key else {
        return;
      };
      (name, a.is_static, a.start())
    }
    _ => return,
  };
  let parent = if is_static { class_path } else { proto_path };
  let key = join_path(parent, &name);
  record_decl_doc(&key, span_start, parsed, out);
}

fn property_name_to_ident(name: &swc_ast::PropName) -> Option<String> {
  match name {
    swc_ast::PropName::Ident(i) => Some(i.sym.as_ref().to_string()),
    swc_ast::PropName::Str(s) => Some(s.value.to_string_lossy().to_string()),
    _ => None,
  }
}

fn expr_to_ident(expr: &swc_ast::Expr) -> Option<String> {
  match expr {
    swc_ast::Expr::Ident(i) => Some(i.sym.as_ref().to_string()),
    swc_ast::Expr::Lit(swc_ast::Lit::Str(s)) => {
      Some(s.value.to_string_lossy().to_string())
    }
    _ => None,
  }
}

fn record_decl_doc(
  key: &str,
  span_start: SourcePos,
  parsed: &ParsedSource,
  out: &mut HashMap<String, DocEntry>,
) {
  if out.contains_key(key) {
    return;
  }
  let Some(comment) = leading_jsdoc_block(parsed, span_start) else {
    return;
  };
  out.insert(key.to_string(), DocEntry { jsdoc: comment });
}

fn leading_jsdoc_block(
  parsed: &ParsedSource,
  pos: SourcePos,
) -> Option<String> {
  let comments = parsed.comments().get_leading(pos)?;
  for comment in comments.iter().rev() {
    if is_jsdoc(comment) {
      return Some(comment.text.to_string());
    }
  }
  None
}

fn is_jsdoc(comment: &Comment) -> bool {
  comment.kind == CommentKind::Block && comment.text.starts_with('*')
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn registry_contains_common_deno_apis() {
    // Known entries from lib.deno.ns.d.ts
    assert!(lookup("Deno.openSync").is_some());
    assert!(lookup("Deno.stdin").is_some());
    assert!(lookup("Deno.stdin.read").is_some());
  }

  #[test]
  fn format_jsdoc_strips_stars_and_indent() {
    let raw = "*\n * Hello\n * @param x foo\n ";
    let formatted = format_jsdoc(raw);
    assert_eq!(formatted, "Hello\n@param x foo");
  }
}
