// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::msg::MediaType;
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
use crate::swc_common::Span;
use crate::swc_ecma_ast;
use crate::swc_ecma_ast::Program;
use crate::swc_ecma_parser::lexer::Lexer;
use crate::swc_ecma_parser::EsConfig;
use crate::swc_ecma_parser::JscTarget;
use crate::swc_ecma_parser::Parser;
use crate::swc_ecma_parser::SourceFileInput;
use crate::swc_ecma_parser::Syntax;
use crate::swc_ecma_parser::TsConfig;
use crate::swc_ecma_visit::FoldWith;
use deno_core::ErrBox;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::RwLock;
use swc_common::chain;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Node;
use swc_ecma_transforms::fixer;
use swc_ecma_transforms::typescript;

struct DummyHandler;

impl swc_ecma_codegen::Handlers for DummyHandler {}

fn get_default_es_config() -> EsConfig {
  let mut config = EsConfig::default();
  config.num_sep = true;
  config.class_private_props = true;
  config.class_private_methods = true;
  config.class_props = true;
  config.export_default_from = true;
  config.export_namespace_from = true;
  config.dynamic_import = true;
  config.nullish_coalescing = true;
  config.optional_chaining = true;
  config.import_meta = true;
  config.top_level_await = true;
  config
}

fn get_default_ts_config() -> TsConfig {
  let mut ts_config = TsConfig::default();
  ts_config.dynamic_import = true;
  ts_config.decorators = true;
  ts_config
}

pub fn get_syntax_for_media_type(media_type: MediaType) -> Syntax {
  match media_type {
    MediaType::JavaScript => Syntax::Es(get_default_es_config()),
    MediaType::JSX => {
      let mut config = get_default_es_config();
      config.jsx = true;
      Syntax::Es(config)
    }
    MediaType::TypeScript => Syntax::Typescript(get_default_ts_config()),
    MediaType::TSX => {
      let mut config = get_default_ts_config();
      config.tsx = true;
      Syntax::Typescript(config)
    }
    _ => Syntax::Es(get_default_es_config()),
  }
}

#[derive(Clone, Debug)]
pub struct SwcDiagnosticBuffer {
  pub diagnostics: Vec<String>,
}

impl Error for SwcDiagnosticBuffer {}

impl fmt::Display for SwcDiagnosticBuffer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let msg = self.diagnostics.join(",");

    f.pad(&msg)
  }
}

impl SwcDiagnosticBuffer {
  pub fn from_swc_error(
    error_buffer: SwcErrorBuffer,
    parser: &AstParser,
  ) -> Self {
    let s = error_buffer.0.read().unwrap().clone();

    let diagnostics = s
      .iter()
      .map(|d| {
        let mut msg = d.message();

        if let Some(span) = d.span.primary_span() {
          let location = parser.get_span_location(span);
          let filename = match &location.file.name {
            FileName::Custom(n) => n,
            _ => unreachable!(),
          };
          msg = format!(
            "{} at {}:{}:{}",
            msg, filename, location.line, location.col_display
          );
        }

        msg
      })
      .collect::<Vec<String>>();

    Self { diagnostics }
  }
}

#[derive(Clone)]
pub struct SwcErrorBuffer(Arc<RwLock<Vec<Diagnostic>>>);

impl SwcErrorBuffer {
  pub fn default() -> Self {
    Self(Arc::new(RwLock::new(vec![])))
  }
}

impl Emitter for SwcErrorBuffer {
  fn emit(&mut self, db: &DiagnosticBuilder) {
    self.0.write().unwrap().push((**db).clone());
  }
}

/// Low-level utility structure with common AST parsing functions.
///
/// Allows to build more complicated parser by providing a callback
/// to `parse_module`.
pub struct AstParser {
  pub buffered_error: SwcErrorBuffer,
  pub source_map: Arc<SourceMap>,
  pub handler: Handler,
  pub comments: Comments,
  pub globals: Globals,
}

