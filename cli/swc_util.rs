use crate::file_fetcher::SourceFile;
use std::sync::Arc;
use std::sync::RwLock;
use swc_common::errors::Diagnostic;
use swc_common::errors::DiagnosticBuilder;
use swc_common::errors::Emitter;
use swc_common::errors::Handler;
use swc_common::errors::HandlerFlags;
use swc_common::FileName;
use swc_common::SourceMap;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::JscTarget;
use swc_ecma_parser::Parser;
use swc_ecma_parser::Session;
use swc_ecma_parser::SourceFileInput;
use swc_ecma_parser::Syntax;
use swc_ecma_parser::TsConfig;

type SwcDiagnostics = Vec<Diagnostic>;

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

pub fn parse_file(source_file: SourceFile) {
  let source_map = SourceMap::default();
  let swc_source_file = source_map.new_source_file(
    FileName::Custom(source_file.url.as_str().into()),
    std::str::from_utf8(&source_file.source_code)
      .unwrap()
      .into(),
  );

  let buffered_error = BufferedError::default();

  let handler = Handler::with_emitter_and_flags(
    Box::new(buffered_error.clone()),
    HandlerFlags {
      dont_buffer_diagnostics: true,
      can_emit_warnings: true,
      ..Default::default()
    },
  );

  let session = Session { handler: &handler };

  let mut ts_config = TsConfig::default();
  ts_config.dynamic_import = true;
  ts_config.decorators = true;
  let syntax = Syntax::Typescript(ts_config);

  let lexer = Lexer::new(
    session,
    syntax,
    JscTarget::Es2019,
    SourceFileInput::from(&*swc_source_file),
    None,
  );

  let mut parser = Parser::new_from(session, lexer);

  let result =
    parser
      .parse_module()
      .map_err(move |mut err: DiagnosticBuilder| {
        err.cancel();
        SwcDiagnostics::from(buffered_error)
      });

  match result {
    Ok(module) => {
      let serialized = serde_json::to_string_pretty(&module).unwrap();
      println!("{}", serialized)
    }
    Err(e) => {
      eprintln!("error ast {:#?}", e);
    }
  }
}
