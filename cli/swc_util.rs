use crate::file_fetcher::SourceFile;
use crate::msg::MediaType;
use std::sync::Arc;
use std::sync::RwLock;
use swc_common::errors::Diagnostic;
use swc_common::errors::DiagnosticBuilder;
use swc_common::errors::Emitter;
use swc_common::errors::Handler;
use swc_common::errors::HandlerFlags;
use swc_common::FileName;
use swc_common::Globals;
use swc_common::SourceMap;
use swc_common::GLOBALS;
use swc_ecma_ast::Module;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::JscTarget;
use swc_ecma_parser::Parser;
use swc_ecma_parser::Session;
use swc_ecma_parser::SourceFileInput;
use swc_ecma_parser::Syntax;
use swc_ecma_parser::TsConfig;

pub type SwcDiagnostics = Vec<Diagnostic>;

#[derive(Clone, Default)]
pub(crate) struct BufferedError(Arc<RwLock<SwcDiagnostics>>);

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

pub struct Compiler {
  buffered_error: BufferedError,
  globals: Globals,
  pub source_map: Arc<SourceMap>,
  pub handler: Handler,
}

impl Compiler {
  pub fn new() -> Self {
    let buffered_error = BufferedError::default();

    let handler = Handler::with_emitter_and_flags(
      Box::new(buffered_error.clone()),
      HandlerFlags {
        dont_buffer_diagnostics: true,
        can_emit_warnings: true,
        ..Default::default()
      },
    );

    Compiler {
      buffered_error,
      source_map: Arc::new(SourceMap::default()),
      handler,
      globals: Globals::new(),
    }
  }

  fn run<R, F>(&self, op: F) -> R
  where
    F: FnOnce() -> R,
  {
    GLOBALS.set(&self.globals, || op())
  }

  pub fn parse_file(
    &mut self,
    source_file: SourceFile,
  ) -> Result<Module, SwcDiagnostics> {
    self.run(|| {
      let swc_source_file = self.source_map.new_source_file(
        FileName::Custom(source_file.url.as_str().into()),
        std::str::from_utf8(&source_file.source_code)
          .unwrap()
          .into(),
      );

      let buffered_err = self.buffered_error.clone();
      let session = Session {
        handler: &self.handler,
      };

      let syntax = match source_file.media_type {
        MediaType::TypeScript => {
          let mut ts_config = TsConfig::default();
          ts_config.dynamic_import = true;
          Syntax::Typescript(ts_config)
        }
        _ => Syntax::Es(Default::default()),
      };

      let lexer = Lexer::new(
        session,
        syntax,
        JscTarget::Es2019,
        SourceFileInput::from(&*swc_source_file),
        None,
      );

      let mut parser = Parser::new_from(session, lexer);

      parser
        .parse_module()
        .map_err(move |mut err: DiagnosticBuilder| {
          err.cancel();
          SwcDiagnostics::from(buffered_err)
        })
    })
  }
}
