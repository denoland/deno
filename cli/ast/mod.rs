// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::config_file;
use crate::text_encoding::strip_bom;

use deno_ast::get_syntax;
use deno_ast::swc::ast::Module;
use deno_ast::swc::ast::Program;
use deno_ast::swc::codegen::text_writer::JsWriter;
use deno_ast::swc::codegen::Node;
use deno_ast::swc::common::chain;
use deno_ast::swc::common::comments::SingleThreadedComments;
use deno_ast::swc::common::errors::Diagnostic as SwcDiagnostic;
use deno_ast::swc::common::BytePos;
use deno_ast::swc::common::FileName;
use deno_ast::swc::common::Globals;
use deno_ast::swc::common::Mark;
use deno_ast::swc::common::SourceMap;
use deno_ast::swc::common::Spanned;
use deno_ast::swc::parser::error::Error as SwcError;
use deno_ast::swc::parser::error::SyntaxError;
use deno_ast::swc::parser::lexer::Lexer;
use deno_ast::swc::parser::StringInput;
use deno_ast::swc::transforms::fixer;
use deno_ast::swc::transforms::helpers;
use deno_ast::swc::transforms::hygiene;
use deno_ast::swc::transforms::pass::Optional;
use deno_ast::swc::transforms::proposals;
use deno_ast::swc::transforms::react;
use deno_ast::swc::transforms::resolver_with_mark;
use deno_ast::swc::transforms::typescript;
use deno_ast::swc::visit::FoldWith;
use deno_ast::Diagnostic;
use deno_ast::LineAndColumnDisplay;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

mod bundle_hook;
mod transforms;

pub use bundle_hook::BundleHook;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Location {
  pub specifier: String,
  pub line: usize,
  pub col: usize,
}

impl Location {
  pub fn from_pos(parsed_source: &ParsedSource, pos: BytePos) -> Self {
    Location::from_line_and_column(
      parsed_source.specifier().to_string(),
      parsed_source.source().line_and_column_index(pos),
    )
  }

  pub fn from_line_and_column(
    specifier: String,
    line_and_column: deno_ast::LineAndColumnIndex,
  ) -> Self {
    Location {
      specifier,
      line: line_and_column.line_index + 1,
      col: line_and_column.column_index,
    }
  }
}

impl From<deno_ast::swc::common::Loc> for Location {
  fn from(swc_loc: deno_ast::swc::common::Loc) -> Self {
    use deno_ast::swc::common::FileName::*;

    let filename = match &swc_loc.file.name {
      Real(path_buf) => path_buf.to_string_lossy().to_string(),
      Custom(str_) => str_.to_string(),
      Url(url) => url.to_string(),
      _ => panic!("invalid filename"),
    };

    Location {
      specifier: filename,
      line: swc_loc.line,
      col: swc_loc.col.0,
    }
  }
}

impl From<Location> for ModuleSpecifier {
  fn from(loc: Location) -> Self {
    resolve_url_or_path(&loc.specifier).unwrap()
  }
}

impl std::fmt::Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}:{}:{}", self.specifier, self.line, self.col)
  }
}

#[derive(Debug)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl std::error::Error for Diagnostics {}

impl fmt::Display for Diagnostics {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, diagnostic) in self.0.iter().enumerate() {
      if i > 0 {
        write!(f, "\n\n")?;
      }

      write!(f, "{}", diagnostic)?
    }

    Ok(())
  }
}

#[derive(Debug, Clone)]
pub enum ImportsNotUsedAsValues {
  Remove,
  Preserve,
  Error,
}

