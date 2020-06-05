// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::file_fetcher::map_file_extension;
use crate::op_error::OpError;
use crate::swc_common::comments::CommentKind;
use crate::swc_common::Span;
use crate::swc_ecma_ast;
use crate::swc_ecma_ast::Decl;
use crate::swc_ecma_ast::DefaultDecl;
use crate::swc_ecma_ast::ModuleDecl;
use crate::swc_ecma_ast::Stmt;
use crate::swc_util::AstParser;
use crate::swc_util::SwcDiagnosticBuffer;

use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use futures::Future;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

use super::namespace::NamespaceDef;
use super::node;
use super::node::ModuleDoc;
use super::DocNode;
use super::DocNodeKind;
use super::Location;

pub trait DocFileLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<ModuleSpecifier, OpError> {
    ModuleSpecifier::resolve_import(specifier, referrer).map_err(OpError::from)
  }

  fn load_source_code(
    &self,
    specifier: &str,
  ) -> Pin<Box<dyn Future<Output = Result<String, OpError>>>>;
}

pub struct DocParser {
  pub ast_parser: AstParser,
  pub loader: Box<dyn DocFileLoader>,
}

impl DocParser {
  pub fn new(loader: Box<dyn DocFileLoader>) -> Self {
    DocParser {
      loader,
      ast_parser: AstParser::new(),
    }
  }

  fn parse_module(
    &self,
    file_name: &str,
    source_code: &str,
  ) -> Result<ModuleDoc, SwcDiagnosticBuffer> {
    let media_type = map_file_extension(&PathBuf::from(file_name));
    self.ast_parser.parse_module(
      file_name,
      media_type,
      source_code,
      |parse_result| {
        let module = parse_result?;
        let doc_entries =
          self.get_doc_nodes_for_module_body(module.body.clone());
        let reexports = self.get_reexports_for_module_body(module.body);
        let module_doc = ModuleDoc {
          exports: doc_entries,
          reexports,
        };
        Ok(module_doc)
      },
    )
  }

  pub async fn parse(&self, file_name: &str) -> Result<Vec<DocNode>, ErrBox> {
    let source_code = self.loader.load_source_code(file_name).await?;

    self.parse_source(file_name, source_code.as_str())
  }

  pub fn parse_source(
    &self,
    file_name: &str,
    source_code: &str,
  ) -> Result<Vec<DocNode>, ErrBox> {
    let module_doc = self.parse_module(file_name, &source_code)?;
    Ok(module_doc.exports)
  }

  async fn flatten_reexports(
    &self,
    reexports: &[node::Reexport],
    referrer: &str,
  ) -> Result<Vec<DocNode>, ErrBox> {
    let mut by_src: HashMap<String, Vec<node::Reexport>> = HashMap::new();

    let mut processed_reexports: Vec<DocNode> = vec![];

    for reexport in reexports {
      if by_src.get(&reexport.src).is_none() {
        by_src.insert(reexport.src.to_string(), vec![]);
      }

      let bucket = by_src.get_mut(&reexport.src).unwrap();
      bucket.push(reexport.clone());
    }

    for specifier in by_src.keys() {
      let resolved_specifier = self.loader.resolve(specifier, referrer)?;
      let doc_nodes = self.parse(&resolved_specifier.to_string()).await?;
      let reexports_for_specifier = by_src.get(specifier).unwrap();

      for reexport in reexports_for_specifier {
        match &reexport.kind {
          node::ReexportKind::All => {
            processed_reexports.extend(doc_nodes.clone())
          }
          node::ReexportKind::Namespace(ns_name) => {
            let ns_def = NamespaceDef {
              elements: doc_nodes.clone(),
            };
            let ns_doc_node = DocNode {
              kind: DocNodeKind::Namespace,
              name: ns_name.to_string(),
              location: Location {
                filename: specifier.to_string(),
                line: 1,
                col: 0,
              },
              js_doc: None,
              namespace_def: Some(ns_def),
              enum_def: None,
              type_alias_def: None,
              interface_def: None,
              variable_def: None,
              function_def: None,
              class_def: None,
            };
            processed_reexports.push(ns_doc_node);
          }
          node::ReexportKind::Named(ident, maybe_alias) => {
            // Try to find reexport.
            // NOTE: the reexport might actually be reexport from another
            // module; for now we're skipping nested reexports.
            let maybe_doc_node =
              doc_nodes.iter().find(|node| &node.name == ident);

            if let Some(doc_node) = maybe_doc_node {
              let doc_node = doc_node.clone();
              let doc_node = if let Some(alias) = maybe_alias {
                DocNode {
                  name: alias.to_string(),
                  ..doc_node
                }
              } else {
                doc_node
              };

              processed_reexports.push(doc_node);
            }
          }
          node::ReexportKind::Default => {
            // TODO: handle default export from child module
          }
        }
      }
    }

    Ok(processed_reexports)
  }