impl AstParser {
  pub fn default() -> Self {
    let buffered_error = SwcErrorBuffer::default();

    let handler = Handler::with_emitter_and_flags(
      Box::new(buffered_error.clone()),
      HandlerFlags {
        dont_buffer_diagnostics: true,
        can_emit_warnings: true,
        ..Default::default()
      },
    );

    AstParser {
      buffered_error,
      source_map: Arc::new(SourceMap::default()),
      handler,
      comments: Comments::default(),
      globals: Globals::new(),
    }
  }

  pub fn parse_module<F, R>(
    &self,
    file_name: &str,
    media_type: MediaType,
    source_code: &str,
    callback: F,
  ) -> R
  where
    F: FnOnce(Result<swc_ecma_ast::Module, SwcDiagnosticBuffer>) -> R,
  {
    swc_common::GLOBALS.set(&self.globals, || {
      let swc_source_file = self.source_map.new_source_file(
        FileName::Custom(file_name.to_string()),
        source_code.to_string(),
      );

      let buffered_err = self.buffered_error.clone();
      let syntax = get_syntax_for_media_type(media_type);

      let lexer = Lexer::new(
        syntax,
        JscTarget::Es2019,
        SourceFileInput::from(&*swc_source_file),
        Some(&self.comments),
      );

      let mut parser = Parser::new_from(lexer);

      let parse_result = parser.parse_module().map_err(move |err| {
        let mut diagnostic = err.into_diagnostic(&self.handler);
        diagnostic.emit();
        SwcDiagnosticBuffer::from_swc_error(buffered_err, self)
      });

      callback(parse_result)
    })
  }

  pub fn strip_types(
    &self,
    file_name: &str,
    media_type: MediaType,
    source_code: &str,
  ) -> Result<String, ErrBox> {
    self.parse_module(file_name, media_type, source_code, |parse_result| {
      let module = parse_result?;
      let program = Program::Module(module);
      let mut compiler_pass = chain!(typescript::strip(), fixer());
      let program = swc_ecma_transforms::util::COMMENTS
        .set(&self.comments, || program.fold_with(&mut compiler_pass));

      let mut src_map_buf = vec![];
      let mut buf = vec![];
      {
        let handlers = Box::new(DummyHandler);
        let writer = Box::new(JsWriter::new(
          self.source_map.clone(),
          "\n",
          &mut buf,
          Some(&mut src_map_buf),
        ));
        let config = swc_ecma_codegen::Config { minify: false };
        let mut emitter = swc_ecma_codegen::Emitter {
          cfg: config,
          comments: Some(&self.comments),
          cm: self.source_map.clone(),
          wr: writer,
          handlers,
        };
        program.emit_with(&mut emitter)?;
      }
      let mut src = String::from_utf8(buf).map_err(ErrBox::from)?;
      {
        let mut buf = vec![];
        self
          .source_map
          .build_source_map_from(&mut src_map_buf, None)
          .to_writer(&mut buf)?;
        let map = String::from_utf8(buf)?;

        src.push_str("//# sourceMappingURL=data:application/json;base64,");
        let encoded_map = base64::encode(map.as_bytes());
        src.push_str(&encoded_map);
      }
      Ok(src)
    })
  }

  pub fn get_span_location(&self, span: Span) -> swc_common::Loc {
    self.source_map.lookup_char_pos(span.lo())
  }

  pub fn get_span_comments(
    &self,
    span: Span,
  ) -> Vec<swc_common::comments::Comment> {
    let maybe_comments = self.comments.take_leading_comments(span.lo());

    if let Some(comments) = maybe_comments {
      // clone the comments and put them back in map
      let to_return = comments.clone();
      self.comments.add_leading(span.lo(), comments);
      to_return
    } else {
      vec![]
    }
  }
}

#[test]
fn test_strip_types() {
  let ast_parser = AstParser::default();
  let result = ast_parser
    .strip_types("test.ts", MediaType::TypeScript, "const a: number = 10;")
    .unwrap();
  assert!(result.starts_with(
    "const a = 10;\n//# sourceMappingURL=data:application/json;base64,"
  ));
}
