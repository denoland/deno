// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::msg::MediaType;
use crate::Result;

use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use either::Either;
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;
use swc_common::chain;
use swc_common::comments::Comment;
use swc_common::comments::SingleThreadedComments;
use swc_common::errors::Diagnostic;
use swc_common::errors::DiagnosticBuilder;
use swc_common::errors::Emitter;
use swc_common::errors::Handler;
use swc_common::errors::HandlerFlags;
use swc_common::FileName;
use swc_common::Globals;
use swc_common::Loc;
pub use swc_common::SourceMap;
pub use swc_common::Span;
use swc_ecmascript::ast::Module;
use swc_ecmascript::ast::Program;
use swc_ecmascript::codegen::text_writer::JsWriter;
use swc_ecmascript::codegen::Config;
use swc_ecmascript::codegen::Node;
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::EsConfig;
use swc_ecmascript::parser::JscTarget;
use swc_ecmascript::parser::StringInput;
use swc_ecmascript::parser::Syntax;
use swc_ecmascript::parser::TsConfig;
use swc_ecmascript::transforms::fixer;
use swc_ecmascript::transforms::helpers;
use swc_ecmascript::transforms::proposals::decorators;
use swc_ecmascript::transforms::react;
use swc_ecmascript::transforms::typescript;
use swc_ecmascript::visit;
use swc_ecmascript::visit::FoldWith;
use swc_ecmascript::visit::Visit;

static TARGET: JscTarget = JscTarget::Es2020;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Location {
  pub filename: String,
  pub line: usize,
  pub col: usize,
}

impl Into<Location> for Loc {
  fn into(self) -> Location {
    use swc_common::FileName::*;

    let filename = match &self.file.name {
      Real(path_buf) => path_buf.to_string_lossy().to_string(),
      Custom(str_) => str_.to_string(),
      _ => panic!("invalid filename"),
    };

    Location {
      filename,
      line: self.line,
      col: self.col_display,
    }
  }
}

/// A buffer for collecting diagnostic messages from the AST parser.
#[derive(Debug)]
pub struct DiagnosticBuffer(Vec<String>);

impl Error for DiagnosticBuffer {}

impl fmt::Display for DiagnosticBuffer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = self.0.join(",");
    f.pad(&s)
  }
}

impl DiagnosticBuffer {
  pub fn from_error_buffer<F>(error_buffer: ErrorBuffer, get_loc: F) -> Self
  where
    F: Fn(Span) -> Loc,
  {
    let s = error_buffer.0.read().unwrap().clone();
    let diagnostics = s
      .iter()
      .map(|d| {
        let mut msg = d.message();

        if let Some(span) = d.span.primary_span() {
          let loc = get_loc(span);
          let file_name = match &loc.file.name {
            FileName::Custom(n) => n,
            _ => unreachable!(),
          };
          msg = format!(
            "{} at {}:{}:{}",
            msg, file_name, loc.line, loc.col_display
          );
        }

        msg
      })
      .collect::<Vec<String>>();

    Self(diagnostics)
  }
}

#[derive(Debug, Clone)]
pub struct ErrorBuffer(Arc<RwLock<Vec<Diagnostic>>>);

impl ErrorBuffer {
  pub fn new() -> Self {
    Self(Arc::new(RwLock::new(Vec::new())))
  }
}

