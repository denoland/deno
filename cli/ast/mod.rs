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
use deno_ast::swc::common::BytePos;
use deno_ast::swc::common::FileName;
use deno_ast::swc::common::Globals;
use deno_ast::swc::common::Mark;
use deno_ast::swc::common::SourceMap;
use deno_ast::swc::common::Spanned;
use deno_ast::swc::parser::lexer::Lexer;
use deno_ast::swc::parser::StringInput;
use deno_ast::swc::transforms::fixer;
use deno_ast::swc::transforms::helpers;
use deno_ast::swc::transforms::hygiene;
use deno_ast::swc::transforms::pass::Optional;
use deno_ast::swc::transforms::proposals;
use deno_ast::swc::transforms::react;
use deno_ast::swc::transforms::resolver::ts_resolver;
use deno_ast::swc::transforms::typescript;
use deno_ast::swc::visit::FoldWith;
use deno_ast::Diagnostic;
use deno_ast::LineAndColumnDisplay;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
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
      inline_sources: true,
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
      inline_sources: options.inline_sources,
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

/// Transform a TypeScript file into a JavaScript file, based on the supplied
/// options.
///
/// The result is a tuple of the code and optional source map as strings.
pub fn transpile(
  parsed_source: &ParsedSource,
  options: &EmitOptions,
) -> Result<(String, Option<String>), AnyError> {
  let program: Program = (*parsed_source.program()).clone();
  let source_map = Rc::new(SourceMap::default());
  let specifier = resolve_url_or_path(parsed_source.specifier())?;
  let file_name = FileName::Url(specifier);
  source_map
    .new_source_file(file_name, parsed_source.source().text().to_string());
  let comments = parsed_source.comments().as_single_threaded(); // needs to be mutable
  let globals = Globals::new();
  deno_ast::swc::common::GLOBALS.set(&globals, || {
    let top_level_mark = Mark::fresh(Mark::root());
    let jsx_pass = chain!(
      ts_resolver(top_level_mark),
      react::react(
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
        top_level_mark,
      ),
    );
    let mut passes = chain!(
      Optional::new(jsx_pass, options.transform_jsx),
      Optional::new(transforms::DownlevelImportsFolder, options.repl_imports),
      Optional::new(transforms::StripExportsFolder, options.repl_imports),
      proposals::decorators::decorators(proposals::decorators::Config {
        legacy: true,
        emit_metadata: options.emit_metadata
      }),
      helpers::inject_helpers(),
      typescript::strip::strip_with_config(strip_config_from_emit_options(
        options
      )),
      fixer(Some(&comments)),
      hygiene(),
    );

    let program = helpers::HELPERS.set(&helpers::Helpers::new(false), || {
      program.fold_with(&mut passes)
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
      let config = deno_ast::swc::codegen::Config { minify: false };
      let mut emitter = deno_ast::swc::codegen::Emitter {
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
  })
}

/// A low level function which transpiles a source module into an swc
/// SourceFile.
pub fn transpile_module(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  emit_options: &EmitOptions,
  globals: &Globals,
  cm: Rc<SourceMap>,
) -> Result<(Rc<deno_ast::swc::common::SourceFile>, Module), AnyError> {
  let source = strip_bom(source);
  let source_file =
    cm.new_source_file(FileName::Url(specifier.clone()), source.to_string());
  let input = StringInput::from(&*source_file);
  let comments = SingleThreadedComments::default();
  let syntax = get_syntax(media_type);
  let lexer = Lexer::new(syntax, deno_ast::TARGET, input, Some(&comments));
  let mut parser = deno_ast::swc::parser::Parser::new_from(lexer);
  let module = parser.parse_module().map_err(|err| {
    let location = cm.lookup_char_pos(err.span().lo);
    Diagnostic {
      display_position: LineAndColumnDisplay {
        line_number: location.line,
        column_number: location.col_display + 1,
      },
      specifier: specifier.to_string(),
      message: err.into_kind().msg().to_string(),
    }
  })?;

  deno_ast::swc::common::GLOBALS.set(globals, || {
    let top_level_mark = Mark::fresh(Mark::root());
    let jsx_pass = chain!(
      ts_resolver(top_level_mark),
      react::react(
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
        top_level_mark,
      ),
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
      hygiene(),
    );

    let module = helpers::HELPERS.set(&helpers::Helpers::new(false), || {
      module.fold_with(&mut passes)
    });

    Ok((source_file, module))
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_ast::parse_module;
  use deno_ast::ParseParams;
  use deno_ast::SourceTextInfo;

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
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .expect("could not parse module");
    let (code, maybe_map) = transpile(&module, &EmitOptions::default())
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
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::Tsx,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: true, // ensure scope analysis doesn't conflict with a second resolver pass
    })
    .expect("could not parse module");
    let (code, _) = transpile(&module, &EmitOptions::default())
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
    let module = parse_module(ParseParams {
      specifier: specifier.as_str().to_string(),
      source: SourceTextInfo::from_string(source.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .expect("could not parse module");
    let (code, _) = transpile(&module, &EmitOptions::default())
      .expect("could not strip types");
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
    algorithm = {
    };
    return test(algorithm, false, keyUsages);
}"#;
    assert_eq!(&code[..expected.len()], expected);
  }
}
