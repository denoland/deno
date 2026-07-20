// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::swc;
use deno_ast::swc::ast::Bool;
use deno_ast::swc::ecma_visit::VisitMut;
use deno_ast::swc::ecma_visit::VisitMutWith;

pub struct BundleImportMetaMainTransform {
  is_entrypoint: bool,
}

impl BundleImportMetaMainTransform {
  pub fn new(is_entrypoint: bool) -> Self {
    Self { is_entrypoint }
  }
}

/// Rewrites raw imports (`import x from "./f" with { type: "text" }`) to a
/// query-suffixed specifier (`./f?deno-raw-text`) and drops the attribute
/// clause. Rolldown dedups modules by resolved id, so without a distinct id
/// a file imported both normally and as text/bytes would collapse into one
/// module. The resolve/load hooks recognize the suffix and inline the raw
/// contents. `type: "json"` imports are left alone — `.json` files already
/// load as JSON modules.
#[derive(Default)]
pub struct RawImportsTransform {
  pub rewrote_any: bool,
}

fn raw_import_type(with: &swc::ast::ObjectLit) -> Option<&'static str> {
  for prop in &with.props {
    let swc::ast::PropOrSpread::Prop(prop) = prop else {
      continue;
    };
    let swc::ast::Prop::KeyValue(kv) = &**prop else {
      continue;
    };
    let key_matches = match &kv.key {
      swc::ast::PropName::Ident(ident) => ident.sym.as_str() == "type",
      swc::ast::PropName::Str(s) => s.value.as_str() == Some("type"),
      _ => false,
    };
    if !key_matches {
      continue;
    }
    if let swc::ast::Expr::Lit(swc::ast::Lit::Str(value)) = &*kv.value {
      return match value.value.as_str() {
        Some("text") => Some("text"),
        Some("bytes") => Some("bytes"),
        Some("css") => Some("css"),
        _ => None,
      };
    }
  }
  None
}

impl VisitMut for RawImportsTransform {
  fn visit_mut_import_decl(&mut self, node: &mut swc::ast::ImportDecl) {
    if let Some(with) = &node.with
      && let Some(raw_type) = raw_import_type(with)
    {
      let new_src =
        format!("{}?deno-raw-{}", node.src.value.to_string_lossy(), raw_type);
      node.src.value = new_src.into();
      node.src.raw = None;
      node.with = None;
      self.rewrote_any = true;
    }
    node.visit_mut_children_with(self);
  }
}

impl VisitMut for BundleImportMetaMainTransform {
  fn visit_mut_expr(&mut self, node: &mut swc::ast::Expr) {
    // if entrypoint to bundle:
    //   import.meta.main => import.meta.main
    // else:
    //   import.meta.main => false
    if let swc::ast::Expr::Member(member) = node
      && let swc::ast::Expr::MetaProp(meta_prop) = &mut *member.obj
      && meta_prop.kind == swc::ast::MetaPropKind::ImportMeta
      && member.prop.is_ident_with("main")
    {
      if self.is_entrypoint {
        return;
      } else {
        let span = member.span;
        *node =
          swc::ast::Expr::Lit(swc::ast::Lit::Bool(Bool { span, value: false }));
        return;
      }
    }
    node.visit_mut_children_with(self);
  }
}
