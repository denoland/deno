// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::config_file;
use crate::media_type::MediaType;
use crate::text_encoding::strip_bom;

use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use swc_common::chain;
use swc_common::comments::Comment;
use swc_common::comments::CommentKind;
use swc_common::comments::Comments;
use swc_common::comments::SingleThreadedComments;
use swc_common::BytePos;
use swc_common::FileName;
use swc_common::Globals;
use swc_common::SourceFile;
use swc_common::SourceMap;
use swc_common::Span;
use swc_common::Spanned;
use swc_ecmascript::ast::Module;
use swc_ecmascript::ast::Program;
use swc_ecmascript::codegen::text_writer::JsWriter;
use swc_ecmascript::codegen::Node;
use swc_ecmascript::dep_graph::analyze_dependencies;
use swc_ecmascript::dep_graph::DependencyDescriptor;
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::token::Token;
use swc_ecmascript::parser::EsConfig;
use swc_ecmascript::parser::JscTarget;
use swc_ecmascript::parser::StringInput;
use swc_ecmascript::parser::Syntax;
use swc_ecmascript::parser::TsConfig;
use swc_ecmascript::transforms::fixer;
use swc_ecmascript::transforms::helpers;
use swc_ecmascript::transforms::hygiene;
use swc_ecmascript::transforms::pass::Optional;
use swc_ecmascript::transforms::proposals;
use swc_ecmascript::transforms::react;
use swc_ecmascript::transforms::typescript;
use swc_ecmascript::visit::FoldWith;

mod bundle_hook;
mod comments;
mod source_file_info;
mod transforms;

pub use bundle_hook::BundleHook;
use comments::MultiThreadedComments;
use source_file_info::SourceFileInfo;

static TARGET: JscTarget = JscTarget::Es2020;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Location {
  pub specifier: String,
  pub line: usize,
  pub col: usize,
}

impl From<swc_common::Loc> for Location {
  fn from(swc_loc: swc_common::Loc) -> Self {
    use swc_common::FileName::*;

    let filename = match &swc_loc.file.name {
      Real(path_buf) => path_buf.to_string_lossy().to_string(),
      Custom(str_) => str_.to_string(),
      _ => panic!("invalid filename"),
    };

    Location {
      specifier: filename,
      line: swc_loc.line,
      col: swc_loc.col_display,
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

/// A diagnostic from the AST parser.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Diagnostic {
  pub location: Location,
  pub message: String,
}

impl Error for Diagnostic {}

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} at {}", self.message, self.location)
  }
}

fn get_es_config(jsx: bool) -> EsConfig {
  EsConfig {
    class_private_methods: true,
    class_private_props: true,
    class_props: true,
    dynamic_import: true,
    export_default_from: true,
    export_namespace_from: true,
    import_meta: true,
    jsx,
    nullish_coalescing: true,
    num_sep: true,
    optional_chaining: true,
    top_level_await: true,
    ..EsConfig::default()
  }
}

fn get_ts_config(tsx: bool, dts: bool) -> TsConfig {
  TsConfig {
    decorators: true,
    dts,
    dynamic_import: true,
    tsx,
    ..TsConfig::default()
  }
}

pub fn get_syntax(media_type: &MediaType) -> Syntax {
  match media_type {
    MediaType::JavaScript => Syntax::Es(get_es_config(false)),
    MediaType::Jsx => Syntax::Es(get_es_config(true)),
    MediaType::TypeScript => Syntax::Typescript(get_ts_config(false, false)),
    MediaType::Dts => Syntax::Typescript(get_ts_config(false, true)),
    MediaType::Tsx => Syntax::Typescript(get_ts_config(true, false)),
    _ => Syntax::Es(get_es_config(false)),
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
  // Should a corresponding .map file be created for the output. This should be
  // false if inline_source_map is true. Defaults to `false`.
  pub source_map: bool,
  /// When transforming JSX, what value should be used for the JSX factory.
  /// Defaults to `React.createElement`.
  pub jsx_factory: String,
  /// When transforming JSX, what value should be used for the JSX fragment
  /// factory.  Defaults to `React.Fragment`.
  pub jsx_fragment_factory: String,
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
      source_map: false,
      jsx_factory: "React.createElement".into(),
      jsx_fragment_factory: "React.Fragment".into(),
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
    EmitOptions {
      emit_metadata: options.emit_decorator_metadata,
      imports_not_used_as_values,
      inline_source_map: options.inline_source_map,
      source_map: options.source_map,
      jsx_factory: options.jsx_factory,
      jsx_fragment_factory: options.jsx_fragment_factory,
      transform_jsx: options.jsx == "react",
      repl_imports: false,
    }
  }
}

