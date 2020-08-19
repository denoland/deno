// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;

use super::parser::DocParser;
use super::DocNode;

#[derive(Debug, Serialize, Clone)]
pub struct NamespaceDef {
  pub elements: Vec<DocNode>,
}

pub fn get_doc_for_ts_namespace_decl(
  doc_parser: &DocParser,
  ts_namespace_decl: &swc_ecmascript::ast::TsNamespaceDecl,
) -> DocNode {
  let js_doc = doc_parser.js_doc_for_span(ts_namespace_decl.span);
  let location = doc_parser
    .ast_parser
    .get_span_location(ts_namespace_decl.span)
    .into();
  let namespace_name = ts_namespace_decl.id.sym.to_string();

  use swc_ecmascript::ast::TsNamespaceBody::*;

  let elements = match &*ts_namespace_decl.body {
    TsModuleBlock(ts_module_block) => {
      doc_parser.get_doc_nodes_for_module_body(ts_module_block.body.clone())
    }
    TsNamespaceDecl(ts_namespace_decl) => {
      vec![get_doc_for_ts_namespace_decl(doc_parser, ts_namespace_decl)]
    }
  };

  let ns_def = NamespaceDef { elements };

  DocNode::namespace(namespace_name, location, js_doc, ns_def)
}

pub fn get_doc_for_ts_module(
  doc_parser: &DocParser,
  ts_module_decl: &swc_ecmascript::ast::TsModuleDecl,
) -> (String, NamespaceDef) {
  use swc_ecmascript::ast::TsModuleName;
  let namespace_name = match &ts_module_decl.id {
    TsModuleName::Ident(ident) => ident.sym.to_string(),
    TsModuleName::Str(str_) => str_.value.to_string(),
  };

  let elements = if let Some(body) = &ts_module_decl.body {
    use swc_ecmascript::ast::TsNamespaceBody::*;

    match &body {
      TsModuleBlock(ts_module_block) => {
        doc_parser.get_doc_nodes_for_module_body(ts_module_block.body.clone())
      }
      TsNamespaceDecl(ts_namespace_decl) => {
        vec![get_doc_for_ts_namespace_decl(doc_parser, ts_namespace_decl)]
      }
    }
  } else {
    vec![]
  };

  let ns_def = NamespaceDef { elements };

  (namespace_name, ns_def)
}
