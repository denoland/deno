// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use swc_common::Spanned;

use super::parser::DocParser;
use super::DocNode;

pub fn get_doc_node_for_export_decl(
  doc_parser: &DocParser,
  export_decl: &swc_ecmascript::ast::ExportDecl,
) -> DocNode {
  let export_span = export_decl.span();
  use swc_ecmascript::ast::Decl;

  let js_doc = doc_parser.js_doc_for_span(export_span);
  let location = doc_parser.ast_parser.get_span_location(export_span).into();

  match &export_decl.decl {
    Decl::Class(class_decl) => {
      let (name, class_def) =
        super::class::get_doc_for_class_decl(doc_parser, class_decl);
      DocNode::class(name, location, js_doc, class_def)
    }
    Decl::Fn(fn_decl) => {
      let (name, function_def) =
        super::function::get_doc_for_fn_decl(doc_parser, fn_decl);
      DocNode::function(name, location, js_doc, function_def)
    }
    Decl::Var(var_decl) => {
      let (name, var_def) = super::variable::get_doc_for_var_decl(var_decl);
      DocNode::variable(name, location, js_doc, var_def)
    }
    Decl::TsInterface(ts_interface_decl) => {
      let (name, interface_def) =
        super::interface::get_doc_for_ts_interface_decl(
          doc_parser,
          ts_interface_decl,
        );
      DocNode::interface(name, location, js_doc, interface_def)
    }
    Decl::TsTypeAlias(ts_type_alias) => {
      let (name, type_alias_def) =
        super::type_alias::get_doc_for_ts_type_alias_decl(
          doc_parser,
          ts_type_alias,
        );
      DocNode::type_alias(name, location, js_doc, type_alias_def)
    }
    Decl::TsEnum(ts_enum) => {
      let (name, enum_def) =
        super::r#enum::get_doc_for_ts_enum_decl(doc_parser, ts_enum);
      DocNode::r#enum(name, location, js_doc, enum_def)
    }
    Decl::TsModule(ts_module) => {
      let (name, namespace_def) =
        super::namespace::get_doc_for_ts_module(doc_parser, ts_module);
      DocNode::namespace(name, location, js_doc, namespace_def)
    }
  }
}
