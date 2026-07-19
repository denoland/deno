// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use capacity_builder::StringBuilder;
use deno_core::anyhow;
use deno_core::error::AnyError;

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

#[derive(Debug, Clone, Copy)]
struct HtmlTag<'a> {
  start: usize,
  end: usize,
  name: &'a str,
  attrs: &'a str,
  is_end: bool,
}

fn is_ascii_whitespace(byte: u8) -> bool {
  matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn is_tag_name_delimiter(byte: u8) -> bool {
  is_ascii_whitespace(byte) || matches!(byte, b'/' | b'>')
}

fn is_ascii_alpha(byte: u8) -> bool {
  byte.is_ascii_alphabetic()
}

fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
  haystack
    .as_bytes()
    .get(..needle.len())
    .is_some_and(|value| value.eq_ignore_ascii_case(needle.as_bytes()))
}

fn find_tag_end(input: &str, start: usize) -> (usize, bool) {
  let bytes = input.as_bytes();
  let mut i = start;
  let mut quote = None;
  while i < bytes.len() {
    let byte = bytes[i];
    if let Some(quote_byte) = quote {
      if byte == quote_byte {
        quote = None;
      }
    } else if matches!(byte, b'\'' | b'"') {
      quote = Some(byte);
    } else if byte == b'>' {
      return (i + 1, true);
    }
    i += 1;
  }
  (input.len(), false)
}

fn find_next_tag(input: &str, mut pos: usize) -> Option<HtmlTag<'_>> {
  let bytes = input.as_bytes();
  while pos < bytes.len() {
    let relative_start = input[pos..].find('<')?;
    let start = pos + relative_start;
    let after_lt = start + 1;
    if after_lt >= bytes.len() {
      return None;
    }

    if starts_with_ignore_ascii_case(&input[start..], "<!--") {
      pos = input[start + 4..]
        .find("-->")
        .map(|end| start + 4 + end + 3)
        .unwrap_or(bytes.len());
      continue;
    }

    if matches!(bytes[after_lt], b'!' | b'?') {
      pos = find_tag_end(input, after_lt + 1).0;
      continue;
    }

    let is_end = bytes[after_lt] == b'/';
    let name_start = if is_end { after_lt + 1 } else { after_lt };
    if name_start >= bytes.len() || !is_ascii_alpha(bytes[name_start]) {
      pos = after_lt;
      continue;
    }

    let mut name_end = name_start + 1;
    while name_end < bytes.len() && !is_tag_name_delimiter(bytes[name_end]) {
      name_end += 1;
    }

    let (end, found_end) = find_tag_end(input, name_end);
    let attrs_end = if found_end {
      end.saturating_sub(1)
    } else {
      end
    };
    return Some(HtmlTag {
      start,
      end,
      name: &input[name_start..name_end],
      attrs: &input[name_end..attrs_end],
      is_end,
    });
  }
  None
}

fn attr_value<'a>(attrs: &'a str, name: &str) -> Option<Option<&'a str>> {
  let bytes = attrs.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    while i < bytes.len() && (is_ascii_whitespace(bytes[i]) || bytes[i] == b'/')
    {
      i += 1;
    }
    if i >= bytes.len() {
      return None;
    }

    let name_start = i;
    while i < bytes.len()
      && !is_ascii_whitespace(bytes[i])
      && !matches!(bytes[i], b'=' | b'/' | b'>')
    {
      i += 1;
    }
    if name_start == i {
      i += 1;
      continue;
    }
    let attr_name = &attrs[name_start..i];

    while i < bytes.len() && is_ascii_whitespace(bytes[i]) {
      i += 1;
    }

    let mut value = None;
    if i < bytes.len() && bytes[i] == b'=' {
      i += 1;
      while i < bytes.len() && is_ascii_whitespace(bytes[i]) {
        i += 1;
      }
      if i < bytes.len() && matches!(bytes[i], b'\'' | b'"') {
        let quote = bytes[i];
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
          i += 1;
        }
        value = Some(&attrs[value_start..i]);
        if i < bytes.len() {
          i += 1;
        }
      } else {
        let value_start = i;
        while i < bytes.len()
          && !is_ascii_whitespace(bytes[i])
          && bytes[i] != b'>'
        {
          i += 1;
        }
        value = Some(&attrs[value_start..i]);
      }
    }

    if attr_name.eq_ignore_ascii_case(name) {
      return Some(value);
    }
  }

  None
}

fn has_attr(attrs: &str, name: &str) -> bool {
  attr_value(attrs, name).is_some()
}