/// Options which can be adjusted when transpiling a module.
#[derive(Debug, Clone)]
pub struct EmitOptions {
  /// When emitting a legacy decorator, also emit experimental decorator meta
  /// data.  Defaults to `false`.
  pub emit_metadata: bool,
  /// What to do with import statements that only import types i.e. whether to
  /// remove them (`Remove`), keep them as side-effect imports (`Preserve`)
  /// or error (`Error`). Defaults to `Remove`.
  pub imports_not_used_as_values: ImportsNotUsedAsValues,
  /// Should the source map be inlined in the emitted code file, or provided
  /// as a separate file.  Defaults to `true`.
  pub inline_source_map: bool,
  /// Should the sources be inlined in the source map.  Defaults to `true`.
  pub inline_sources: bool,
  /// Should a corresponding .map file be created for the output. This should be
  /// false if inline_source_map is true. Defaults to `false`.
  pub source_map: bool,
  /// `true` if the program should use an implicit JSX import source/the "new"
  /// JSX transforms.
  pub jsx_automatic: bool,
  /// If JSX is automatic, if it is in development mode, meaning that it should
  /// import `jsx-dev-runtime` and transform JSX using `jsxDEV` import from the
  /// JSX import source as well as provide additional debug information to the
  /// JSX factory.
  pub jsx_development: bool,
  /// When transforming JSX, what value should be used for the JSX factory.
  /// Defaults to `React.createElement`.
  pub jsx_factory: String,
  /// When transforming JSX, what value should be used for the JSX fragment
  /// factory.  Defaults to `React.Fragment`.
  pub jsx_fragment_factory: String,
  /// The string module specifier to implicitly import JSX factories from when
  /// transpiling JSX.
  pub jsx_import_source: Option<String>,
  /// Should JSX be transformed or preserved.  Defaults to `true`.
  pub transform_jsx: bool,
  /// Should import declarations be transformed to variable declarations.
  /// This should only be set to true for the REPL.  Defaults to `false`.
  pub repl_imports: bool,
}

impl Default for EmitOptions {
  fn default() -> Self {
    EmitOptions {
      emit_metadata: false,
      imports_not_used_as_values: ImportsNotUsedAsValues::Remove,
      inline_source_map: true,
      inline_sources: true,
      source_map: false,
      jsx_automatic: false,
      jsx_development: false,
      jsx_factory: "React.createElement".into(),
      jsx_fragment_factory: "React.Fragment".into(),
      jsx_import_source: None,
      transform_jsx: true,
      repl_imports: false,
    }
  }
}

impl From<config_file::TsConfig> for EmitOptions {
  fn from(config: config_file::TsConfig) -> Self {
    let options: config_file::EmitConfigOptions =
      serde_json::from_value(config.0).unwrap();
    let imports_not_used_as_values =
      match options.imports_not_used_as_values.as_str() {
        "preserve" => ImportsNotUsedAsValues::Preserve,
        "error" => ImportsNotUsedAsValues::Error,
        _ => ImportsNotUsedAsValues::Remove,
      };
    let (transform_jsx, jsx_automatic, jsx_development) =
      match options.jsx.as_str() {
        "react" => (true, false, false),
        "react-jsx" => (true, true, false),
        "react-jsxdev" => (true, true, true),
        _ => (false, false, false),
      };
    EmitOptions {
      emit_metadata: options.emit_decorator_metadata,
      imports_not_used_as_values,
      inline_source_map: options.inline_source_map,
      inline_sources: options.inline_sources,
      source_map: options.source_map,
      jsx_automatic,
      jsx_development,
      jsx_factory: options.jsx_factory,
      jsx_fragment_factory: options.jsx_fragment_factory,
      jsx_import_source: options.jsx_import_source,
      transform_jsx,
      repl_imports: false,
    }
  }
}

fn strip_config_from_emit_options(
  options: &EmitOptions,
) -> typescript::strip::Config {
  typescript::strip::Config {
    pragma: Some(options.jsx_factory.clone()),
    pragma_frag: Some(options.jsx_fragment_factory.clone()),
    import_not_used_as_values: match options.imports_not_used_as_values {
      ImportsNotUsedAsValues::Remove => {
        typescript::strip::ImportsNotUsedAsValues::Remove
      }
      ImportsNotUsedAsValues::Preserve => {
        typescript::strip::ImportsNotUsedAsValues::Preserve
      }
      // `Error` only affects the type-checking stage. Fall back to `Remove` here.
      ImportsNotUsedAsValues::Error => {
        typescript::strip::ImportsNotUsedAsValues::Remove
      }
    },
    use_define_for_class_fields: true,
    // TODO(bartlomieju): this could be changed to `false` to provide `export {}`
    // in Typescript files without manual changes
    no_empty_export: true,
  }
}

/// Implements a configuration trait for source maps that reflects the logic
/// to embed sources in the source map or not.
#[derive(Debug)]
pub(crate) struct SourceMapConfig {
  pub inline_sources: bool,
}

impl deno_ast::swc::common::source_map::SourceMapGenConfig for SourceMapConfig {
  fn file_name_to_source(&self, f: &FileName) -> String {
    f.to_string()
  }

