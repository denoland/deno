// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast::parse;
use crate::ast::Location;
use crate::ast::ParsedModule;
use crate::media_type::MediaType;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_doc::parser::DocFileLoader;
use deno_doc::DocParser;
use jsdoc::ast::Tag;
use jsdoc::Input;
use regex::Regex;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use swc_common::comments::SingleThreadedComments;
use swc_common::SourceMap;
use swc_common::Span;
use swc_ecmascript::ast;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;

use crate::tools::test_runner::is_supported;

lazy_static! {
  // matches non-dynamic js imports
  static ref IMPORT_PATTERN: Regex =
    Regex::new(r#"import\s+?(?:(?:(?:[\w*\s{},]*)\s+from\s+?)|)(?:(?:".*?")|(?:'.*?'))[\s]*?(?:;|$|)"#).unwrap();
  static ref TEST_TAG_PATTERN: Regex = Regex::new(r"```(\w+)?").unwrap();
  static ref AWAIT_PATTERN: Regex = Regex::new(r"\Wawait\s").unwrap();
  // matches jsdoc example caption
  static ref CAPTION_PATTERN: Regex =
      Regex::new(r"<caption>([\s\w\W]+)</caption>").unwrap();
}

struct DocTestVisitor {
  comments: SingleThreadedComments,
  source_map: Rc<SourceMap>,
  examples: RefCell<Vec<(Location, String)>>,
}

struct DocTester {
  doctest_visitor: DocTestVisitor,
  parsed_module: ParsedModule,
}

impl DocTester {
  fn new(parsed_module: ParsedModule) -> Self {
    Self {
      doctest_visitor: DocTestVisitor::new(
        parsed_module.comments.clone(),
        Rc::clone(&parsed_module.source_map),
      ),
      parsed_module,
    }
  }
  pub fn get_comments(&mut self) -> Vec<(Location, String)> {
    let visitor = self.doctest_visitor.borrow_mut();
    visitor
      .visit_module(&self.parsed_module.module, &self.parsed_module.module);
    visitor.examples.clone().into_inner()
  }
}

impl DocTestVisitor {
  fn new(comments: SingleThreadedComments, source_map: Rc<SourceMap>) -> Self {
    Self {
      comments,
      source_map,
      examples: RefCell::new(vec![]),
    }
  }

  fn get_span_location(&self, span: Span) -> Location {
    self.source_map.lookup_char_pos(span.lo()).into()
  }

  fn parse_span_comments(&mut self, span: Span) {
    let comments = self
      .comments
      .with_leading(span.lo, |comments| comments.to_vec());
    let examples = comments
      .iter()
      .filter_map(|comment| {
        jsdoc::parse(Input::from(comment)).map(|op| op.1).ok()
      })
      .flat_map(|js_doc| {
        js_doc
          .tags
          .into_iter()
          .filter_map(|tag_item| match tag_item.tag {
            Tag::Example(ex_tag) => Some((
              self.get_span_location(ex_tag.span),
              ex_tag.text.value.to_string(),
            )),
            _ => None,
          })
      });
    self.examples.borrow_mut().extend(examples);
  }

  fn check_var_decl(
    &mut self,
    var_decl: &ast::VarDecl,
    opt_export_decl: Option<&ast::ExportDecl>,
  ) {
    var_decl.decls.iter().for_each(|decl| {
      if let Some(expr) = &decl.init {
        match &**expr {
          ast::Expr::Object(_)
          | ast::Expr::Fn(_)
          | ast::Expr::Class(_)
          | ast::Expr::Arrow(_) => {
            if let Some(export_decl) = opt_export_decl {
              self.parse_span_comments(export_decl.span);
            } else {
              self.parse_span_comments(var_decl.span);
            }
          }
          _ => {}
        }
      }
    });
  }
}

impl Visit for DocTestVisitor {
  fn visit_class(&mut self, class: &ast::Class, parent: &dyn Node) {
    self.parse_span_comments(class.span);
    swc_ecmascript::visit::visit_class(self, class, parent);
  }

  fn visit_function(&mut self, function: &ast::Function, parent: &dyn Node) {
    self.parse_span_comments(function.span);
    swc_ecmascript::visit::visit_function(self, function, parent);
  }

  fn visit_var_decl(&mut self, var_decl: &ast::VarDecl, parent: &dyn Node) {
    self.check_var_decl(var_decl, None);
    swc_ecmascript::visit::visit_var_decl(self, var_decl, parent);
  }

  fn visit_export_decl(
    &mut self,
    export_decl: &ast::ExportDecl,
    parent: &dyn Node,
  ) {
    match &export_decl.decl {
      ast::Decl::Var(var_decl) => {
        self.check_var_decl(var_decl, Some(export_decl))
      }
      ast::Decl::Class(_) | ast::Decl::Fn(_) => {
        self.parse_span_comments(export_decl.span)
      }
      _ => {}
    }
    swc_ecmascript::visit::visit_export_decl(self, export_decl, parent);
  }
}

pub async fn parse_jsdocs(
  source_files: &[Url],
  loader: Box<dyn DocFileLoader>,
) -> Result<Vec<(Location, String)>, AnyError> {
  let doc_parser = DocParser::new(loader, false);
  let mut results = vec![];
  for url in source_files {
    let source_code =
      doc_parser.loader.load_source_code(&url.to_string()).await?;
    let path = PathBuf::from(&url.to_string());
    let media_type = MediaType::from(&path);
    let specifier = ModuleSpecifier::resolve_url(&url.to_string())?;
    let parsed_module = parse(specifier.as_str(), &source_code, &media_type)?;
    let mut doc_tester = DocTester::new(parsed_module);
    results.extend(doc_tester.get_comments());
  }
  Ok(results)
}

pub fn prepare_doctests(
  jsdocs: Vec<(Location, String)>,
  fail_fast: bool,
  quiet: bool,
  filter: Option<String>,
) -> Result<String, AnyError> {
  let mut test_file = "".to_string();
  let mut import_set = HashSet::new();

  let cwd = std::env::current_dir()?;
  let cwd_url_str = Url::from_directory_path(cwd)
    .map(|url| url.to_string())
    .unwrap_or_else(|_| "".to_string());

  let tests: String = jsdocs
    .into_iter()
    .filter_map(|(loc, example)| {
      let ex_str = clean_string(&example);
      let test_tag = extract_test_tag(&ex_str);
      if test_tag == Some("text") {
        return  None;
      }
      let ignore = test_tag == Some("ignore");
      extract_imports(&ex_str).into_iter().for_each(|import| { import_set.insert(import.to_string()); });
      let caption = extract_caption(&ex_str);
      let code_body = extract_code_body(&ex_str);
      let is_async = has_await(&ex_str);
      let res = format!(
        "Deno.test({{\n\tname: \"{} - {} (line {})\",\n\tignore: {},\n\t{} fn() {{\n{}\n}}\n}});\n",
        loc.filename.replace(&cwd_url_str, ""),
        caption.unwrap_or(""),
        loc.line,
        ignore,
        if is_async { "async"} else {""},
        code_body
    );
    Some(res)
    })
    .collect();
  let imports_str = import_set.into_iter().collect::<Vec<_>>().join("\n");
  test_file.push_str(&imports_str);
  test_file.push('\n');
  test_file.push_str(&tests);
  test_file.push('\n');
  test_file.push_str("// @ts-ignore\n");

  let options = if let Some(filter) = filter {
    json!({ "failFast": fail_fast, "reportToConsole": !quiet, "disableLog": quiet, "isDoctest": true, "filter": filter })
  } else {
    json!({ "failFast": fail_fast, "reportToConsole": !quiet, "disableLog": quiet, "isDoctest": true })
  };

  let run_tests_cmd =
    format!("await Deno[Deno.internal].runTests({});\n", options);

  test_file.push_str(&run_tests_cmd);
  Ok(test_file)
}

fn clean_string(input: &str) -> String {
  input
    .lines()
    .map(|line| {
      if line.trim().starts_with('*') {
        &line.trim()[1..]
      } else {
        line.trim()
      }
    })
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn is_supported_doctest(path: &Path) -> bool {
  let valid_ext = ["ts", "tsx", "js", "jsx"];
  path
    .extension()
    .and_then(OsStr::to_str)
    .map(|ext| valid_ext.contains(&ext) && !is_supported(path))
    .unwrap_or(false)
}

fn extract_test_tag(input: &str) -> Option<&str> {
  TEST_TAG_PATTERN
    .captures(input)
    .and_then(|m| m.get(1).map(|c| c.as_str()))
}

fn extract_caption(input: &str) -> Option<&str> {
  CAPTION_PATTERN
    .captures(input)
    .and_then(|m| m.get(1).map(|c| c.as_str()))
}

fn extract_imports(input: &str) -> Vec<&str> {
  IMPORT_PATTERN
    .captures_iter(input)
    .filter_map(|caps| caps.get(0).map(|m| m.as_str()))
    .collect()
}

fn has_await(input: &str) -> bool {
  AWAIT_PATTERN.find(input).is_some()
}

fn extract_code_body(ex: &str) -> String {
  let code_sans_imports = IMPORT_PATTERN
    .replace_all(ex, "\n")
    .lines()
    .filter_map(|line| {
      let res = line.trim();
      if !line.is_empty() {
        Some(res)
      } else {
        None
      }
    })
    .collect::<Vec<_>>()
    .join("\n");
  let code_sans_tag = TEST_TAG_PATTERN.replace_all(&code_sans_imports, "");
  CAPTION_PATTERN
    .replace(&code_sans_tag, "")
    .trim()
    .to_string()
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn test_extract_fns() {
    let test = r#"<caption>Linkedlists.compareWith</caption>
    * ```ts
    * import { LinkedList } from './js_test/linkedlist.ts'
    * const testArr = [1, 2, 3, 4, 5, 6, 78, 9, 0, 65];
    * const firstList = new LinkedList<number>();
    * const secondList = new LinkedList<number>();
    * for (let data of testArr) {
    *   firstList.insertNode(data);
    *   secondList.insertNode(data);
    * }
    * const result = firstList.compareWith(secondList);
    * assert.assert(result);
    ```"#;
    let res = clean_string(test);
    assert_eq!(extract_caption(&res), Some("Linkedlists.compareWith"));
    assert_eq!(
      extract_imports(&res),
      vec!["import { LinkedList } from './js_test/linkedlist.ts'"]
    );
    assert!(!has_await(&res));
    assert_eq!(extract_test_tag(&res), Some("ts"));
    assert_eq!(
      extract_code_body(&res),
      vec![
        "const testArr = [1, 2, 3, 4, 5, 6, 78, 9, 0, 65];",
        "const firstList = new LinkedList<number>();",
        "const secondList = new LinkedList<number>();",
        "for (let data of testArr) {",
        "firstList.insertNode(data);",
        "secondList.insertNode(data);",
        "}",
        "const result = firstList.compareWith(secondList);",
        "assert.assert(result);"
      ]
      .join("\n")
    )
  }

  #[test]
  fn test_async_detection() {
    let test = r#"
    ```ts
    const response = await fetch("https://deno.land");
    const body = await response.text();
    assert(body.length > 0);
    ```"#;
    assert!(has_await(&test));
  }
}
