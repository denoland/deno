// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_common;
use crate::swc_common::comments::CommentKind;
use crate::swc_common::comments::Comments;
use crate::swc_common::errors::Diagnostic;
use crate::swc_common::errors::DiagnosticBuilder;
use crate::swc_common::errors::Emitter;
use crate::swc_common::errors::Handler;
use crate::swc_common::errors::HandlerFlags;
use crate::swc_common::FileName;
use crate::swc_common::Globals;
use crate::swc_common::SourceMap;
use crate::swc_common::Span;
use crate::swc_ecma_ast;
use crate::swc_ecma_ast::Decl;
use crate::swc_ecma_ast::ModuleDecl;
use crate::swc_ecma_ast::Stmt;
use crate::swc_ecma_parser::lexer::Lexer;
use crate::swc_ecma_parser::JscTarget;
use crate::swc_ecma_parser::Parser;
use crate::swc_ecma_parser::Session;
use crate::swc_ecma_parser::SourceFileInput;
use crate::swc_ecma_parser::Syntax;
use crate::swc_ecma_parser::TsConfig;
use regex::Regex;
use std::sync::Arc;
use std::sync::RwLock;

use super::DocNode;
use super::DocNodeKind;
use super::Location;

pub type SwcDiagnostics = Vec<Diagnostic>;

#[derive(Clone, Default)]
pub struct BufferedError(Arc<RwLock<SwcDiagnostics>>);

impl Emitter for BufferedError {
  fn emit(&mut self, db: &DiagnosticBuilder) {
    self.0.write().unwrap().push((**db).clone());
  }
}

impl From<BufferedError> for Vec<Diagnostic> {
  fn from(buf: BufferedError) -> Self {
    let s = buf.0.read().unwrap();
    s.clone()
  }
}

pub struct DocParser {
  pub buffered_error: BufferedError,
  pub source_map: Arc<SourceMap>,
  pub handler: Handler,
  pub comments: Comments,
  pub globals: Globals,
}

impl DocParser {
  pub fn default() -> Self {
    let buffered_error = BufferedError::default();

    let handler = Handler::with_emitter_and_flags(
      Box::new(buffered_error.clone()),
      HandlerFlags {
        dont_buffer_diagnostics: true,
        can_emit_warnings: true,
        ..Default::default()
      },
    );

    DocParser {
      buffered_error,
      source_map: Arc::new(SourceMap::default()),
      handler,
      comments: Comments::default(),
      globals: Globals::new(),
    }
  }

  pub fn parse(
    &self,
    file_name: String,
    source_code: String,
  ) -> Result<Vec<DocNode>, SwcDiagnostics> {
    swc_common::GLOBALS.set(&self.globals, || {
      let swc_source_file = self
        .source_map
        .new_source_file(FileName::Custom(file_name), source_code);

      let buffered_err = self.buffered_error.clone();
      let session = Session {
        handler: &self.handler,
      };

      let mut ts_config = TsConfig::default();
      ts_config.dynamic_import = true;
      let syntax = Syntax::Typescript(ts_config);

      let lexer = Lexer::new(
        session,
        syntax,
        JscTarget::Es2019,
        SourceFileInput::from(&*swc_source_file),
        Some(&self.comments),
      );

      let mut parser = Parser::new_from(session, lexer);

      let module =
        parser
          .parse_module()
          .map_err(move |mut err: DiagnosticBuilder| {
            err.cancel();
            SwcDiagnostics::from(buffered_err)
          })?;

      let doc_entries = self.get_doc_nodes_for_module_body(module.body);
      Ok(doc_entries)
    })
  }

  pub fn get_doc_nodes_for_module_decl(
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
      ModuleDecl::ExportNamed(_named_export) => {
        vec![]
        // TODO(bartlomieju):
        // super::module::get_doc_nodes_for_named_export(self, named_export)
      }
      ModuleDecl::ExportDefaultDecl(_) => vec![],
      ModuleDecl::ExportDefaultExpr(_) => vec![],
      ModuleDecl::ExportAll(_) => vec![],
      ModuleDecl::TsExportAssignment(_) => vec![],
      ModuleDecl::TsNamespaceExport(_) => vec![],
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
    let location = self.source_map.lookup_char_pos(span.lo()).into();
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
          super::function::get_doc_for_fn_decl(self, fn_decl);
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
        let (name, var_def) =
          super::variable::get_doc_for_var_decl(self, var_decl);
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

  pub fn get_doc_nodes_for_module_body(
    &self,
    module_body: Vec<swc_ecma_ast::ModuleItem>,
  ) -> Vec<DocNode> {
    let mut doc_entries: Vec<DocNode> = vec![];
    for node in module_body.iter() {
      match node {
        swc_ecma_ast::ModuleItem::ModuleDecl(module_decl) => {
          doc_entries.extend(self.get_doc_nodes_for_module_decl(module_decl));
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
    let comments = self.comments.take_leading_comments(span.lo())?;
    let js_doc_comment = comments.iter().find(|comment| {
      comment.kind == CommentKind::Block && comment.text.starts_with('*')
    })?;

    let mut margin_pat = String::from("");
    if let Some(margin) = self.source_map.span_to_margin(span) {
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