  fn inline_sources_content(&self, f: &FileName) -> bool {
    match f {
      FileName::Real(..) | FileName::Custom(..) => false,
      FileName::Url(..) => self.inline_sources,
      _ => true,
    }
  }
}

/// Transform a TypeScript file into a JavaScript file, based on the supplied
/// options.
///
/// The result is a tuple of the code and optional source map as strings.
pub fn transpile(
  parsed_source: &ParsedSource,
  options: &EmitOptions,
) -> Result<(String, Option<String>), AnyError> {
  ensure_no_fatal_diagnostics(parsed_source.diagnostics().iter())?;
  let program: Program = (*parsed_source.program()).clone();
  let source_map = Rc::new(SourceMap::default());
  let source_map_config = SourceMapConfig {
    inline_sources: options.inline_sources,
  };
  let specifier = resolve_url_or_path(parsed_source.specifier())?;
  let file_name = FileName::Url(specifier);
  source_map
    .new_source_file(file_name, parsed_source.source().text().to_string());
  let comments = parsed_source.comments().as_single_threaded(); // needs to be mutable
  let globals = Globals::new();
  deno_ast::swc::common::GLOBALS.set(&globals, || {
    let top_level_mark = Mark::fresh(Mark::root());
    let module = fold_program(
      program,
      options,
      source_map.clone(),
      &comments,
      top_level_mark,
    )?;

    let mut src_map_buf = vec![];
    let mut buf = vec![];
    {
      let writer = Box::new(JsWriter::new(
        source_map.clone(),
        "\n",
        &mut buf,
        Some(&mut src_map_buf),
      ));
      let config = deno_ast::swc::codegen::Config { minify: false };
      let mut emitter = deno_ast::swc::codegen::Emitter {
        cfg: config,
        comments: Some(&comments),
        cm: source_map.clone(),
        wr: writer,
      };
      module.emit_with(&mut emitter)?;
    }
    let mut src = String::from_utf8(buf)?;
    let mut map: Option<String> = None;
    {
      let mut buf = Vec::new();
      source_map
        .build_source_map_with_config(&mut src_map_buf, None, source_map_config)
        .to_writer(&mut buf)?;

      if options.inline_source_map {
        src.push_str("//# sourceMappingURL=data:application/json;base64,");
        let encoded_map = base64::encode(buf);
        src.push_str(&encoded_map);
      } else {
        map = Some(String::from_utf8(buf)?);
      }
    }
    Ok((src, map))
  })
}

/// A low level function which transpiles a source module into an swc
/// SourceFile.
pub fn transpile_module(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  options: &EmitOptions,
  cm: Rc<SourceMap>,
) -> Result<(Rc<deno_ast::swc::common::SourceFile>, Module), AnyError> {
  let source = strip_bom(source);
  let source = if media_type == MediaType::Json {
    format!(
      "export default JSON.parse(`{}`);",
      source.replace("${", "\\${").replace('`', "\\`")
    )
  } else {
    source.to_string()
  };
  let source_file =
    cm.new_source_file(FileName::Url(specifier.clone()), source);
  let input = StringInput::from(&*source_file);
  let comments = SingleThreadedComments::default();
  let syntax = if media_type == MediaType::Json {
    get_syntax(MediaType::JavaScript)
  } else {
    get_syntax(media_type)
  };
  let lexer = Lexer::new(syntax, deno_ast::ES_VERSION, input, Some(&comments));
  let mut parser = deno_ast::swc::parser::Parser::new_from(lexer);
  let module = parser
    .parse_module()
    .map_err(|e| swc_err_to_diagnostic(&cm, specifier, e))?;
  let diagnostics = parser
    .take_errors()
    .into_iter()
    .map(|e| swc_err_to_diagnostic(&cm, specifier, e))
    .collect::<Vec<_>>();

  ensure_no_fatal_diagnostics(diagnostics.iter())?;

  let top_level_mark = Mark::fresh(Mark::root());
  let program = fold_program(
    Program::Module(module),
    options,
    cm,
    &comments,
    top_level_mark,
  )?;
  let module = match program {
    Program::Module(module) => module,
    _ => unreachable!(),
  };

  Ok((source_file, module))
}

#[derive(Default, Clone)]
struct DiagnosticCollector {
  diagnostics_cell: Rc<RefCell<Vec<SwcDiagnostic>>>,
}

