// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_common::Spanned;
use crate::swc_ecma_ast;

use super::namespace::NamespaceDef;
use super::parser::DocParser;
use super::DocNode;
use super::DocNodeKind;
use super::Location;

pub fn get_doc_node_for_export_decl(
  doc_parser: &DocParser,
  export_decl: &swc_ecma_ast::ExportDecl,
) -> DocNode {
  let export_span = export_decl.span();
  use crate::swc_ecma_ast::Decl;

  let js_doc = doc_parser.js_doc_for_span(export_span);
  let location = doc_parser
    .source_map
    .lookup_char_pos(export_span.lo())
    .into();

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
      let (name, function_def) =
        super::function::get_doc_for_fn_decl(doc_parser, fn_decl);
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
      let (name, var_def) =
        super::variable::get_doc_for_var_decl(doc_parser, var_decl);
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

#[allow(unused)]
pub fn get_doc_nodes_for_named_export(
  doc_parser: &DocParser,
  named_export: &swc_ecma_ast::NamedExport,
  referrer: &str,
) -> Vec<DocNode> {
  let file_name = named_export.src.as_ref().expect("").value.to_string();
  let resolved_specifier = doc_parser
    .loader
    .resolve(&file_name, referrer, false)
    .expect("Failed to resolve specifier");

  let mut reexported_doc_nodes: Vec<DocNode> = vec![];

  // Now parse that module, but skip parsing its reexports
  let doc_nodes = doc_parser
    .new_parse(&file_name, false)
    .expect("Failed to print docs");

  // Filter out not needed nodes and then
  // rename nodes if needed.
  for export_specifier in &named_export.specifiers {
    use crate::swc_ecma_ast::ExportSpecifier::*;

    match export_specifier {
      Named(named_export_specifier) => {
        let original_name = named_export_specifier.orig.sym.to_string();

        let mut named_export_node = doc_nodes
          .iter()
          .find(|doc_node| doc_node.name == original_name)
          .expect("Node module not found")
          .clone();

        if let Some(alias) = &named_export_specifier.exported {
          named_export_node.name = alias.sym.to_string();
        }

        reexported_doc_nodes.push(named_export_node);
      }
      Namespace(ns_export_specifier) => {
        let ns_name = ns_export_specifier.name.sym.to_string();
        let location = Location {
          filename: resolved_specifier.to_string(),
          line: 0,
          col: 0,
        };
        let ns_def = NamespaceDef {
          elements: doc_nodes.clone(),
        };
        let ns_doc_node = DocNode {
          kind: DocNodeKind::Namespace,
          name: ns_name,
          location,
          js_doc: None,
          namespace_def: Some(ns_def),
          function_def: None,
          variable_def: None,
          enum_def: None,
          class_def: None,
          type_alias_def: None,
          interface_def: None,
        };
        reexported_doc_nodes.push(ns_doc_node);
      }
      // TODO: not handled
      Default(_) => {}
    }
  }

  reexported_doc_nodes
}
