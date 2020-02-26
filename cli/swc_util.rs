#[allow(warnings)]
#[allow(dead_code)]
use crate::file_fetcher::SourceFile;
use crate::global_state::GlobalState;
use crate::msg::MediaType;
use core::ops::Deref;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
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
use swc_ecma_parser::Session;
use swc_ecma_parser::SourceFileInput;
use swc_ecma_parser::Syntax;
use swc_ecma_parser::TsConfig;
use url::Url;

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

#[derive(Debug, Serialize)]
struct SourceGraph(HashMap<String, SourceGraphFile>);

#[derive(Debug, Serialize)]
struct SourceGraphFile {
  local: PathBuf,
  compiled: Option<PathBuf>,
  media_type: MediaType,
  deps: Vec<String>,
}

impl Deref for SourceGraph {
  type Target = HashMap<String, SourceGraphFile>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[allow(dead_code)]
impl SourceGraph {
  pub async fn fetch(
    global_state: GlobalState,
    root: Url,
  ) -> Result<SourceGraph, ErrBox> {
    let mut sg = SourceGraph(HashMap::new());
    let mut to_visit: Vec<String> = vec![root.as_str().to_string()];
    loop {
      if let Some(cur) = to_visit.pop() {
        sg.visit(global_state.clone(), cur.clone()).await?;
        if let Some(source_graph_file) = sg.0.get(&cur) {
          for d in &source_graph_file.deps {
            to_visit.push(d.clone());
          }
        } else {
          unreachable!()
        }
      } else {
        break;
      }
    }
    Ok(sg)
  }

  async fn visit(
    &mut self,
    global_state: GlobalState,
    module_specifier_s: String,
  ) -> Result<(), ErrBox> {
    if self.0.contains_key(&module_specifier_s) {
      return Ok(());
    }

    let module_specifier =
      ModuleSpecifier::resolve_url(&module_specifier_s).unwrap();

    let source_file = global_state
      .file_fetcher
      .fetch_source_file(&module_specifier, None)
      .await?;
    let dep_specs = dependent_specifiers(source_file.clone()).unwrap();

    let mut deps = Vec::<String>::new();
    for dep in dep_specs {
      let m = ModuleSpecifier::resolve_import(&dep, &module_specifier_s)?;
      deps.push(m.as_str().to_string());
    }

    let compiled = if source_file.media_type == MediaType::TypeScript {
      // Assuming the following call does not launch ts_compiler.
      if let Ok(compiled_source_file) = global_state
        .ts_compiler
        .get_compiled_source_file(&module_specifier.into())
      {
        Some(compiled_source_file.filename)
      } else {
        None
      }
    } else {
      None
    };

    self.0.insert(
      module_specifier_s,
      SourceGraphFile {
        local: source_file.filename,
        compiled,
        media_type: source_file.media_type,
        deps,
      },
    );
    Ok(())
  }
}

/// Parses the provided source code, and extracts all the imports. Works with JS
/// and TS. Does not preform I/O. Does not cache results.
#[allow(dead_code)]
fn dependent_specifiers(
  source_file: SourceFile,
) -> Result<Vec<String>, SwcDiagnostics> {
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

  let syntax = match source_file.media_type {
    MediaType::TypeScript => {
      let mut ts_config = TsConfig::default();
      ts_config.dynamic_import = true;
      ts_config.decorators = true;
      Syntax::Typescript(ts_config)
    }
    _ => Syntax::Es(Default::default()),
  };
  let target = JscTarget::Es2019;
  let source_file_input = SourceFileInput::from(&*swc_source_file);
  let lexer = Lexer::new(session, syntax, target, source_file_input, None);
  let mut parser = swc_ecma_parser::Parser::new_from(session, lexer);
  let module =
    parser
      .parse_module()
      .map_err(move |mut err: DiagnosticBuilder| {
        err.cancel();
        SwcDiagnostics::from(buffered_error)
      })?;
  let mut out = Vec::<String>::new();
  for child in module.body {
    use swc_ecma_ast::ModuleDecl;
    use swc_ecma_ast::ModuleItem;
    match child {
      ModuleItem::ModuleDecl(ModuleDecl::Import(decl)) => {
        let atom = decl.src.value;
        out.push(atom.to_string());
      }
      ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(decl)) => {
        if let Some(src) = decl.src {
          // example: export { isMod4 } from "./mod4.js";
          out.push(src.value.to_string());
        }
      }
      _ => {} // ignored
    }
  }
  // Make sure Swc isn't loading more files.
  assert_eq!(source_map.files().len(), 1);
  Ok(out)
}

#[cfg(test)]
pub mod tests {
  use super::*;

  fn rel_module_specifier(relpath: &str) -> ModuleSpecifier {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join(relpath)
      .into_os_string();
    let ps = p.to_str().unwrap();
    // TODO(ry) Why doesn't ModuleSpecifier::resolve_path actually take a
    // Path?!
    ModuleSpecifier::resolve_url_or_path(ps).unwrap()
  }

  #[test]
  fn dependent_specifiers_mock_source_file() {
    let mock_source_file = SourceFile {
      url: Url::parse("https://deno.land/std/examples/cat.ts").unwrap(),
      filename: PathBuf::from("/foo/bar"),
      types_url: None,
      media_type: MediaType::TypeScript,
      source_code: "import { foo } from './bar.js';\n\n".as_bytes().to_vec(),
    };

    let actual = dependent_specifiers(mock_source_file).unwrap();
    let expected = vec!["./bar.js"];
    assert_eq!(actual, expected);
  }

  // TODO(ry) Add simple (sync fs) way to load local SourceFile without a file
  // fetcher.

  #[tokio::test]
  async fn dependent_specifiers_mod6() {
    let (_temp_dir, fetcher) = crate::file_fetcher::tests::test_setup();
    let module_specifier = rel_module_specifier("tests/subdir/mod6.js");
    let result = fetcher.fetch_source_file(&module_specifier, None).await;
    let source_file = result.unwrap();
    let actual = dependent_specifiers(source_file).unwrap();
    let expected = vec!["./mod4.js"];
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn dependent_specifiers_019_media_types() {
    let (_temp_dir, fetcher) = crate::file_fetcher::tests::test_setup();
    let module_specifier = rel_module_specifier("tests/019_media_types.ts");
    let result = fetcher.fetch_source_file(&module_specifier, None).await;
    let source_file = result.unwrap();
    let actual = dependent_specifiers(source_file).unwrap();
    let expected = vec![
      "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts",
      "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts",
      "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts",
      "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts",
      "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js",
      "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js",
      "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js",
      "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js",
    ];
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn dependent_specifiers_error_syntax() {
    let (_temp_dir, fetcher) = crate::file_fetcher::tests::test_setup();
    let module_specifier = rel_module_specifier("tests/error_syntax.js");
    let result = fetcher.fetch_source_file(&module_specifier, None).await;
    let source_file = result.unwrap();
    let result = dependent_specifiers(source_file);
    assert!(result.is_err());
    let diagnostics = result.unwrap_err();
    assert_eq!(diagnostics.len(), 0); // Correct?
  }

  #[tokio::test]
  async fn source_graph_fetch() {
    let http_server_guard = crate::test_util::http_server();
    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = rel_module_specifier("tests/019_media_types.ts");
    let sg = SourceGraph::fetch(global_state, module_specifier.into())
      .await
      .unwrap();
    assert_eq!(sg.len(), 9);
    let r =
      sg.get("http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts");
    assert!(r.is_some());

    println!("{}", serde_json::to_string_pretty(&sg).unwrap());

    drop(http_server_guard);
  }
}
