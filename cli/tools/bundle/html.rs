// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::HashMap;
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
  for item in attrs.iter().take(attrs.len() - 1) {
    item.write_out(out);
    out.append(' ');
  }

  attrs.last().unwrap().write_out(out);
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
  pub canonical_path: PathBuf,
  pub scripts: Vec<Script>,
  pub temp_module: String,
  pub contents: String,
  pub entry_name: String,

  pub virtual_module_path: PathBuf,
}

const VIRTUAL_ENTRY_SUFFIX: &str = ".deno-bundle-html.entry";

// Helper to create a filesystem-friendly name based on a path
fn sanitize_entry_name(cwd: &Path, path: &Path) -> String {
  let rel =
    pathdiff::diff_paths(path, cwd).unwrap_or_else(|| path.to_path_buf());
  let stem = rel
    .with_extension("")
    .to_string_lossy()
    .replace(['\\', '/', ':'], "_");
  if stem.is_empty() {
    "entry".to_string()
  } else {
    stem
  }
}

fn parse_html_entrypoint(
  cwd: &Path,
  path: &Path,
  canonical_path: PathBuf,
  contents: String,
) -> anyhow::Result<HtmlEntrypoint> {
  let mut scripts = collect_scripts(&contents)?;

  let mut temp_module = String::new();
  for script in &mut scripts {
    if let Some(src) = &mut script.src {
      let src = src.trim_start_matches('/');
      let path = path.parent().unwrap_or(Path::new("")).join(src);

      let url = deno_path_util::url_from_file_path(&path)?;

      temp_module.push_str(&format!("import \"{}\";\n", url));
      script.resolved_path = Some(path);
    }
  }

  let entry_name = sanitize_entry_name(cwd, path);
  let virtual_module_path = path
    .parent()
    .unwrap_or(Path::new(""))
    .join(format!("{}{}.js", entry_name, VIRTUAL_ENTRY_SUFFIX));

  Ok(HtmlEntrypoint {
    path: path.to_path_buf(),
    canonical_path,
    scripts,
    temp_module,
    contents,
    entry_name,
    virtual_module_path,
  })
}

pub fn load_html_entrypoint(
  cwd: &Path,
  path: &Path,
) -> anyhow::Result<HtmlEntrypoint> {
  let contents = std::fs::read_to_string(path)?;
  let canonical_path = crate::util::fs::canonicalize_path(path)?;
  parse_html_entrypoint(cwd, path, canonical_path, contents)
}

#[derive(Debug, Clone)]
pub struct ParsedOutput {
  path: PathBuf,
  index: usize,
  hash: String,
}

#[derive(Debug)]
pub struct HtmlOutputFiles<'a, 'f> {
  output_files: &'f mut Vec<OutputFile<'a>>,
  index: HashMap<String, ParsedOutput>,
}

impl<'a, 'f> HtmlOutputFiles<'a, 'f> {
  pub fn new(output_files: &'f mut Vec<OutputFile<'a>>) -> Self {
    let re =
      lazy_regex::regex!(r"(^.+\.deno-bundle-html.entry)-([^.]+)(\..+)$");
    let mut index = std::collections::HashMap::new();
    for (i, f) in output_files.iter().enumerate() {
      if let Some(name) = f.path.file_name().map(|s| s.to_string_lossy()) {
        let Some(captures) = re.captures(&name) else {
          continue;
        };
        let mut entry_name = captures.get(1).unwrap().as_str().to_string();
        let ext = captures.get(3).unwrap().as_str();
        entry_name.push_str(ext);

        index.insert(
          entry_name,
          ParsedOutput {
            path: f.path.clone(),
            index: i,
            hash: captures.get(2).unwrap().as_str().to_string(),
          },
        );
      }
    }
    Self {
      output_files,
      index,
    }
  }