fn find_script_end(input: &str, from: usize) -> usize {
  let bytes = input.as_bytes();
  let mut pos = from;
  while pos < bytes.len() {
    let Some(relative_start) = input[pos..].find('<') else {
      return input.len();
    };
    let start = pos + relative_start;
    if starts_with_ignore_ascii_case(&input[start..], "</script") {
      let after_name = start + "</script".len();
      if after_name >= bytes.len() || is_tag_name_delimiter(bytes[after_name]) {
        return find_tag_end(input, after_name).0;
      }
    }
    pos = start + 1;
  }
  input.len()
}

fn collect_scripts(doc: &str) -> Result<Vec<Script>, AnyError> {
  let mut scripts = Vec::new();
  let mut pos = 0;
  while let Some(tag) = find_next_tag(doc, pos) {
    if !tag.is_end && tag.name.eq_ignore_ascii_case("script") {
      if let Some(src) = attr_value(tag.attrs, "src").flatten()
        && !has_attr(tag.attrs, "deno-ignore")
        && !has_attr(tag.attrs, "vite-ignore")
      {
        let typ = attr_value(tag.attrs, "type").flatten();
        if matches!(typ, Some("module") | None) {
          scripts.push(Script {
            src: Some(src.to_string()),
            is_async: has_attr(tag.attrs, "async"),
            is_module: matches!(typ, Some("module")),
            resolved_path: None,
          });
        }
      }
      pos = find_script_end(doc, tag.end);
    } else {
      pos = tag.end;
    }
  }
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

  /// Rewrite the `sourceMappingURL` reference in the JS output identified by
  /// `name`, replacing the (renamed) sourcemap's old file name with its new
  /// one. Used after a linked sourcemap is renamed alongside its JS bundle.
  fn update_js_sourcemap_ref(&mut self, name: &str, from: &str, to: &str) {
    let Some(parsed_output) = self.index.get(name) else {
      return;
    };
    let file = &mut self.output_files[parsed_output.index];
    let Ok(contents) = std::str::from_utf8(&file.contents) else {
      return;
    };
    if !contents.contains(from) {
      return;
    }
    file.contents = Cow::Owned(contents.replace(from, to).into_bytes());
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

    // esbuild emits the external/linked sourcemap (`*.js.map`) for the entry
    // under the internal virtual-entry name. Rename it to match the renamed JS
    // bundle (e.g. `index-HASH.js.map`) and fix the JS `sourceMappingURL`
    // reference so the emitted output doesn't leak the `deno-bundle-html.entry`
    // implementation detail. See denoland/deno#30750.
    let map_entry_name = format!("{}.js.map", entry_name);
    let mut old_map_name = None;
    let new_map_out =
      html_output_files.get_and_update_path(&map_entry_name, |p, _| {
        old_map_name = p
          .file_name()
          .map(|name| name.to_string_lossy().into_owned());
        p.to_string_lossy()
          .replace(entry_name.as_str(), &original_entry_name)
          .into()
      });
    if let Some(old_map_name) = old_map_name
      && let Some(new_map_name) = new_map_out
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|name| name.to_string_lossy().into_owned())
    {
      html_output_files.update_js_sourcemap_ref(
        &js_entry_name,
        &old_map_name,
        &new_map_name,
      );
    }

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
  let script = to_inject.to_element_string();
  let link = css_to_inject_path
    .as_ref()
    .map(|p| stylesheet_str(p))
    .unwrap_or_default();
  let to_insert = format!("{script}{link}");

  let mut removals = Vec::new();
  let mut first_head_insert_after_open = None;
  let mut insert_at = None;
  let mut awaiting_first_head_close = false;
  let mut pos = 0;

  while let Some(tag) = find_next_tag(input, pos) {
    if tag.is_end {
      if awaiting_first_head_close && tag.name.eq_ignore_ascii_case("head") {
        insert_at = Some(tag.start);
        awaiting_first_head_close = false;
      }
      pos = tag.end;
      continue;
    }

    if first_head_insert_after_open.is_none()
      && tag.name.eq_ignore_ascii_case("head")
    {
      first_head_insert_after_open = Some(tag.end);
      awaiting_first_head_close = true;
      pos = tag.end;
      continue;
    }

    if tag.name.eq_ignore_ascii_case("script") {
      let script_end = find_script_end(input, tag.end);
      if let Some(src) = attr_value(tag.attrs, "src").flatten()
        && to_remove
          .iter()
          .any(|script| script.src.as_deref() == Some(src))
      {
        removals.push((tag.start, script_end));
      }
      pos = script_end;
    } else {
      pos = tag.end;
    }
  }

  let insertion =
    if let Some(insert_at) = insert_at.or(first_head_insert_after_open) {
      (insert_at, insert_at, to_insert)
    } else {
      let wrapped = format!("<head>{to_insert}</head>");
      (input.len(), input.len(), wrapped)
    };

  let mut edits = removals
    .into_iter()
    .map(|(start, end)| (start, end, String::new()))
    .collect::<Vec<_>>();
  edits.push(insertion);
  edits.sort_by_key(|(start, end, _)| (*start, *end));

  let mut rewritten = String::with_capacity(
    input.len() + edits.iter().map(|(_, _, s)| s.len()).sum::<usize>(),
  );
  let mut last = 0;
  for (start, end, replacement) in edits {
    if start < last {
      continue;
    }
    rewritten.push_str(&input[last..start]);
    rewritten.push_str(&replacement);
    last = end;
  }
  rewritten.push_str(&input[last..]);

  Ok(rewritten)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn script(src: &str) -> Script {
    Script {
      src: Some(src.to_string()),
      is_async: false,
      is_module: true,
      resolved_path: None,
    }
  }

  #[test]
  fn collect_scripts_handles_html_edge_cases() {
    let scripts = collect_scripts(
      r#"
      <!-- <script src="./comment.ts"></script> -->
      <SCRIPT SRC='./case.ts' async></SCRIPT>
      <script type=module src=./unquoted.ts></script>
      <script type="text/javascript" src="./classic.js"></script>
      <script deno-ignore src="./ignored.ts"></script>
      <script vite-ignore src="./vite.ts"></script>
      <script>const nested = '<script src="./nested.ts">';</script>
      <script src="./plain.ts"></script>
    "#,
    )
    .unwrap();

    assert_eq!(scripts.len(), 3);
    assert_eq!(scripts[0].src.as_deref(), Some("./case.ts"));
    assert!(scripts[0].is_async);
    assert!(!scripts[0].is_module);
    assert_eq!(scripts[1].src.as_deref(), Some("./unquoted.ts"));
    assert!(scripts[1].is_module);
    assert_eq!(scripts[2].src.as_deref(), Some("./plain.ts"));
  }

  #[test]
  fn collect_scripts_handles_unterminated_opening_tag_attrs() {
    let scripts =
      collect_scripts(r#"<script src="./unterminated.ts""#).unwrap();

    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0].src.as_deref(), Some("./unterminated.ts"));
  }

  #[test]
  fn inject_scripts_and_css_handles_comments_raw_text_and_duplicate_heads() {
    let actual = inject_scripts_and_css(
      r#"<html><HEAD><title>x</title><script src='./old.ts'>const nested = '<script src="./nested.ts">';</script></HEAD><head></head><!-- <script src='./old.ts'></script> --><script src="./keep.js"></script></html>"#,
      script("./bundle.js"),
      &[script("./old.ts")],
      Some("./style.css".to_string()),
    )
    .unwrap();

    assert!(actual.contains(
      r#"<script src="./bundle.js" type="module" crossorigin></script><link rel="stylesheet" crossorigin href="./style.css"></HEAD>"#
    ));
    assert!(!actual.contains("const nested"));
    assert!(actual.contains(r#"<!-- <script src='./old.ts'></script> -->"#));
    assert!(actual.contains(r#"<script src="./keep.js"></script>"#));
    assert_eq!(actual.matches("./bundle.js").count(), 1);
  }

  #[test]
  fn inject_scripts_and_css_ignores_head_text_inside_inline_script() {
    let actual = inject_scripts_and_css(
      r#"<html><head><script>const head = "<head>";</script></head></html>"#,
      script("./bundle.js"),
      &[],
      None,
    )
    .unwrap();

    assert_eq!(
      actual,
      r#"<html><head><script>const head = "<head>";</script><script src="./bundle.js" type="module" crossorigin></script></head></html>"#,
    );
  }

  #[test]
  fn inject_scripts_and_css_appends_head_when_missing() {
    let actual = inject_scripts_and_css(
      r#"<main><!-- <head></head> --></main>"#,
      script("./bundle.js"),
      &[],
      Some("./style.css".to_string()),
    )
    .unwrap();

    assert!(actual.ends_with(
      r#"<head><script src="./bundle.js" type="module" crossorigin></script><link rel="stylesheet" crossorigin href="./style.css"></head>"#
    ));
  }

  #[test]
  fn inject_scripts_and_css_handles_unclosed_head() {
    let actual = inject_scripts_and_css(
      r#"<html><HeAd><title>x</title><body><script src="./old.ts"></script></body></html>"#,
      script("./bundle.js"),
      &[script("./old.ts")],
      None,
    )
    .unwrap();

    assert!(actual.starts_with(
      r#"<html><HeAd><script src="./bundle.js" type="module" crossorigin></script><title>x</title>"#
    ));
    assert!(!actual.contains(r#"src="./old.ts""#));
  }
}
