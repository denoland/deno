use deno_ast::swc::common as swc_common;
use std::{
  path::{Path, PathBuf},
  rc::Rc,
};

use deno_core::anyhow;
use swc_common::{BytePos, SourceFile};
use swc_html_ast::{
  Attribute, Child, Comment, Document, DocumentType, Element, Text,
};
use swc_html_codegen::writer::basic::BasicHtmlWriter;
use swc_html_codegen::Emit;
use swc_html_parser::parse_file_as_document;

#[derive(Debug, Clone)]
pub struct Script {
  pub src: Option<String>,
  pub is_async: bool,
  pub is_module: bool,
  pub is_ignored: bool,
  pub resolved_path: Option<PathBuf>,
}

pub trait VisitHtml: Sized {
  fn visit_element(&mut self, e: &Element) {
    walk::element(e, self);
  }
  fn visit_text(&mut self, _t: &Text) {}
  fn visit_comment(&mut self, _c: &Comment) {}
  fn visit_document_type(&mut self, _d: &DocumentType) {}
  fn visit_document(&mut self, d: &Document) {
    for child in &d.children {
      match child {
        Child::Element(e) => self.visit_element(e),
        Child::Text(t) => self.visit_text(t),
        Child::Comment(c) => self.visit_comment(c),
        Child::DocumentType(d) => self.visit_document_type(d),
      }
    }
  }
}

pub mod walk {
  use swc_html_ast::Element;

  use super::VisitHtml;

  pub fn element(elem: &Element, visitor: &mut impl VisitHtml) {
    for e in &elem.children {
      match e {
        swc_html_ast::Child::DocumentType(document_type) => {
          visitor.visit_document_type(document_type)
        }
        swc_html_ast::Child::Element(element) => visitor.visit_element(element),
        swc_html_ast::Child::Text(text) => visitor.visit_text(text),
        swc_html_ast::Child::Comment(comment) => visitor.visit_comment(comment),
      }
    }
  }
}

struct Visitor {
  scripts: Vec<Script>,
}

fn get_attr<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
  e.attributes
    .iter()
    .find(|a| a.name == name)
    .and_then(|a| a.value.as_ref().map(|v| v.as_str()))
}

impl VisitHtml for Visitor {
  fn visit_element(&mut self, e: &Element) {
    if e.tag_name == "script" {
      let src = get_attr(e, "src");
      let typ = get_attr(e, "type");
      if let (Some(src), Some(typ @ "module")) = (src, typ) {
        self.scripts.push(Script {
          src: Some(src.to_string()),
          is_async: get_attr(e, "async").is_some(),
          is_module: typ == "module",
          is_ignored: get_attr(e, "deno-ignore").is_some()
            || get_attr(e, "vite-ignore").is_some(),
          resolved_path: None,
        });
      }
    }
    walk::element(e, self);
  }
}

fn collect_scripts(doc: &Document) -> Vec<Script> {
  let mut visitor = Visitor { scripts: vec![] };
  visitor.visit_document(doc);
  visitor.scripts
}

#[derive(Debug, Clone)]
pub struct HtmlEntrypoint {
  pub path: PathBuf,
  pub scripts: Vec<Script>,
  pub temp_module: String,
  pub doc: Document,
}

pub fn doit(path: &Path) -> anyhow::Result<HtmlEntrypoint> {
  let file: Rc<deno_ast::swc::common::FileName> =
    Rc::new(PathBuf::from(path).into());
  let file = SourceFile::new(
    file.clone(),
    false,
    file.clone(),
    std::fs::read_to_string(path)?,
    BytePos(1),
  );
  let mut errors = Vec::new();
  let doc = swc_html_parser::parse_file_as_document(
    &file,
    swc_html_parser::parser::ParserConfig {
      ..Default::default()
    },
    &mut errors,
  )
  .unwrap();

  let mut scripts = collect_scripts(&doc);

  let mut temp_module = String::new();
  for script in &mut scripts {
    if let Some(src) = &mut script.src {
      let src = src.trim_start_matches('/');
      let path = path.parent().unwrap().join(src);

      temp_module
        .push_str(&format!("import \"{}\";\n", path.to_string_lossy()));
      script.resolved_path = Some(path);
    }
  }

  Ok(HtmlEntrypoint {
    path: path.to_path_buf(),
    scripts,
    temp_module,
    doc,
  })
}