  pub fn get_and_update_path(
    &mut self,
    name: &str,
    f: impl FnOnce(PathBuf, &ParsedOutput) -> PathBuf,
  ) -> Option<PathBuf> {
    let parsed_output = self.index.get_mut(name)?;
    let new_path = f(parsed_output.path.clone(), parsed_output);
    parsed_output.path = new_path.clone();
    self.output_files[parsed_output.index].path = new_path.clone();
    Some(new_path)
  }
}

impl HtmlEntrypoint {
  fn original_entry_name(&self) -> String {
    self.path.file_stem().unwrap().to_string_lossy().to_string()
  }
  pub fn patch_html_with_response<'a>(
    self,
    _cwd: &Path,
    outdir: &Path,
    html_output_files: &mut HtmlOutputFiles<'a, '_>,
  ) -> anyhow::Result<()> {
    let original_entry_name = self.original_entry_name();

    if self.scripts.is_empty() {
      let html_out_path =
        // TODO(nathanwhit): not really correct
        { outdir.join(format!("{}.html", &original_entry_name)) };
      html_output_files.output_files.push(OutputFile {
        path: html_out_path,
        contents: Cow::Owned(self.contents.into_bytes()),
        hash: None,
      });
      return Ok(());
    }

    let entry_name = format!("{}{}", self.entry_name, VIRTUAL_ENTRY_SUFFIX);
    let js_entry_name = format!("{}.js", entry_name);

    let mut js_out_no_hash = None;
    let js_out = html_output_files
      .get_and_update_path(&js_entry_name, |p, f| {
        let p = p.to_string_lossy();
        js_out_no_hash = Some(
          p.replace(entry_name.as_str(), &original_entry_name)
            .replace(&format!("-{}", f.hash), "")
            .into(),
        );

        p.replace(entry_name.as_str(), &original_entry_name).into()
      })
      .ok_or_else(|| {
        anyhow::anyhow!(
          "failed to locate output for HTML entry '{}'; {js_entry_name}",
          self.entry_name
        )
      })?;
    let html_out_path = js_out_no_hash
      .unwrap_or_else(|| js_out.clone())
      .with_extension("html");

    let css_entry_name = format!("{}.css", entry_name);
    let css_out =
      html_output_files.get_and_update_path(&css_entry_name, |p, _| {
        p.to_string_lossy()
          .replace(entry_name.as_str(), &original_entry_name)
          .into()
      });

    let script_src = {
      let base = html_out_path.parent().unwrap_or(outdir);
      let mut rel = pathdiff::diff_paths(&js_out, base)
        .unwrap_or_else(|| js_out.clone())
        .to_string_lossy()
        .into_owned();
      if std::path::MAIN_SEPARATOR != '/' {
        rel = rel.replace('\\', "/");
      }
      rel
    };
    let any_async = self.scripts.iter().any(|s| s.is_async);
    let any_module = self.scripts.iter().any(|s| s.is_module);

    let to_inject = Script {
      src: Some(
        if !script_src.starts_with(".") && !script_src.starts_with("/") {
          format!("./{}", script_src)
        } else {
          script_src
        },
      ),
      is_async: any_async,
      is_module: any_module,
      resolved_path: None,
    };

    let css_href = css_out.as_ref().map(|p| {
      let base = html_out_path.parent().unwrap_or(outdir);
      let mut rel = pathdiff::diff_paths(p, base)
        .unwrap_or_else(|| p.clone())
        .to_string_lossy()
        .into_owned();
      if std::path::MAIN_SEPARATOR != '/' {
        rel = rel.replace('\\', "/");
      }
      if !rel.starts_with(".") && !rel.starts_with("/") {
        rel = format!("./{}", rel);
      }
      rel
    });

    let patched = inject_scripts_and_css(
      &self.contents,
      to_inject,
      &self.scripts,
      css_href,
    )?;

    html_output_files.output_files.push(OutputFile {
      path: html_out_path,
      contents: Cow::Owned(patched.into_bytes()),
      hash: None,
    });

    Ok(())
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
