// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use crate::swc_common;
use crate::swc_common::comments::Comments;
use crate::swc_common::errors::Diagnostic;
use crate::swc_common::errors::DiagnosticBuilder;
use crate::swc_common::errors::Emitter;
use crate::swc_common::errors::Handler;
use crate::swc_common::errors::HandlerFlags;
use crate::swc_common::FileName;
use crate::swc_common::Globals;
use crate::swc_common::SourceMap;
use crate::swc_ecma_ast;
use crate::swc_ecma_ast::ModuleDecl;
use crate::swc_ecma_ast::ModuleItem;
use crate::swc_ecma_parser::lexer::Lexer;
use crate::swc_ecma_parser::JscTarget;
use crate::swc_ecma_parser::Parser;
use crate::swc_ecma_parser::Session;
use crate::swc_ecma_parser::SourceFileInput;
use crate::swc_ecma_parser::Syntax;
use crate::swc_ecma_parser::TsConfig;
use std::sync::Arc;
use std::sync::RwLock;

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

pub struct ImportParser {
  pub buffered_error: BufferedError,
  pub source_map: Arc<SourceMap>,
  pub handler: Handler,
  pub comments: Comments,
  pub globals: Globals,
}

impl ImportParser {
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

    ImportParser {
      buffered_error,
      source_map: Arc::new(SourceMap::default()),
      handler,
      comments: Comments::default(),
      globals: Globals::new(),
    }
  }

  fn parse_source_file(
    &self,
    file_name: String,
    source_code: String,
  ) -> Result<Vec<String>, SwcDiagnostics> {
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

      let import_specifiers = self.get_imports_for_module_body(module.body);
      Ok(import_specifiers)
    })
  }

  pub fn get_import_for_module_decl(
    &self,
    module_decl: &ModuleDecl,
  ) -> Option<String> {
    match module_decl {
      ModuleDecl::Import(import_decl) => {
        Some(import_decl.src.value.to_string())
      }
      ModuleDecl::ExportNamed(named_export) => {
        named_export.src.as_ref().map(|s| s.value.to_string())
      }
      ModuleDecl::ExportAll(export_all) => {
        Some(export_all.src.value.to_string())
      }
      _ => None,
    }
  }

  pub fn get_imports_for_module_body(
    &self,
    module_body: Vec<swc_ecma_ast::ModuleItem>,
  ) -> Vec<String> {
    let mut import_specifiers: Vec<String> = vec![];

    for node in module_body.iter() {
      if let ModuleItem::ModuleDecl(module_decl) = node {
        if let Some(specifier) = self.get_import_for_module_decl(module_decl) {
          import_specifiers.push(specifier);
        }
      }
    }

    import_specifiers
  }
}

pub fn get_module_imports(
  file_name: String,
  source_code: String,
) -> Vec<String> {
  let import_parser = ImportParser::default();
  import_parser
    .parse_source_file(file_name, source_code)
    .expect("Failed to parse source file")
}
