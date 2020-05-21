// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_common::Spanned;
use crate::swc_ecma_ast;

use super::parser::DocParser;
use super::DocNode;
use super::DocNodeKind;

pub fn get_doc_node_for_export_decl(
  doc_parser: &DocParser,
  export_decl: &swc_ecma_ast::ExportDecl,
) -> DocNode {
  let export_span = export_decl.span();
  use crate::swc_ecma_ast::Decl;

  let js_doc = doc_parser.js_doc_for_span(export_span);
  let location = doc_parser.ast_parser.get_span_location(export_span).into();

  match &export_decl.decl {
    Decl::Class(class_decl) => {
      let (name, class_def) =
        super::class::get_doc_for_class_decl(doc_parser, class_decl);
      DocNode {
        kind: DocNodeKind::Class,
        name,
        location,
        js_doc,
        class_def: Some(class_def),
        function_def: None,
        variable_def: None,
        enum_def: None,
        type_alias_def: None,
        namespace_def: None,
        interface_def: None,
      }
    }
    Decl::Fn(fn_decl) => {
      let (name, function_def) = super::function::get_doc_for_fn_decl(fn_decl);
      DocNode {
        kind: DocNodeKind::Function,
        name,
        location,
        js_doc,
        function_def: Some(function_def),
        class_def: None,
        variable_def: None,
        enum_def: None,
        type_alias_def: None,
        namespace_def: None,
        interface_def: None,
      }
    }
    Decl::Var(var_decl) => {
      let (name, var_def) = super::variable::get_doc_for_var_decl(var_decl);
      DocNode {
        kind: DocNodeKind::Variable,
        name,
        location,
        js_doc,
        variable_def: Some(var_def),
        function_def: None,
        class_def: None,
        enum_def: None,
        type_alias_def: None,
        namespace_def: None,
        interface_def: None,
      }
    }
    Decl::TsInterface(ts_interface_decl) => {
      let (name, interface_def) =
        super::interface::get_doc_for_ts_interface_decl(
          doc_parser,
          ts_interface_decl,
        );
      DocNode {
        kind: DocNodeKind::Interface,
        name,
        location,
        js_doc,
        interface_def: Some(interface_def),
        variable_def: None,
        function_def: None,
        class_def: None,
        enum_def: None,
        type_alias_def: None,
        namespace_def: None,
      }
    }
    Decl::TsTypeAlias(ts_type_alias) => {
      let (name, type_alias_def) =
        super::type_alias::get_doc_for_ts_type_alias_decl(
          doc_parser,
          ts_type_alias,
        );
      DocNode {
        kind: DocNodeKind::TypeAlias,
        name,
        location,
        js_doc,
        type_alias_def: Some(type_alias_def),
        interface_def: None,
        variable_def: None,
        function_def: None,
        class_def: None,
        enum_def: None,
        namespace_def: None,
      }
    }
    Decl::TsEnum(ts_enum) => {
      let (name, enum_def) =
        super::r#enum::get_doc_for_ts_enum_decl(doc_parser, ts_enum);
      DocNode {
        kind: DocNodeKind::Enum,
        name,
        location,
        js_doc,
        enum_def: Some(enum_def),
        type_alias_def: None,
        interface_def: None,
        variable_def: None,
        function_def: None,
        class_def: None,
        namespace_def: None,
      }
    }
    Decl::TsModule(ts_module) => {
      let (name, namespace_def) =
        super::namespace::get_doc_for_ts_module(doc_parser, ts_module);
      DocNode {
        kind: DocNodeKind::Namespace,
        name,
        location,
        js_doc,
        namespace_def: Some(namespace_def),
        enum_def: None,
        type_alias_def: None,
        interface_def: None,
        variable_def: None,
        function_def: None,
        class_def: None,
      }
    }
  }
}