  pub async fn parse_with_reexports(
    &self,
    file_name: &str,
  ) -> Result<Vec<DocNode>, ErrBox> {
    let source_code = self.loader.load_source_code(file_name).await?;

    let module_doc = self.parse_module(file_name, &source_code)?;

    let flattened_docs = if !module_doc.reexports.is_empty() {
      let mut flattenned_reexports = self
        .flatten_reexports(&module_doc.reexports, file_name)
        .await?;
      flattenned_reexports.extend(module_doc.exports);
      flattenned_reexports
    } else {
      module_doc.exports
    };

    Ok(flattened_docs)
  }

  pub fn get_doc_nodes_for_module_exports(
    &self,
    module_decl: &ModuleDecl,
  ) -> Vec<DocNode> {
    match module_decl {
      ModuleDecl::ExportDecl(export_decl) => {
        vec![super::module::get_doc_node_for_export_decl(
          self,
          export_decl,
        )]
      }
      ModuleDecl::ExportDefaultDecl(export_default_decl) => {
        let (js_doc, location) =
          self.details_for_span(export_default_decl.span);
        let name = "default".to_string();

        let doc_node = match &export_default_decl.decl {
          DefaultDecl::Class(class_expr) => {
            let class_def =
              crate::doc::class::class_to_class_def(self, &class_expr.class);
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
          DefaultDecl::Fn(fn_expr) => {
            let function_def =
              crate::doc::function::function_to_function_def(&fn_expr.function);
            DocNode {
              kind: DocNodeKind::Function,
              name,
              location,
              js_doc,
              class_def: None,
              function_def: Some(function_def),
              variable_def: None,
              enum_def: None,
              type_alias_def: None,
              namespace_def: None,
              interface_def: None,
            }
          }
          DefaultDecl::TsInterfaceDecl(interface_decl) => {
            let (_, interface_def) =
              crate::doc::interface::get_doc_for_ts_interface_decl(
                self,
                interface_decl,
              );
            DocNode {
              kind: DocNodeKind::Interface,
              name,
              location,
              js_doc,
              class_def: None,
              function_def: None,
              variable_def: None,
              enum_def: None,
              type_alias_def: None,
              namespace_def: None,
              interface_def: Some(interface_def),
            }
          }
        };

        vec![doc_node]
      }
      ModuleDecl::ExportDefaultExpr(_export_default_expr) => vec![],
      _ => vec![],
    }
  }

  pub fn get_doc_node_for_stmt(&self, stmt: &Stmt) -> Option<DocNode> {
    match stmt {
      Stmt::Decl(decl) => self.get_doc_node_for_decl(decl),
      _ => None,
    }
  }

  fn details_for_span(&self, span: Span) -> (Option<String>, Location) {
    let js_doc = self.js_doc_for_span(span);
    let location = self.ast_parser.get_span_location(span).into();
    (js_doc, location)
  }

  pub fn get_doc_node_for_decl(&self, decl: &Decl) -> Option<DocNode> {
    match decl {
      Decl::Class(class_decl) => {
        if !class_decl.declare {
          return None;
        }
        let (name, class_def) =
          super::class::get_doc_for_class_decl(self, class_decl);
        let (js_doc, location) = self.details_for_span(class_decl.class.span);
        Some(DocNode {
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
        })
      }
      Decl::Fn(fn_decl) => {
        if !fn_decl.declare {
          return None;
        }
        let (name, function_def) =
          super::function::get_doc_for_fn_decl(fn_decl);
        let (js_doc, location) = self.details_for_span(fn_decl.function.span);
        Some(DocNode {
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
        })
      }
      Decl::Var(var_decl) => {
        if !var_decl.declare {
          return None;
        }
        let (name, var_def) = super::variable::get_doc_for_var_decl(var_decl);
        let (js_doc, location) = self.details_for_span(var_decl.span);
        Some(DocNode {
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
        })
      }
      Decl::TsInterface(ts_interface_decl) => {
        if !ts_interface_decl.declare {
          return None;
        }
        let (name, interface_def) =
          super::interface::get_doc_for_ts_interface_decl(
            self,
            ts_interface_decl,
          );
        let (js_doc, location) = self.details_for_span(ts_interface_decl.span);
        Some(DocNode {
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
        })
      }
      Decl::TsTypeAlias(ts_type_alias) => {
        if !ts_type_alias.declare {
          return None;
        }
        let (name, type_alias_def) =
          super::type_alias::get_doc_for_ts_type_alias_decl(
            self,
            ts_type_alias,
          );
        let (js_doc, location) = self.details_for_span(ts_type_alias.span);
        Some(DocNode {
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
        })
      }
      Decl::TsEnum(ts_enum) => {
        if !ts_enum.declare {
          return None;
        }
        let (name, enum_def) =
          super::r#enum::get_doc_for_ts_enum_decl(self, ts_enum);
        let (js_doc, location) = self.details_for_span(ts_enum.span);
        Some(DocNode {
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
        })
      }
      Decl::TsModule(ts_module) => {
        if !ts_module.declare {
          return None;
        }
        let (name, namespace_def) =
          super::namespace::get_doc_for_ts_module(self, ts_module);
        let (js_doc, location) = self.details_for_span(ts_module.span);
        Some(DocNode {
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
        })
      }
    }
  }

  pub fn get_reexports_for_module_body(
    &self,
    module_body: Vec<swc_ecma_ast::ModuleItem>,
  ) -> Vec<node::Reexport> {
    use swc_ecma_ast::ExportSpecifier::*;

    let mut reexports: Vec<node::Reexport> = vec![];

    for node in module_body.iter() {
      if let swc_ecma_ast::ModuleItem::ModuleDecl(module_decl) = node {
        let r = match module_decl {
          ModuleDecl::ExportNamed(named_export) => {
            if let Some(src) = &named_export.src {
              let src_str = src.value.to_string();
              named_export
                .specifiers
                .iter()
                .map(|export_specifier| match export_specifier {
                  Namespace(ns_export) => node::Reexport {
                    kind: node::ReexportKind::Namespace(
                      ns_export.name.sym.to_string(),
                    ),
                    src: src_str.to_string(),
                  },
                  Default(_) => node::Reexport {
                    kind: node::ReexportKind::Default,
                    src: src_str.to_string(),
                  },
                  Named(named_export) => {
                    let ident = named_export.orig.sym.to_string();
                    let maybe_alias =
                      named_export.exported.as_ref().map(|e| e.sym.to_string());
                    let kind = node::ReexportKind::Named(ident, maybe_alias);
                    node::Reexport {
                      kind,
                      src: src_str.to_string(),
                    }
                  }
                })
                .collect::<Vec<node::Reexport>>()
            } else {
              vec![]
            }
          }
          ModuleDecl::ExportAll(export_all) => {
            let reexport = node::Reexport {
              kind: node::ReexportKind::All,
              src: export_all.src.value.to_string(),
            };
            vec![reexport]
          }
          _ => vec![],
        };

        reexports.extend(r);
      }
    }

    reexports
  }

  pub fn get_doc_nodes_for_module_body(
    &self,
    module_body: Vec<swc_ecma_ast::ModuleItem>,
  ) -> Vec<DocNode> {
    let mut doc_entries: Vec<DocNode> = vec![];
    for node in module_body.iter() {
      match node {
        swc_ecma_ast::ModuleItem::ModuleDecl(module_decl) => {
          doc_entries
            .extend(self.get_doc_nodes_for_module_exports(module_decl));
        }
        swc_ecma_ast::ModuleItem::Stmt(stmt) => {
          if let Some(doc_node) = self.get_doc_node_for_stmt(stmt) {
            doc_entries.push(doc_node);
          }
        }
      }
    }
    doc_entries
  }

  pub fn js_doc_for_span(&self, span: Span) -> Option<String> {
    let comments = self.ast_parser.get_span_comments(span);
    let js_doc_comment = comments.iter().rev().find(|comment| {
      comment.kind == CommentKind::Block && comment.text.starts_with('*')
    })?;

    let mut margin_pat = String::from("");
    if let Some(margin) = self.ast_parser.source_map.span_to_margin(span) {
      for _ in 0..margin {
        margin_pat.push(' ');
      }
    }

    let js_doc_re = Regex::new(r#" ?\* ?"#).unwrap();
    let txt = js_doc_comment
      .text
      .split('\n')
      .map(|line| js_doc_re.replace(line, "").to_string())
      .map(|line| {
        if line.starts_with(&margin_pat) {
          line[margin_pat.len()..].to_string()
        } else {
          line
        }
      })
      .collect::<Vec<String>>()
      .join("\n");

    let txt = txt.trim_start().trim_end().to_string();

    Some(txt)
  }
}
