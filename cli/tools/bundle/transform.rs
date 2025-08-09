// Copyright 2018-2025 the Deno authors. MIT license.

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