impl DiagnosticCollector {
  pub fn into_handler(self) -> deno_ast::swc::common::errors::Handler {
    deno_ast::swc::common::errors::Handler::with_emitter(
      true,
      false,
      Box::new(self),
    )
  }
}

impl deno_ast::swc::common::errors::Emitter for DiagnosticCollector {
  fn emit(
    &mut self,
    db: &deno_ast::swc::common::errors::DiagnosticBuilder<'_>,
  ) {
    use std::ops::Deref;
    self.diagnostics_cell.borrow_mut().push(db.deref().clone());
  }
}

fn fold_program(
  program: Program,
  options: &EmitOptions,
  source_map: Rc<SourceMap>,
  comments: &SingleThreadedComments,
  top_level_mark: Mark,
) -> Result<Program, AnyError> {
  let jsx_pass = react::react(
    source_map.clone(),
    Some(comments),
    react::Options {
      pragma: options.jsx_factory.clone(),
      pragma_frag: options.jsx_fragment_factory.clone(),
      // this will use `Object.assign()` instead of the `_extends` helper
      // when spreading props.
      use_builtins: true,
      runtime: if options.jsx_automatic {
        Some(react::Runtime::Automatic)
      } else {
        None
      },
      development: options.jsx_development,
      import_source: options.jsx_import_source.clone().unwrap_or_default(),
      ..Default::default()
    },
    top_level_mark,
  );
  let mut passes = chain!(
    Optional::new(transforms::DownlevelImportsFolder, options.repl_imports),
    Optional::new(transforms::StripExportsFolder, options.repl_imports),
    proposals::decorators::decorators(proposals::decorators::Config {
      legacy: true,
      emit_metadata: options.emit_metadata
    }),
    helpers::inject_helpers(),
    resolver_with_mark(top_level_mark),
    Optional::new(
      typescript::strip::strip_with_config(
        strip_config_from_emit_options(options),
        top_level_mark
      ),
      !options.transform_jsx
    ),
    Optional::new(
      typescript::strip::strip_with_jsx(
        source_map.clone(),
        strip_config_from_emit_options(options),
        comments,
        top_level_mark
      ),
      options.transform_jsx
    ),
    Optional::new(jsx_pass, options.transform_jsx),
    fixer(Some(comments)),
    hygiene(),
  );

  let emitter = DiagnosticCollector::default();
  let diagnostics_cell = emitter.diagnostics_cell.clone();
  let handler = emitter.into_handler();
  let result = deno_ast::swc::utils::HANDLER.set(&handler, || {
    helpers::HELPERS.set(&helpers::Helpers::new(false), || {
      program.fold_with(&mut passes)
    })
  });

  let diagnostics = diagnostics_cell.borrow();
  ensure_no_fatal_swc_diagnostics(&source_map, diagnostics.iter())?;
  Ok(result)
}

fn ensure_no_fatal_swc_diagnostics<'a>(
  source_map: &SourceMap,
  diagnostics: impl Iterator<Item = &'a SwcDiagnostic>,
) -> Result<(), AnyError> {
  let fatal_diagnostics = diagnostics
    .filter(|d| is_fatal_swc_diagnostic(d))
    .collect::<Vec<_>>();
  if !fatal_diagnostics.is_empty() {
    Err(anyhow!(
      "{}",
      fatal_diagnostics
        .iter()
        .map(|d| format_swc_diagnostic(source_map, d))
        .collect::<Vec<_>>()
        .join("\n\n")
    ))
  } else {
    Ok(())
  }
}

fn is_fatal_swc_diagnostic(diagnostic: &SwcDiagnostic) -> bool {
  use deno_ast::swc::common::errors::Level;
  match diagnostic.level {
    Level::Bug
    | Level::Cancelled
    | Level::FailureNote
    | Level::Fatal
    | Level::PhaseFatal
    | Level::Error => true,
    Level::Help | Level::Note | Level::Warning => false,
  }
}

fn format_swc_diagnostic(
  source_map: &SourceMap,
  diagnostic: &SwcDiagnostic,
) -> String {
  if let Some(span) = &diagnostic.span.primary_span() {
    let file_name = source_map.span_to_filename(*span);
    let loc = source_map.lookup_char_pos(span.lo);
    format!(
      "{} at {}:{}:{}",
      diagnostic.message(),
      file_name.to_string(),
      loc.line,
      loc.col_display + 1,
    )
  } else {
    diagnostic.message()
  }
}

