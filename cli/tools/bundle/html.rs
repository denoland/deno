use deno_ast::swc::common::{self as swc_common, Span};
use std::{
  path::{Path, PathBuf},
  rc::Rc,
};

use deno_ast::swc::atoms::atom;
use deno_core::anyhow;
use swc_common::{BytePos, SourceFile};
use swc_html_ast::{
  Attribute, Child, Comment, Document, DocumentType, Element, Namespace, Text,
};
use swc_html_codegen::writer::basic::BasicHtmlWriter;
use swc_html_codegen::Emit;

use crate::tools::bundle::OutputFile;

#[derive(Debug, Clone)]
pub struct Script {
  pub src: Option<String>,
  pub is_async: bool,
  pub is_module: bool,
  pub is_ignored: bool,
  pub resolved_path: Option<PathBuf>,
}

fn make_attr(name: &str, value: Option<&str>) -> Attribute {
  Attribute {
    name: name.into(),
    value: value.map(|v| v.into()),
    span: Span::default(),
    namespace: None,
    prefix: None,
    raw_name: None,
    raw_value: None,
  }
}

impl Script {
  pub fn to_element(&self) -> Element {
    let mut attributes = Vec::new();
    if let Some(src) = &self.src {
      attributes.push(make_attr("src", Some(src)));
    }

    if self.is_async {
      attributes.push(make_attr("async", None));
    }

    if self.is_module {
      attributes.push(make_attr("type", Some("module")));
    }
    attributes.push(make_attr("crossorigin", None));

    Element {
      attributes,
      children: vec![],
      content: None,
      span: Span::default(),
      tag_name: atom!("script"),
      namespace: Namespace::HTML,
      is_self_closing: false,
    }
  }
}

pub trait VisitHtml: Sized {
  fn visit_element(&mut self, e: &mut Element) {
    walk::element(e, self);
  }
  fn visit_text(&mut self, _t: &mut Text) {}
  fn visit_comment(&mut self, _c: &mut Comment) {}
  fn visit_document_type(&mut self, _d: &mut DocumentType) {}
  fn visit_document(&mut self, d: &mut Document) {
    for child in &mut d.children {
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

  pub fn element(elem: &mut Element, visitor: &mut impl VisitHtml) {
    for e in &mut elem.children {
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
  fn visit_element(&mut self, e: &mut Element) {
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

fn collect_scripts(doc: &mut Document) -> Vec<Script> {
  let mut visitor = Visitor { scripts: vec![] };
  visitor.visit_document(doc);
  visitor.scripts.retain(|s| !s.is_ignored);
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
  let mut doc = swc_html_parser::parse_file_as_document(
    &file,
    swc_html_parser::parser::ParserConfig {
      ..Default::default()
    },
    &mut errors,
  )
  .unwrap();

  let mut scripts = collect_scripts(&mut doc);

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

pub struct Remover {
  to_inject: Script,
  css_to_inject_path: Option<String>,

  injected: bool,
}

impl VisitHtml for Remover {
  fn visit_element(&mut self, e: &mut Element) {
    if e.tag_name == "head" {
      if !self.injected {
        self.injected = true;
        e.children.push(Child::Element(self.to_inject.to_element()));
        if let Some(css_to_inject_path) = &self.css_to_inject_path {
          e.children.push(Child::Element(Element {
            attributes: vec![
              make_attr("rel", Some("stylesheet")),
              make_attr("crossorigin", None),
              make_attr("href", Some(css_to_inject_path)),
            ],
            children: vec![],
            content: None,
            span: Span::default(),
            tag_name: atom!("link"),
            namespace: Namespace::HTML,
            is_self_closing: true,
          }));
        }
      }
    }
    let mut remove = Vec::new();
    for (i, e) in &mut e.children.iter_mut().enumerate() {
      match e {
        Child::Element(element) => {
          if element.tag_name == "script" {
            if get_attr(element, "src").is_some() {
              remove.push(i);
            }
          } else {
            self.visit_element(element);
          }
        }
        _ => {}
      }
    }
  }
}

impl HtmlEntrypoint {
  pub fn to_html(&self) -> String {
    let mut out = String::new();
    let mut s = BasicHtmlWriter::new(
      &mut out,
      None,
      swc_html_codegen::writer::basic::BasicHtmlWriterConfig {
        ..Default::default()
      },
    );
    let mut codegen = swc_html_codegen::CodeGenerator::new(
      &mut s,
      swc_html_codegen::CodegenConfig {
        ..Default::default()
      },
    );
    codegen.emit(&self.doc).unwrap();
    out
  }
  pub fn patch_html_with_response(
    mut self,
    response: &esbuild_client::protocol::BuildResponse,
    outdir: &Path,
  ) -> anyhow::Result<Vec<OutputFile>> {
    let any_async = self.scripts.iter().any(|s| s.is_async);
    let any_module = self.scripts.iter().any(|s| s.is_module);

    let output_files = response.output_files.as_ref().unwrap();

    let entrypoint_js = output_files
      .iter()
      .find(|f| {
        f.path
          .ends_with(&format!("{}stdin.js", std::path::MAIN_SEPARATOR))
      })
      .unwrap();

    let out_name = format!("index-{}.js", reencode_hash(&entrypoint_js.hash));
    let out_path = outdir.join(&out_name);

    let entrypoint_css_maybe = output_files.iter().find(|f| {
      f.path
        .ends_with(&format!("{}stdin.css", std::path::MAIN_SEPARATOR))
    });

    let to_inject = Script {
      src: Some(format!("./{}", out_name)),
      is_async: any_async,
      is_module: any_module,
      is_ignored: false,
      resolved_path: None,
    };

    let mut replacer = Remover {
      to_inject,
      css_to_inject_path: entrypoint_css_maybe
        .map(|f| format!("./index-{}.css", reencode_hash(&f.hash))),
      injected: false,
    };
    replacer.visit_document(&mut self.doc);

    let mut output_files = Vec::new();
    output_files.push(OutputFile {
      path: out_path,
      contents: entrypoint_js.contents.clone(),
    });

    if let Some(css_to_inject_path) = entrypoint_css_maybe {
      let css_path = outdir.join(&format!(
        "index-{}.css",
        reencode_hash(&css_to_inject_path.hash)
      ));
      output_files.push(OutputFile {
        path: css_path,
        contents: css_to_inject_path.contents.clone(),
      });
    }

    output_files.push(OutputFile {
      path: outdir.join("index.html"),
      contents: self.to_html().into_bytes(),
    });

    Ok(output_files)
  }
}

fn reencode_hash(hash: &str) -> String {
  use base64::prelude::*;
  let bytes = BASE64_STANDARD_NO_PAD.decode(hash).unwrap();
  BASE64_URL_SAFE_NO_PAD.encode(bytes)
}
