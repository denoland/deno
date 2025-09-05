// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::path::Path;
use std::path::PathBuf;

use capacity_builder::StringBuilder;
use deno_core::anyhow;
use deno_core::error::AnyError;
use lol_html::element;
use lol_html::html_content::ContentType as LolContentType;

use crate::tools::bundle::OutputFile;

#[derive(Debug, Clone)]
pub struct Script {
  pub src: Option<String>,
  pub is_async: bool,
  pub is_module: bool,
  pub resolved_path: Option<PathBuf>,
}

struct Attr<'a> {
  name: Cow<'static, str>,
  value: Option<Cow<'a, str>>,
}

impl<'a> Attr<'a> {
  fn new(
    name: impl Into<Cow<'static, str>>,
    value: Option<Cow<'a, str>>,
  ) -> Self {
    Self {
      name: name.into(),
      value,
    }
  }
  fn write_out<'s>(&'s self, out: &mut StringBuilder<'s>)
  where
    'a: 's,
  {
    out.append(&self.name);
    if let Some(value) = &self.value {
      out.append("=\"");
      out.append(value);
      out.append('"');
    }
  }
}

fn write_attr_list<'a, 's>(attrs: &'s [Attr<'a>], out: &mut StringBuilder<'s>)
where
  'a: 's,
{
  if attrs.is_empty() {
    return;
  }

  out.append(' ');
  for i in 0..attrs.len() - 1 {
    attrs[i].write_out(out);
    out.append(' ');
  }

  attrs[attrs.len() - 1].write_out(out);
}

impl Script {
  pub fn to_element_string(&self) -> String {
    let mut attrs = Vec::new();
    if let Some(src) = &self.src {
      attrs.push(Attr::new("src", Some(Cow::Borrowed(src))));
    }
    if self.is_async {
      attrs.push(Attr::new("async", None));
    }
    if self.is_module {
      attrs.push(Attr::new("type", Some("module".into())));
    }
    attrs.push(Attr::new("crossorigin", None));
    StringBuilder::build(|out| {
      out.append("<script");

      write_attr_list(&attrs, out);

      out.append("></script>");
    })
    .unwrap()
  }
}

struct NoOutput;

impl lol_html::OutputSink for NoOutput {
  fn handle_chunk(&mut self, _: &[u8]) {}
}

fn collect_scripts(doc: &str) -> Result<Vec<Script>, AnyError> {
  let mut scripts = Vec::new();
  let mut rewriter = lol_html::HtmlRewriter::new(
    lol_html::Settings {
      element_content_handlers: vec![element!("script[src]", |el| {
        let is_ignored =
          el.has_attribute("deno-ignore") || el.has_attribute("vite-ignore");
        if is_ignored {
          return Ok(());
        }
        let typ = el.get_attribute("type");
        let (Some("module") | None) = typ.as_deref() else {
          return Ok(());
        };
        let src = el.get_attribute("src").unwrap();
        let is_async = el.has_attribute("async");
        let is_module = matches!(typ.as_deref(), Some("module"));

        scripts.push(Script {
          src: Some(src),
          is_async,
          is_module,
          resolved_path: None,
        });
        Ok(())
      })],
      ..lol_html::Settings::new()
    },
    NoOutput,
  );
  rewriter.write(doc.as_bytes())?;
  rewriter.end()?;
  Ok(scripts)
}

#[derive(Debug, Clone)]
pub struct HtmlEntrypoint {
  pub path: PathBuf,
  pub scripts: Vec<Script>,
  pub temp_module: String,
  pub contents: String,
}

fn parse_html_entrypoint(
  path: &Path,
  contents: String,
) -> anyhow::Result<HtmlEntrypoint> {
  let mut scripts = collect_scripts(&contents)?;

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
    contents,
  })
}

pub fn load_html_entrypoint(path: &Path) -> anyhow::Result<HtmlEntrypoint> {
  let contents = std::fs::read_to_string(path)?;
  parse_html_entrypoint(path, contents)
}

impl HtmlEntrypoint {
  pub fn patch_html_with_response<'a>(
    self,
    response: &'a esbuild_client::protocol::BuildResponse,
    outdir: &Path,
  ) -> anyhow::Result<Vec<OutputFile<'a>>> {
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
      resolved_path: None,
    };

    let css_to_inject_path = entrypoint_css_maybe
      .map(|f| format!("./index-{}.css", reencode_hash(&f.hash)));

    let patched_contents = inject_scripts_and_css(
      &self.contents,
      to_inject,
      &self.scripts,
      css_to_inject_path,
    )?;

    let mut output_files = Vec::new();
    output_files.push(OutputFile {
      path: out_path,
      contents: Cow::Borrowed(&entrypoint_js.contents),
    });

    if let Some(css_to_inject_path) = entrypoint_css_maybe {
      let css_path = outdir.join(format!(
        "index-{}.css",
        reencode_hash(&css_to_inject_path.hash)
      ));
      output_files.push(OutputFile {
        path: css_path,
        contents: Cow::Borrowed(&css_to_inject_path.contents),
      });
    }

    output_files.push(OutputFile {
      path: outdir.join("index.html"),
      contents: patched_contents.into_bytes().into(),
    });

    Ok(output_files)
  }
}

fn make_link_str(attrs: &[Attr]) -> String {
  StringBuilder::build(|out| {
    out.append("<link");
    write_attr_list(attrs, out);
    out.append(">");
  })
  .unwrap()
}

fn stylesheet_str(path: &str) -> String {
  let attrs = &[
    Attr::new("rel", Some("stylesheet".into())),
    Attr::new("crossorigin", None),
    Attr::new("href", Some(Cow::Borrowed(path))),
  ];
  make_link_str(attrs)
}

fn inject_scripts_and_css(
  input: &str,
  to_inject: Script,
  to_remove: &[Script],
  css_to_inject_path: Option<String>,
) -> anyhow::Result<String> {
  let did_inject = Cell::new(false);
  let rewritten = lol_html::rewrite_str(
    input,
    lol_html::Settings {
      element_content_handlers: vec![
        element!("head", |el| {
          let already_done = did_inject.replace(true);
          if already_done {
            return Ok(());
          }
          el.append(&to_inject.to_element_string(), LolContentType::Html);

          if let Some(css_to_inject_path) = &css_to_inject_path {
            let link = stylesheet_str(css_to_inject_path);
            el.append(&link, LolContentType::Html);
          }

          Ok(())
        }),
        element!("script[src]", |el| {
          let src = el.get_attribute("src").unwrap();
          if to_remove
            .iter()
            .any(|script| script.src.as_deref() == Some(src.as_str()))
          {
            el.remove();
          }
          Ok(())
        }),
      ],
      document_content_handlers: vec![lol_html::end!(|end| {
        if !did_inject.replace(true) {
          let script = to_inject.to_element_string();
          let link = css_to_inject_path
            .as_ref()
            .map(|p| stylesheet_str(p))
            .unwrap_or_default();
          end.append(
            &format!("<head>{script}{link}</head>"),
            LolContentType::Html,
          );
        }
        Ok(())
      })],
      ..lol_html::Settings::new()
    },
  )?;
  Ok(rewritten)
}

fn reencode_hash(hash: &str) -> String {
  use base64::prelude::*;
  let bytes = BASE64_STANDARD_NO_PAD.decode(hash).unwrap();
  BASE64_URL_SAFE_NO_PAD.encode(bytes)
}
