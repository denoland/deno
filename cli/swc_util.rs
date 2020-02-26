use crate::file_fetcher::SourceFile;
use std::sync::Arc;
use swc_common::errors::ColorConfig;
use swc_common::errors::Handler;
use swc_common::FileName;
use swc_common::SourceMap;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::Session;
use swc_ecma_parser::Syntax;
use swc_ecma_parser::TsConfig;

// Parses the provided source code, and extracts all the imports. Works with JS
// and TS. Does not preform I/O.
#[allow(dead_code)]
fn get_imports(source_file: SourceFile) -> Result<Vec<String>, ()> {
  let cm: Arc<SourceMap> = Default::default();
  let fm = cm.new_source_file(
    FileName::Custom(source_file.url.as_str().into()),
    std::str::from_utf8(&source_file.source_code)
      .unwrap()
      .into(),
  );

  let handler =
    Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
  let session = Session { handler: &handler };

  let mut ts_config: TsConfig = Default::default();
  ts_config.dynamic_import = true;
  ts_config.decorators = true;

  let lexer = Lexer::new(
    session,
    // Syntax::Es(Default::default()),
    Syntax::Typescript(ts_config),
    Default::default(),
    swc_ecma_parser::SourceFileInput::from(&*fm),
    None,
  );

  let mut parser = swc_ecma_parser::Parser::new_from(session, lexer);

  let module = parser
    .parse_module()
    .map_err(|mut e| {
      e.emit();
      ()
    })
    .expect("failed to parser module");

  let mut out = Vec::<String>::new();
  for child in module.body {
    use swc_ecma_ast::ModuleDecl;
    use swc_ecma_ast::ModuleItem;
    if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = child {
      let atom = import_decl.src.value;
      out.push(atom.to_string());
    }
  }
  Ok(out)
}

#[test]
fn test_get_imports() {
  use crate::msg::MediaType;
  use std::path::PathBuf;
  use url::Url;
  let mock_source_file = SourceFile {
    url: Url::parse("https://deno.land/std/examples/cat.ts").unwrap(),
    filename: PathBuf::from("/foo/bar"),
    types_url: None,
    media_type: MediaType::TypeScript,
    source_code: "import { foo } from './bar.js';\n\n".as_bytes().to_vec(),
  };

  let imports = get_imports(mock_source_file).unwrap();
  assert_eq!(imports, vec!["./bar.js".to_string()]);
}