impl Emitter for ErrorBuffer {
  fn emit(&mut self, db: &DiagnosticBuilder) {
    self.0.write().unwrap().push((**db).clone());
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

fn get_ts_config(tsx: bool) -> TsConfig {
  TsConfig {
    decorators: true,
    dynamic_import: true,
    tsx,
    ..TsConfig::default()
  }
}

fn get_syntax(media_type: MediaType) -> Syntax {
  match media_type {
    MediaType::JavaScript => Syntax::Es(get_es_config(false)),
    MediaType::JSX => Syntax::Es(get_es_config(true)),
    MediaType::TypeScript => Syntax::Typescript(get_ts_config(false)),
    MediaType::TSX => Syntax::Typescript(get_ts_config(true)),
    _ => Syntax::Es(get_es_config(false)),
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyDescriptor {
  /// A flag indicating if the import is dynamic or not.
  pub is_dynamic: bool,
  /// A flag indicating that the import is a `type` only type of import.
  pub is_type: bool,
  /// Any leading comments associated with the dependency.  This is used for
  /// further processing of supported pragma that impact the dependency.
  pub leading_comments: Vec<Comment>,
  /// The location of the import/export statement.
  pub location: Location,
  /// The text specifier associated with the import/export statement.
  pub specifier: String,
}

/// Visits a pattern node, recursively looking for any names that end up in the
/// local scope, pushing them onto the passed vector.
fn visit_pat(pat: &swc_ecmascript::ast::Pat, names: &mut Vec<String>) {
  match pat {
    swc_ecmascript::ast::Pat::Ident(ident) => names.push(ident.sym.to_string()),
    swc_ecmascript::ast::Pat::Array(array_pat) => {
      for elem in array_pat.elems.iter() {
        if let Some(pat) = elem {
          visit_pat(pat, names);
        }
      }
    }
    swc_ecmascript::ast::Pat::Rest(rest_pat) => {
      visit_pat(rest_pat.arg.as_ref(), names)
    }
    swc_ecmascript::ast::Pat::Object(object_pat) => {
      for prop in object_pat.props.iter() {
        match prop {
          swc_ecmascript::ast::ObjectPatProp::Assign(assign_pat) => {
            names.push(assign_pat.key.sym.to_string())
          }
          swc_ecmascript::ast::ObjectPatProp::KeyValue(key_value) => {
            visit_pat(key_value.value.as_ref(), names)
          }
          swc_ecmascript::ast::ObjectPatProp::Rest(rest_pat) => {
            visit_pat(rest_pat.arg.as_ref(), names)
          }
        }
      }
    }
    swc_ecmascript::ast::Pat::Assign(assign_pat) => {
      visit_pat(assign_pat.left.as_ref(), names)
    }
    // Invalid and Expressions are noops
    _ => {}
  }
}

#[derive(Default)]
struct ExportCollector {
  pub names: Vec<String>,
  pub export_all_specifiers: Vec<String>,
}

impl Visit for ExportCollector {
  fn visit_export_decl(
    &mut self,
    node: &swc_ecmascript::ast::ExportDecl,
    _parent: &dyn visit::Node,
  ) {
    match &node.decl {
      swc_ecmascript::ast::Decl::Class(class_decl) => {
        self.names.push(class_decl.ident.sym.to_string());
      }
      swc_ecmascript::ast::Decl::Fn(fn_decl) => {
        self.names.push(fn_decl.ident.sym.to_string());
      }
      swc_ecmascript::ast::Decl::Var(var_decl) => {
        for decl in var_decl.decls.iter() {
          visit_pat(&decl.name, &mut self.names);
        }
      }
      swc_ecmascript::ast::Decl::TsEnum(ts_enum_decl) => {
        self.names.push(ts_enum_decl.id.sym.to_string());
      }
      // Interfaces, Type Aliases, and TS Module/Namespace decl are noops
      _ => {}
    }
  }

  fn visit_named_export(
    &mut self,
    node: &swc_ecmascript::ast::NamedExport,
    _parent: &dyn visit::Node,
  ) {
    for spec in node.specifiers.iter() {
      match spec {
        swc_ecmascript::ast::ExportSpecifier::Named(named_spec) => {
          if let Some(ident) = &named_spec.exported {
            self.names.push(ident.sym.to_string());
          } else {
            self.names.push(named_spec.orig.sym.to_string());
          }
        }
        swc_ecmascript::ast::ExportSpecifier::Namespace(namespace_spec) => {
          self.names.push(namespace_spec.name.sym.to_string());
        }
        // Default is only proposed syntax, not current supported, so noop
        _ => {}
      }
    }
  }

  fn visit_export_default_decl(
    &mut self,
    node: &swc_ecmascript::ast::ExportDefaultDecl,
    _parent: &dyn visit::Node,
  ) {
    match &node.decl {
      swc_ecmascript::ast::DefaultDecl::Class(_) => {
        self.names.push("default".to_string())
      }
      swc_ecmascript::ast::DefaultDecl::Fn(_) => {
        self.names.push("default".to_string())
      }
      // Interface is a noop
      _ => {}
    }
  }

  fn visit_export_default_expr(
    &mut self,
    _node: &swc_ecmascript::ast::ExportDefaultExpr,
    _parent: &dyn visit::Node,
  ) {
    self.names.push("default".to_string());
  }

  fn visit_export_all(
    &mut self,
    node: &swc_ecmascript::ast::ExportAll,
    _parent: &dyn visit::Node,
  ) {
    self.export_all_specifiers.push(node.src.value.to_string());
  }
}

struct DependencyCollector<'a> {
  comments: &'a SingleThreadedComments,
  pub items: Vec<DependencyDescriptor>,
  source_map: &'a SourceMap,
}

impl<'a> Visit for DependencyCollector<'a> {
  fn visit_import_decl(
    &mut self,
    node: &swc_ecmascript::ast::ImportDecl,
    _parent: &dyn visit::Node,
  ) {
    let specifier = node.src.value.to_string();
    let span = node.span;
    let location: Location = self.source_map.lookup_char_pos(span.lo).into();
    let leading_comments = self
      .comments
      .with_leading(span.lo, |comments| comments.to_vec());
    self.items.push(DependencyDescriptor {
      leading_comments,
      location,
      specifier,
      ..Default::default()
    });
  }

  fn visit_named_export(
    &mut self,
    node: &swc_ecmascript::ast::NamedExport,
    _parent: &dyn visit::Node,
  ) {
    if let Some(src) = &node.src {
      let specifier = src.value.to_string();
      let span = node.span;
      let location: Location = self.source_map.lookup_char_pos(span.lo).into();
      let leading_comments = self
        .comments
        .with_leading(span.lo, |comments| comments.to_vec());
      self.items.push(DependencyDescriptor {
        leading_comments,
        location,
        specifier,
        ..Default::default()
      });
    }
  }

  fn visit_export_all(
    &mut self,
    node: &swc_ecmascript::ast::ExportAll,
    _parent: &dyn visit::Node,
  ) {
    let specifier = node.src.value.to_string();
    let span = node.span;
    let location: Location = self.source_map.lookup_char_pos(span.lo).into();
    let leading_comments = self
      .comments
      .with_leading(span.lo, |comments| comments.to_vec());
    self.items.push(DependencyDescriptor {
      leading_comments,
      location,
      specifier,
      ..Default::default()
    });
  }

  fn visit_ts_import_type(
    &mut self,
    node: &swc_ecmascript::ast::TsImportType,
    _parent: &dyn visit::Node,
  ) {
    let specifier = node.arg.value.to_string();
    let span = node.span;
    let location: Location = self.source_map.lookup_char_pos(span.lo).into();
    let leading_comments = self
      .comments
      .with_leading(span.lo, |comments| comments.to_vec());
    self.items.push(DependencyDescriptor {
      is_type: true,
      leading_comments,
      location,
      specifier,
      ..Default::default()
    });
  }

  fn visit_call_expr(
    &mut self,
    node: &swc_ecmascript::ast::CallExpr,
    _parent: &dyn visit::Node,
  ) {
    use swc_ecmascript::ast::Expr::*;
    use swc_ecmascript::ast::ExprOrSuper::*;

    swc_ecmascript::visit::visit_call_expr(self, node, _parent);
    let call_expr = match node.callee.clone() {
      Super(_) => return,
      Expr(boxed) => boxed,
    };

    match &*call_expr {
      Ident(ident) => {
        if &ident.sym.to_string() != "import" {
          return;
        }
      }
      _ => return,
    };

    if let Some(arg) = node.args.get(0) {
      if let Lit(lit) = &*arg.expr {
        if let swc_ecmascript::ast::Lit::Str(str_) = lit {
          let specifier = str_.value.to_string();
          let span = node.span;
          let location: Location =
            self.source_map.lookup_char_pos(span.lo).into();
          let leading_comments = self
            .comments
            .with_leading(span.lo, |comments| comments.to_vec());
          self.items.push(DependencyDescriptor {
            is_dynamic: true,
            leading_comments,
            location,
            specifier,
            ..Default::default()
          });
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct EmitTranspileOptions {
  /// When emitting a legacy decorator, also emit experimental decorator meta
  /// data.  Defaults to `false`.
  pub emit_metadata: bool,
  /// Should the source map be inlined in the emitted code file, or provided
  /// as a separate file.  Defaults to `true`.
  pub inline_source_map: bool,
  /// When transforming JSX, what value should be used for the JSX factory.
  /// Defaults to `React.createElement`.
  pub jsx_factory: String,
  /// When transforming JSX, what value should be used for the JSX fragment
  /// factory.  Defaults to `React.Fragment`.
  pub jsx_fragment_factory: String,
  /// Should JSX be transformed or preserved.  Defaults to `true`.
  pub transform_jsx: bool,
}

impl Default for EmitTranspileOptions {
  fn default() -> Self {
    EmitTranspileOptions {
      emit_metadata: false,
      inline_source_map: true,
      jsx_factory: "React.createElement".into(),
      jsx_fragment_factory: "React.Fragment".into(),
      transform_jsx: true,
    }
  }
}

/// A logical structure to hold the value of a parsed module for further
/// processing.
#[derive(Clone)]
pub struct ParsedModule {
  comments: SingleThreadedComments,
  /// Leading comments for the module, where references and pragmas might be
  /// contained.
  pub leading_comments: Vec<Comment>,
  module: Module,
  /// The AST source for the module.
  pub source_map: Rc<SourceMap>,
}

impl fmt::Debug for ParsedModule {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "ParsedModule {{ }}")
  }
}

impl ParsedModule {
  /// Identifies all syntactical dependencies of a module (e.g. `import` and
  /// `export` statements) and returns them as a vector of descriptors.
  pub fn get_dependencies(&self) -> Vec<DependencyDescriptor> {
    let mut collector = DependencyCollector {
      comments: &self.comments,
      items: Vec::new(),
      source_map: &self.source_map,
    };
    collector.visit_module(&self.module, &self.module);

    collector.items
  }

  /// Identifies all the names that are exported from a module, or where there
  /// is an export all statement, the string specifier of the re-exported
  /// module.
  pub fn get_export_names(&self) -> (Vec<String>, Vec<String>) {
    let mut collector = ExportCollector::default();
    collector.visit_module(&self.module, &self.module);

    (collector.names, collector.export_all_specifiers)
  }

  /// Transpile a module to JavaScript supportable by Deno, erasing TypeScript
  /// types and performing other transforms.
  pub fn transpile(
    self,
    options: &EmitTranspileOptions,
  ) -> Result<(String, Option<String>)> {
    let program = Program::Module(self.module);
    let base_pass = chain!(
      decorators::decorators(decorators::Config {
        legacy: true,
        emit_metadata: options.emit_metadata,
      }),
      helpers::inject_helpers(),
      typescript::strip(),
      fixer(Some(&self.comments)),
    );
    let jsx_pass = react::react(
      self.source_map.clone(),
      react::Options {
        pragma: options.jsx_factory.clone(),
        pragma_frag: options.jsx_fragment_factory.clone(),
        // this will use `Object.assign()` instead of the `_extends` helper
        // when spreading props.
        use_builtins: true,
        ..Default::default()
      },
    );
    let mut passes = if options.transform_jsx {
      Either::Left(chain!(jsx_pass, base_pass))
    } else {
      Either::Right(base_pass)
    };
    let program = swc_common::GLOBALS.set(&Globals::new(), || {
      helpers::HELPERS.set(&helpers::Helpers::new(false), || {
        program.fold_with(&mut passes)
      })
    });

    let mut mappings = Vec::new();
    let mut buf = Vec::new();
    {
      let wr = Box::new(JsWriter::new(
        self.source_map.clone(),
        "\n",
        &mut buf,
        Some(&mut mappings),
      ));
      let cfg = Config { minify: false };
      let mut emitter = swc_ecmascript::codegen::Emitter {
        cfg,
        comments: Some(&self.comments),
        cm: self.source_map.clone(),
        wr,
      };
      program.emit_with(&mut emitter)?;
    }
    let mut code = String::from_utf8(buf)?;
    let mut map: Option<String> = None;
    {
      let mut buf = Vec::new();
      self
        .source_map
        .build_source_map_from(&mut mappings, None)
        .to_writer(&mut buf)?;

      if options.inline_source_map {
        code.push_str("//# sourceMappingURL=data:application/json;base64,");
        let encoded_map = base64::encode(buf);
        code.push_str(&encoded_map);
      } else {
        map = Some(String::from_utf8(buf)?);
      }
    }

    Ok((code, map))
  }
}

/// For a given specifier, source, and media type, parse the source of the
/// module and return a representation which can be further processed.
///
/// # Arguments
///
/// - `specifier` - The module specifier for the module.
/// - `source` - The source code for the module.
/// - `media_type` - The media type for the module.
///
pub fn parse(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<ParsedModule> {
  let source_map = SourceMap::default();
  let source_file = source_map.new_source_file(
    FileName::Custom(specifier.to_string()),
    source.to_string(),
  );
  let error_buffer = ErrorBuffer::new();
  let syntax = get_syntax(media_type);
  let input = StringInput::from(&*source_file);
  let comments = SingleThreadedComments::default();

  let handler = Handler::with_emitter_and_flags(
    Box::new(error_buffer.clone()),
    HandlerFlags {
      can_emit_warnings: true,
      dont_buffer_diagnostics: true,
      ..HandlerFlags::default()
    },
  );

  let lexer = Lexer::new(syntax, TARGET, input, Some(&comments));
  let mut parser = swc_ecmascript::parser::Parser::new_from(lexer);

  let sm = &source_map;
  let module = parser.parse_module().map_err(move |err| {
    let mut diagnostic = err.into_diagnostic(&handler);
    diagnostic.emit();

    ErrBox::from(DiagnosticBuffer::from_error_buffer(error_buffer, |span| {
      sm.lookup_char_pos(span.lo)
    }))
  })?;
  let leading_comments =
    comments.with_leading(module.span.lo, |comments| comments.to_vec());

  Ok(ParsedModule {
    leading_comments,
    module,
    source_map: Rc::new(source_map),
    comments,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parsed_module_get_dependencies() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.js")
        .unwrap();
    let source = r#"import * as bar from "./test.ts";
    const foo = await import("./foo.ts");
    "#;
    let parsed_module = parse(&specifier, source, MediaType::JavaScript)
      .expect("could not parse module");
    let actual = parsed_module.get_dependencies();
    assert_eq!(
      actual,
      vec![
        DependencyDescriptor {
          is_dynamic: false,
          is_type: false,
          leading_comments: Vec::new(),
          location: Location {
            filename: "https://deno.land/x/mod.js".to_owned(),
            col: 0,
            line: 1,
          },
          specifier: "./test.ts".to_owned()
        },
        DependencyDescriptor {
          is_dynamic: true,
          is_type: false,
          leading_comments: Vec::new(),
          location: Location {
            filename: "https://deno.land/x/mod.js".to_owned(),
            col: 22,
            line: 2,
          },
          specifier: "./foo.ts".to_owned()
        }
      ]
    );
  }

  #[test]
  fn test_parsed_module_get_export_names() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .unwrap();
    let source = r#"
    export * from "./a.ts";

    export { a, b as c } from "./b.ts";

    export default function () {
      console.log("hello");
    }

    export enum C {
      A,
      B,
      C,
    }

    export const [d, e, ...f] = [1, 2, 3, 4, 5];

    export const g = 1;

    export const { h, i: j, ...k } = { h: true, i: false, j: 1, k: 2 };

    export class A {}

    export function l() {}

    export * as m from "./m.ts";
    "#;
    let parsed_module = parse(&specifier, source, MediaType::TypeScript)
      .expect("could not parse module");
    let (names, export_all_specifiers) = parsed_module.get_export_names();
    assert_eq!(
      names,
      vec![
        "a", "c", "default", "C", "d", "e", "f", "g", "h", "j", "k", "A", "l",
        "m"
      ]
    );
    assert_eq!(export_all_specifiers, vec!["./a.ts"]);
  }

  #[test]
  fn test_parsed_module_get_leading_comments() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .unwrap();
    let source = r#"// this is the first comment
      // this is the second comment
      import * as bar from "./test.ts";"#;
    let parsed_module = parse(&specifier, source, MediaType::TypeScript)
      .expect("could not parse module");
    let dependencies = parsed_module.get_dependencies();
    let leading_comments: Vec<&str> = dependencies[0]
      .leading_comments
      .iter()
      .map(|c| c.text.as_str())
      .collect();
    assert_eq!(
      leading_comments,
      vec![" this is the first comment", " this is the second comment"]
    );
  }

  #[test]
  fn test_transpile() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
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
    let module = parse(&specifier, source, MediaType::TypeScript)
      .expect("could not parse module");
    let (code, maybe_map) = module
      .transpile(&EmitTranspileOptions::default())
      .expect("could not strip types");
    assert!(code.starts_with("var D;\n(function(D) {\n"));
    assert!(
      code.contains("\n//# sourceMappingURL=data:application/json;base64,")
    );
    assert!(maybe_map.is_none());
  }

  #[test]
  fn test_transpile_tsx() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .expect("could not resolve specifier");
    let source = r#"
    export class A {
      render() {
        return <div><span></span></div>
      }
    }
    "#;
    let module = parse(&specifier, source, MediaType::TSX)
      .expect("could not parse module");
    let (code, _) = module
      .transpile(&EmitTranspileOptions::default())
      .expect("could not strip types");
    assert!(code.contains("React.createElement(\"div\", null"));
  }

  #[test]
  fn test_transpile_decorators() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
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
    let module = parse(&specifier, source, MediaType::TypeScript)
      .expect("could not parse module");
    let (code, _) = module
      .transpile(&EmitTranspileOptions::default())
      .expect("could not strip types");
    assert!(code.contains("_applyDecoratedDescriptor("));
  }
}