fn swc_err_to_diagnostic(
  source_map: &SourceMap,
  specifier: &ModuleSpecifier,
  err: SwcError,
) -> Diagnostic {
  let location = source_map.lookup_char_pos(err.span().lo);
  Diagnostic {
    specifier: specifier.to_string(),
    span: err.span(),
    display_position: LineAndColumnDisplay {
      line_number: location.line,
      column_number: location.col_display + 1,
    },
    kind: err.into_kind(),
  }
}

fn ensure_no_fatal_diagnostics<'a>(
  diagnostics: impl Iterator<Item = &'a Diagnostic>,
) -> Result<(), Diagnostics> {
  let fatal_diagnostics = diagnostics
    .filter(|d| is_fatal_syntax_error(&d.kind))
    .map(ToOwned::to_owned)
    .collect::<Vec<_>>();
  if !fatal_diagnostics.is_empty() {
    Err(Diagnostics(fatal_diagnostics))
  } else {
    Ok(())
  }
}

fn is_fatal_syntax_error(error_kind: &SyntaxError) -> bool {
  matches!(
    error_kind,
    // expected identifier
    SyntaxError::TS1003 |
        // expected semi-colon
        SyntaxError::TS1005 |
        // expected expression
        SyntaxError::TS1109 |
        // unterminated string literal
        SyntaxError::UnterminatedStrLit
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_ast::parse_module;
  use deno_ast::ParseParams;
  use deno_ast::SourceTextInfo;

  use pretty_assertions::assert_eq;

  #[test]
  fn test_transpile() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
enum D {
  A,
  B,
}

namespace N {
  export enum D {
    A = "value"
  }
  export const Value = 5;
}

export class A {
  private b: string;
  protected c: number = 1;
  e: "foo";
  constructor (public d = D.A) {
    const e = "foo" as const;
    this.e = e;
    console.log(N.Value);
  }
}
    "#;
    let module = deno_ast::parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    let (code, maybe_map) =
      transpile(&module, &EmitOptions::default()).unwrap();
    let expected_text = r#"var D;
(function(D) {
    D[D["A"] = 0] = "A";
    D[D["B"] = 1] = "B";
})(D || (D = {}));
var N;
(function(N1) {
    let D;
    (function(D) {
        D["A"] = "value";
    })(D = N1.D || (N1.D = {}));
    var Value = N1.Value = 5;
})(N || (N = {}));
export class A {
    d;
    b;
    c = 1;
    e;
    constructor(d = D.A){
        this.d = d;
        const e = "foo";
        this.e = e;
        console.log(N.Value);
    }
}
"#;
    assert_eq!(&code[..expected_text.len()], expected_text);
    assert!(
      code.contains("\n//# sourceMappingURL=data:application/json;base64,")
    );
    assert!(maybe_map.is_none());
  }

  #[test]
  fn test_transpile_tsx() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
    export class A {
      render() {
        return <div><span></span></div>
      }
    }
    "#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Tsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: true, // ensure scope analysis doesn't conflict with a second resolver pass
    })
    .unwrap();
    let (code, _) = transpile(&module, &EmitOptions::default()).unwrap();
    assert!(code.contains("React.createElement(\"div\", null"));
  }

  #[test]
  fn test_transpile_jsx_pragma() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
/** @jsx h */
/** @jsxFrag Fragment */
import { h, Fragment } from "https://deno.land/x/mod.ts";

function App() {
  return (
    <div><></></div>
  );
}"#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Jsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: true,
    })
    .unwrap();
    let (code, _) = transpile(&module, &EmitOptions::default()).unwrap();
    let expected = r#"/** @jsx h */ /** @jsxFrag Fragment */ import { h, Fragment } from "https://deno.land/x/mod.ts";
function App() {
    return(/*#__PURE__*/ h("div", null, /*#__PURE__*/ h(Fragment, null)));
}"#;
    assert_eq!(&code[..expected.len()], expected);
  }

  #[test]
  fn test_transpile_jsx_import_source_pragma() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.tsx").unwrap();
    let source = r#"
/** @jsxImportSource jsx_lib */

