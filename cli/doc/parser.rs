// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use regex::Regex;
use std::sync::Arc;
use std::sync::RwLock;
use swc_common;
use swc_common::comments::CommentKind;
use swc_common::comments::Comments;
use swc_common::errors::Diagnostic;
use swc_common::errors::DiagnosticBuilder;
use swc_common::errors::Emitter;
use swc_common::errors::Handler;
use swc_common::errors::HandlerFlags;
use swc_common::FileName;
use swc_common::Globals;
use swc_common::SourceMap;
use swc_common::Span;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::JscTarget;
use swc_ecma_parser::Parser;
use swc_ecma_parser::Session;
use swc_ecma_parser::SourceFileInput;
use swc_ecma_parser::Syntax;
use swc_ecma_parser::TsConfig;

use super::DocNode;

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
    module_decl: &swc_ecma_ast::ModuleDecl,
  ) -> Vec<DocNode> {
    use swc_ecma_ast::ModuleDecl;

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

  pub fn get_doc_nodes_for_module_body(
    &self,
    module_body: Vec<swc_ecma_ast::ModuleItem>,
  ) -> Vec<DocNode> {
    let mut doc_entries: Vec<DocNode> = vec![];
    for node in module_body.iter() {
      if let swc_ecma_ast::ModuleItem::ModuleDecl(module_decl) = node {
        doc_entries.extend(self.get_doc_nodes_for_module_decl(module_decl));
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