fn strip_config_from_emit_options(
  options: &EmitOptions,
) -> typescript::strip::Config {
  typescript::strip::Config {
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

/// A logical structure to hold the value of a parsed module for further
/// processing.
#[derive(Clone)]
pub struct ParsedModule {
  info: Arc<SourceFileInfo>,
  comments: MultiThreadedComments,
  pub module: Module,
}

impl fmt::Debug for ParsedModule {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("ParsedModule")
      .field("comments", &self.comments)
      .field("module", &self.module)
      .finish()
  }
}

impl ParsedModule {
  /// Return a vector of dependencies for the module.
  pub fn analyze_dependencies(&self) -> Vec<DependencyDescriptor> {
    analyze_dependencies(&self.module, &self.comments)
  }

  /// Get the module's leading comments, where triple slash directives might
  /// be located.
  pub fn get_leading_comments(&self) -> Vec<Comment> {
    self
      .comments
      .get_leading(self.module.span.lo)
      .unwrap_or_else(Vec::new)
  }

  /// Get the module's comments sorted by position.
  pub fn get_comments(&self) -> Vec<Comment> {
    self.comments.get_vec()
  }

  /// Get a location for a given position within the module.
  pub fn get_location(&self, pos: BytePos) -> Location {
    self.info.get_location(pos)
  }

  /// Transform a TypeScript file into a JavaScript file, based on the supplied
  /// options.
  ///
  /// The result is a tuple of the code and optional source map as strings.
  pub fn transpile(
    self,
    options: &EmitOptions,
  ) -> Result<(String, Option<String>), AnyError> {
    let program = Program::Module(self.module);
    let source_map = Rc::new(SourceMap::default());
    let file_name = FileName::Custom(self.info.specifier.clone());
    source_map.new_source_file(file_name, self.info.text.clone());
    let comments = self.comments.as_single_threaded(); // needs to be mutable

    let jsx_pass = react::react(
      source_map.clone(),
      Some(&comments),
      react::Options {
        pragma: options.jsx_factory.clone(),
        pragma_frag: options.jsx_fragment_factory.clone(),
        // this will use `Object.assign()` instead of the `_extends` helper
        // when spreading props.
        use_builtins: true,
        ..Default::default()
      },
    );
    let mut passes = chain!(
      Optional::new(jsx_pass, options.transform_jsx),
      Optional::new(transforms::DownlevelImportsFolder, options.repl_imports),
      Optional::new(transforms::StripExportsFolder, options.repl_imports),
      proposals::decorators::decorators(proposals::decorators::Config {
        legacy: true,
        emit_metadata: options.emit_metadata
      }),
      // DownlevelImportsFolder::new(), // todo: make this conditional
      helpers::inject_helpers(),
      typescript::strip::strip_with_config(strip_config_from_emit_options(
        options
      )),
      fixer(Some(&comments)),
      hygiene(),
    );

    let program = swc_common::GLOBALS.set(&Globals::new(), || {
      helpers::HELPERS.set(&helpers::Helpers::new(false), || {
        program.fold_with(&mut passes)
      })
    });

    let mut src_map_buf = vec![];
    let mut buf = vec![];
    {
      let writer = Box::new(JsWriter::new(
        source_map.clone(),
        "\n",
        &mut buf,
        Some(&mut src_map_buf),
      ));
      let config = swc_ecmascript::codegen::Config { minify: false };
      let mut emitter = swc_ecmascript::codegen::Emitter {
        cfg: config,
        comments: Some(&comments),
        cm: source_map.clone(),
        wr: writer,
      };
      program.emit_with(&mut emitter)?;
    }
    let mut src = String::from_utf8(buf)?;
    let mut map: Option<String> = None;
    {
      let mut buf = Vec::new();
      source_map
        .build_source_map_from(&mut src_map_buf, None)
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
  }
}

/// For a given specifier, source, and media type, parse the text of the
/// module and return a representation which can be further processed.
///
/// # Arguments
///
/// - `specifier` - The module specifier for the module.
/// - `source` - The source code for the module.
/// - `media_type` - The media type for the module.
///
// NOTE(bartlomieju): `specifier` has `&str` type instead of
// `&ModuleSpecifier` because runtime compiler APIs don't
// require valid module specifiers
pub fn parse(
  specifier: &str,
  source: &str,
  media_type: &MediaType,
) -> Result<ParsedModule, AnyError> {
  let source = strip_bom(source);
  let info = SourceFileInfo::new(specifier, source);
  let input =
    StringInput::new(source, BytePos(0), BytePos(source.len() as u32));
  let (comments, module) =
    parse_string_input(input, media_type).map_err(|err| Diagnostic {
      location: info.get_location(err.span().lo),
      message: err.into_kind().msg().to_string(),
    })?;

  Ok(ParsedModule {
    info: Arc::new(info),
    comments: MultiThreadedComments::from_single_threaded(comments),
    module,
  })
}

pub enum TokenOrComment {
  Token(Token),
  Comment { kind: CommentKind, text: String },
}

pub struct LexedItem {
  pub span: Span,
  pub inner: TokenOrComment,
}

impl LexedItem {
  pub fn span_as_range(&self) -> Range<usize> {
    self.span.lo.0 as usize..self.span.hi.0 as usize
  }
}

fn flatten_comments(
  comments: SingleThreadedComments,
) -> impl Iterator<Item = Comment> {
  let (leading, trailing) = comments.take_all();
  let mut comments = (*leading).clone().into_inner();
  comments.extend((*trailing).clone().into_inner());
  comments.into_iter().flat_map(|el| el.1)
}

pub fn lex(source: &str, media_type: &MediaType) -> Vec<LexedItem> {
  let comments = SingleThreadedComments::default();
  let lexer = Lexer::new(
    get_syntax(media_type),
    TARGET,
    StringInput::new(source, BytePos(0), BytePos(source.len() as u32)),
    Some(&comments),
  );

  let mut tokens: Vec<LexedItem> = lexer
    .map(|token| LexedItem {
      span: token.span,
      inner: TokenOrComment::Token(token.token),
    })
    .collect();

  tokens.extend(flatten_comments(comments).map(|comment| LexedItem {
    span: comment.span,
    inner: TokenOrComment::Comment {
      kind: comment.kind,
      text: comment.text,
    },
  }));

  tokens.sort_by_key(|item| item.span.lo.0);

  tokens
}

/// A low level function which transpiles a source module into an swc
/// SourceFile.
pub fn transpile_module(
  specifier: &str,
  source: &str,
  media_type: &MediaType,
  emit_options: &EmitOptions,
  globals: &Globals,
  cm: Rc<SourceMap>,
) -> Result<(Rc<SourceFile>, Module), AnyError> {
  let source = strip_bom(source);
  let source_file = cm.new_source_file(
    FileName::Custom(specifier.to_string()),
    source.to_string(),
  );
  let input = StringInput::from(&*source_file);
  let (comments, module) =
    parse_string_input(input, media_type).map_err(|err| Diagnostic {
      location: cm.lookup_char_pos(err.span().lo).into(),
      message: err.into_kind().msg().to_string(),
    })?;

  let jsx_pass = react::react(
    cm,
    Some(&comments),
    react::Options {
      pragma: emit_options.jsx_factory.clone(),
      pragma_frag: emit_options.jsx_fragment_factory.clone(),
      // this will use `Object.assign()` instead of the `_extends` helper
      // when spreading props.
      use_builtins: true,
      ..Default::default()
    },
  );
  let mut passes = chain!(
    Optional::new(jsx_pass, emit_options.transform_jsx),
    proposals::decorators::decorators(proposals::decorators::Config {
      legacy: true,
      emit_metadata: emit_options.emit_metadata
    }),
    helpers::inject_helpers(),
    typescript::strip::strip_with_config(strip_config_from_emit_options(
      emit_options
    )),
    fixer(Some(&comments)),
  );

  let module = swc_common::GLOBALS.set(globals, || {
    helpers::HELPERS.set(&helpers::Helpers::new(false), || {
      module.fold_with(&mut passes)
    })
  });

  Ok((source_file, module))
}

fn parse_string_input(
  input: StringInput,
  media_type: &MediaType,
) -> Result<
  (SingleThreadedComments, Module),
  swc_ecmascript::parser::error::Error,
> {
  let syntax = get_syntax(media_type);
  let comments = SingleThreadedComments::default();
  let lexer = Lexer::new(syntax, TARGET, input, Some(&comments));
  let mut parser = swc_ecmascript::parser::Parser::new_from(lexer);
  let module = parser.parse_module()?;

  Ok((comments, module))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;
  use swc_common::BytePos;
  use swc_ecmascript::dep_graph::DependencyKind;

  #[test]
  fn test_parsed_module_analyze_dependencies() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.js").unwrap();
    let source = "import * as bar from './test.ts';\nconst foo = await import('./foo.ts');";
    let parsed_module =
      parse(specifier.as_str(), source, &MediaType::JavaScript)
        .expect("could not parse module");
    let actual = parsed_module.analyze_dependencies();
    assert_eq!(
      actual,
      vec![
        DependencyDescriptor {
          kind: DependencyKind::Import,
          is_dynamic: false,
          leading_comments: Vec::new(),
          span: Span::new(BytePos(0), BytePos(33), Default::default()),
          specifier: "./test.ts".into(),
          specifier_span: Span::new(
            BytePos(21),
            BytePos(32),
            Default::default()
          ),
          import_assertions: HashMap::default(),
        },
        DependencyDescriptor {
          kind: DependencyKind::Import,
          is_dynamic: true,
          leading_comments: Vec::new(),
          span: Span::new(BytePos(52), BytePos(70), Default::default()),
          specifier: "./foo.ts".into(),
          specifier_span: Span::new(
            BytePos(59),
            BytePos(69),
            Default::default()
          ),
          import_assertions: HashMap::default(),
        }
      ]
    );
  }

  #[test]
  fn test_transpile() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts")
      .expect("could not resolve specifier");
    let source = r#"
    enum D {
      A,
      B,
      C,
    }

    export class A {
      private b: string;
      protected c: number = 1;
      e: "foo";
      constructor (public d = D.A) {
        const e = "foo" as const;
        this.e = e;
      }
    }
    "#;
    let module = parse(specifier.as_str(), source, &MediaType::TypeScript)
      .expect("could not parse module");
    let (code, maybe_map) = module
      .transpile(&EmitOptions::default())
      .expect("could not strip types");
    assert!(code.starts_with("var D;\n(function(D) {\n"));
    assert!(
      code.contains("\n//# sourceMappingURL=data:application/json;base64,")
    );
    assert!(maybe_map.is_none());
  }

  #[test]
  fn test_transpile_tsx() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts")
      .expect("could not resolve specifier");
    let source = r#"
    export class A {
      render() {
        return <div><span></span></div>
      }
    }
    "#;
    let module = parse(specifier.as_str(), source, &MediaType::Tsx)
      .expect("could not parse module");
    let (code, _) = module
      .transpile(&EmitOptions::default())
      .expect("could not strip types");
    assert!(code.contains("React.createElement(\"div\", null"));
  }

  #[test]
  fn test_transpile_decorators() {
    let specifier = resolve_url_or_path("https://deno.land/x/mod.ts")
      .expect("could not resolve specifier");
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
    let module = parse(specifier.as_str(), source, &MediaType::TypeScript)
      .expect("could not parse module");
    let (code, _) = module
      .transpile(&EmitOptions::default())
      .expect("could not strip types");
    assert!(code.contains("_applyDecoratedDescriptor("));
  }
}