function App() {
  return (
    <div><></></div>
  );
}"#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Jsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: true,
    })
    .unwrap();
    let (code, _) = transpile(&module, &EmitOptions::default()).unwrap();
    let expected = r#"import { jsx as _jsx, Fragment as _Fragment } from "jsx_lib/jsx-runtime";
/** @jsxImportSource jsx_lib */ function App() {
    return(/*#__PURE__*/ _jsx("div", {
        children: /*#__PURE__*/ _jsx(_Fragment, {})
    }));
"#;
    assert_eq!(&code[..expected.len()], expected);
  }

  #[test]
  fn test_transpile_jsx_import_source_no_pragma() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.tsx").unwrap();
    let source = r#"
function App() {
  return (
    <div><></></div>
  );
}"#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Jsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: true,
    })
    .unwrap();
    let emit_options = EmitOptions {
      jsx_automatic: true,
      jsx_import_source: Some("jsx_lib".to_string()),
      ..Default::default()
    };
    let (code, _) = transpile(&module, &emit_options).unwrap();
    let expected = r#"import { jsx as _jsx, Fragment as _Fragment } from "jsx_lib/jsx-runtime";
function App() {
    return(/*#__PURE__*/ _jsx("div", {
        children: /*#__PURE__*/ _jsx(_Fragment, {})
    }));
}
"#;
    assert_eq!(&code[..expected.len()], expected);
  }

  // TODO(@kitsonk) https://github.com/swc-project/swc/issues/2656
  //   #[test]
  //   fn test_transpile_jsx_import_source_no_pragma_dev() {
  //     let specifier = resolve_url_or_path("https://deno.land/x/mod.tsx").unwrap();
  //     let source = r#"
  // function App() {
  //   return (
  //     <div><></></div>
  //   );
  // }"#;
  //     let module = parse_module(ParseParams {
  //       specifier: specifier.as_str().to_string(),
  //       source: SourceTextInfo::from_string(source.to_string()),
  //       media_type: deno_ast::MediaType::Jsx,
  //       capture_tokens: false,
  //       maybe_syntax: None,
  //       scope_analysis: true,
  //     })
  //     .unwrap();
  //     let emit_options = EmitOptions {
  //       jsx_automatic: true,
  //       jsx_import_source: Some("jsx_lib".to_string()),
  //       jsx_development: true,
  //       ..Default::default()
  //     };
  //     let (code, _) = transpile(&module, &emit_options).unwrap();
  //     let expected = r#"import { jsx as _jsx, Fragment as _Fragment } from "jsx_lib/jsx-dev-runtime";
  // function App() {
  //     return(/*#__PURE__*/ _jsx("div", {
  //         children: /*#__PURE__*/ _jsx(_Fragment, {
  //         })
  //     }));
  // }
  // "#;
  //     assert_eq!(&code[..expected.len()], expected);
  //   }

  #[test]
  fn test_transpile_decorators() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
    function enumerable(value: boolean) {
      return function (
        _target: any,
        _propertyKey: string,
        descriptor: PropertyDescriptor,
      ) {
        descriptor.enumerable = value;
      };
    }

    export class A {
      @enumerable(false)
      a() {
        Test.value;
      }
    }
    "#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    let (code, _) = transpile(&module, &EmitOptions::default()).unwrap();
    assert!(code.contains("_applyDecoratedDescriptor("));
  }

  #[test]
  fn transpile_handle_code_nested_in_ts_nodes_with_jsx_pass() {
    // from issue 12409
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
export function g() {
  let algorithm: any
  algorithm = {}

  return <Promise>(
    test(algorithm, false, keyUsages)
  )
}
  "#;
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    let emit_options = EmitOptions {
      transform_jsx: true,
      ..Default::default()
    };
    let (code, _) = transpile(&module, &emit_options).unwrap();
    let expected = r#"export function g() {
    let algorithm;
    algorithm = {};
    return test(algorithm, false, keyUsages);
}"#;
    assert_eq!(&code[..expected.len()], expected);
  }

  #[test]
  fn diagnostic_jsx_spread_instead_of_panic() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts").unwrap();
    let source = r#"const A = () => {
  return <div>{...[]}</div>;
};"#;
    let parsed_source = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Tsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    let err = transpile(&parsed_source, &Default::default())
      .err()
      .unwrap();

    assert_eq!(err.to_string(), "Spread children are not supported in React. at https://deno.land/x/mod.ts:2:15");
  }
}
