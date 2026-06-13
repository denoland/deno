// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal [EditorConfig](https://editorconfig.org/) loader used by `deno fmt`.
//!
//! Walks up the directory tree from a given file, parses encountered
//! `.editorconfig` files, and resolves matching properties for that file.
//! Results are cached so repeated lookups within a tree do not re-read
//! and re-parse the same files.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use deno_config::deno_json::FmtOptionsConfig;
use deno_config::deno_json::NewLineKind;
use regex::Regex;

use crate::util::fs::canonicalize_path;

/// Properties resolved from `.editorconfig` files for a particular file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorConfigProperties {
  pub indent_style: Option<IndentStyle>,
  pub indent_size: Option<u8>,
  pub tab_width: Option<u8>,
  pub max_line_length: Option<u32>,
  pub end_of_line: Option<EndOfLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
  Space,
  Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndOfLine {
  Lf,
  Crlf,
  Cr,
}

impl EditorConfigProperties {
  /// Apply the resolved properties to `cfg`, filling in only fields
  /// that are currently `None`. Deno's own config and CLI flags
  /// therefore always take precedence.
  pub fn apply_to(&self, cfg: &mut FmtOptionsConfig) {
    if cfg.use_tabs.is_none()
      && let Some(style) = self.indent_style
    {
      cfg.use_tabs = Some(matches!(style, IndentStyle::Tab));
    }

    if cfg.indent_width.is_none() {
      // Per the editorconfig spec, when indent_style is "tab" and
      // indent_size is not set, indent_size defaults to tab_width.
      // For "space" or unset indent_style, indent_size is taken as-is.
      let indent = self.indent_size.or(
        if matches!(self.indent_style, Some(IndentStyle::Tab)) {
          self.tab_width
        } else {
          None
        },
      );
      if let Some(n) = indent {
        cfg.indent_width = Some(n);
      }
    }

    if cfg.line_width.is_none()
      && let Some(n) = self.max_line_length
    {
      cfg.line_width = Some(n);
    }

    if cfg.new_line_kind.is_none() {
      cfg.new_line_kind = match self.end_of_line {
        Some(EndOfLine::Lf) => Some(NewLineKind::LineFeed),
        Some(EndOfLine::Crlf) => Some(NewLineKind::CarriageReturnLineFeed),
        // No mapping for CR-only; leave unset.
        Some(EndOfLine::Cr) | None => None,
      };
    }
  }

  pub fn is_empty(&self) -> bool {
    self.indent_style.is_none()
      && self.indent_size.is_none()
      && self.tab_width.is_none()
      && self.max_line_length.is_none()
      && self.end_of_line.is_none()
  }
}

#[derive(Debug)]
struct ParsedFile {
  root: bool,
  sections: Vec<Section>,
}

#[derive(Debug)]
struct Section {
  /// Anchored regex compiled from the section's glob pattern,
  /// matched against the slash-separated path relative to the
  /// `.editorconfig` directory. `None` if the pattern was empty or
  /// failed to compile (in which case the section is inert). Patterns
  /// without a `/` match the basename in any subdirectory; this is
  /// encoded by a `(?:.*/)?` prefix in the compiled regex.
  regex: Option<Regex>,
  properties: SectionProperties,
}

#[derive(Debug, Clone, Default)]
struct SectionProperties {
  indent_style: Option<IndentStyle>,
  indent_size: Option<u8>,
  tab_width: Option<u8>,
  max_line_length: Option<u32>,
  end_of_line: Option<EndOfLine>,
}

/// One entry in a resolved `.editorconfig` chain — a parsed file and
/// the directory it lives in (needed to compute the path relative to
/// the `.editorconfig` for pattern matching).
#[derive(Debug)]
struct ChainEntry {
  dir: PathBuf,
  file: Arc<ParsedFile>,
}

/// Cache of parsed `.editorconfig` files, keyed by the absolute path
/// of the directory the file lives in. The cache also memoizes the
/// resolved chain (outermost → innermost) for each starting directory
/// so that the directory walk runs once per unique directory rather
/// than once per file.
#[derive(Debug, Default)]
pub struct EditorConfigCache {
  files: Mutex<HashMap<PathBuf, Option<Arc<ParsedFile>>>>,
  chains: Mutex<HashMap<PathBuf, Arc<Vec<ChainEntry>>>>,
}

impl EditorConfigCache {
  pub fn new() -> Self {
    Self::default()
  }

  /// Resolve `.editorconfig` properties for `file_path`. Returns
  /// `Default::default()` if no `.editorconfig` files apply.
  pub fn resolve(&self, file_path: &Path) -> EditorConfigProperties {
    let abs_path = match canonicalize_path(file_path) {
      Ok(p) => p,
      Err(_) => file_path.to_path_buf(),
    };
    let start = abs_path.parent().unwrap_or(&abs_path).to_path_buf();
    let chain = self.resolve_chain(&start);
    if chain.is_empty() {
      return EditorConfigProperties::default();
    }

    let mut out = EditorConfigProperties::default();
    for entry in chain.iter() {
      // Compute the file path relative to this .editorconfig's dir.
      let Ok(rel) = abs_path.strip_prefix(&entry.dir) else {
        continue;
      };
      let rel = path_to_forward_slash(rel);
      for section in &entry.file.sections {
        if let Some(re) = &section.regex
          && re.is_match(&rel)
        {
          merge_section(&mut out, &section.properties);
        }
      }
    }
    out
  }

  /// Resolve the (outermost → innermost) chain of `.editorconfig`
  /// files that apply to anything in `dir`. Result is cached per dir,
  /// so files in the same directory share the same walk.
  fn resolve_chain(&self, dir: &Path) -> Arc<Vec<ChainEntry>> {
    if let Some(c) = self.chains.lock().unwrap().get(dir) {
      return c.clone();
    }
    let mut entries: Vec<ChainEntry> = Vec::new();
    let mut cur: Option<&Path> = Some(dir);
    while let Some(d) = cur {
      let ec_path = d.join(".editorconfig");
      if let Some(parsed) = self.read_and_parse(&ec_path) {
        let is_root = parsed.root;
        entries.push(ChainEntry {
          dir: d.to_path_buf(),
          file: parsed,
        });
        if is_root {
          break;
        }
      }
      cur = d.parent();
    }
    // Apply outermost first so nearer files override farther ones.
    entries.reverse();
    let arc = Arc::new(entries);
    self
      .chains
      .lock()
      .unwrap()
      .insert(dir.to_path_buf(), arc.clone());
    arc
  }

  fn read_and_parse(&self, path: &Path) -> Option<Arc<ParsedFile>> {
    {
      let files = self.files.lock().unwrap();
      if let Some(cached) = files.get(path) {
        return cached.clone();
      }
    }
    let parsed = std::fs::read_to_string(path)
      .ok()
      .map(|s| Arc::new(parse(&s)));
    if parsed.is_some() {
      log::debug!("Found .editorconfig at {} and using it", path.display());
    }
    let mut files = self.files.lock().unwrap();
    files.insert(path.to_path_buf(), parsed.clone());
    parsed
  }
}

fn merge_section(dst: &mut EditorConfigProperties, src: &SectionProperties) {
  if let Some(v) = src.indent_style {
    dst.indent_style = Some(v);
  }
  if let Some(v) = src.indent_size {
    dst.indent_size = Some(v);
  }
  if let Some(v) = src.tab_width {
    dst.tab_width = Some(v);
  }
  if let Some(v) = src.max_line_length {
    dst.max_line_length = Some(v);
  }
  if let Some(v) = src.end_of_line {
    dst.end_of_line = Some(v);
  }
}

fn parse(contents: &str) -> ParsedFile {
  let mut root = false;
  let mut sections: Vec<Section> = Vec::new();
  let mut current: Option<Section> = None;

  for raw_line in contents.lines() {
    let line = strip_comment(raw_line).trim();
    if line.is_empty() {
      continue;
    }
    if let Some(rest) = line.strip_prefix('[')
      && let Some(pattern) = rest.strip_suffix(']')
    {
      if let Some(prev) = current.take() {
        sections.push(prev);
      }
      let regex = compile_glob_regex(pattern);
      current = Some(Section {
        regex,
        properties: SectionProperties::default(),
      });
      continue;
    }

    let Some((key, value)) = line.split_once('=') else {
      continue;
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();

    if current.is_none() {
      // Preamble: only "root" is meaningful.
      if key == "root" && value.eq_ignore_ascii_case("true") {
        root = true;
      }
      continue;
    }
    let props = &mut current.as_mut().unwrap().properties;
    match key.as_str() {
      "indent_style" => {
        props.indent_style = match value.to_ascii_lowercase().as_str() {
          "tab" => Some(IndentStyle::Tab),
          "space" => Some(IndentStyle::Space),
          _ => None,
        };
      }
      "indent_size" => {
        // "tab" means use tab_width; otherwise parse as integer,
        // clamping out-of-range values rather than dropping them.
        if !value.eq_ignore_ascii_case("tab")
          && let Some(n) = parse_saturating_u8(value)
        {
          props.indent_size = Some(n);
        }
      }
      "tab_width" => {
        if let Some(n) = parse_saturating_u8(value) {
          props.tab_width = Some(n);
        }
      }
      "max_line_length" => {
        if !value.eq_ignore_ascii_case("off")
          && let Some(n) = parse_saturating_u32(value)
        {
          props.max_line_length = Some(n);
        }
      }
      "end_of_line" => {
        props.end_of_line = match value.to_ascii_lowercase().as_str() {
          "lf" => Some(EndOfLine::Lf),
          "crlf" => Some(EndOfLine::Crlf),
          "cr" => Some(EndOfLine::Cr),
          _ => None,
        };
      }
      _ => {}
    }
  }
  if let Some(prev) = current.take() {
    sections.push(prev);
  }

  ParsedFile { root, sections }
}

/// Parse a non-negative integer editorconfig value, saturating to the
/// target type's maximum on overflow instead of discarding the value.
/// Returns `None` for negative or non-numeric input so the property is
/// simply ignored.
fn parse_saturating_u8(value: &str) -> Option<u8> {
  let n = value.trim().parse::<u64>().ok()?;
  Some(n.min(u8::MAX as u64) as u8)
}

fn parse_saturating_u32(value: &str) -> Option<u32> {
  let n = value.trim().parse::<u64>().ok()?;
  Some(n.min(u32::MAX as u64) as u32)
}

fn strip_comment(s: &str) -> &str {
  // EditorConfig allows ';' or '#' as comment markers. They start
  // a comment if at the beginning of a line or preceded by whitespace.
  let mut prev_ws = true;
  for (i, ch) in s.char_indices() {
    if (ch == ';' || ch == '#') && prev_ws {
      return &s[..i];
    }
    prev_ws = ch.is_whitespace();
  }
  s
}

/// Compile a `.editorconfig` section glob pattern to a regex anchored
/// against the slash-separated relative path of a file. Returns `None`
/// if the pattern is empty or fails to compile (the section then has
/// no effect).
fn compile_glob_regex(pattern: &str) -> Option<Regex> {
  if pattern.is_empty() {
    return None;
  }
  // If the pattern doesn't contain a path separator it matches the
  // basename in any subdirectory; otherwise it's anchored at the
  // `.editorconfig` directory.
  let match_any_dir = !pattern.contains('/');
  let pattern_re = glob_to_regex(pattern, match_any_dir);
  Regex::new(&pattern_re).ok()
}

fn path_to_forward_slash(p: &Path) -> String {
  let s = p.to_string_lossy().into_owned();
  if std::path::MAIN_SEPARATOR == '/' {
    s
  } else {
    s.replace(std::path::MAIN_SEPARATOR, "/")
  }
}

/// Convert an editorconfig glob pattern to a regex string anchored
/// with `^` and `$`. If `match_any_dir`, the pattern is allowed to
/// be preceded by any number of leading directory components.
fn glob_to_regex(pattern: &str, match_any_dir: bool) -> String {
  glob_to_regex_depth(pattern, match_any_dir, 0)
}

/// Maximum brace-nesting depth expanded before a pattern degrades to a
/// literal match. Guards against a stack overflow on a pathological
/// pattern such as `{a,{a,{a,...}}}` nested thousands deep.
const MAX_GLOB_DEPTH: u32 = 32;

fn glob_to_regex_depth(
  pattern: &str,
  match_any_dir: bool,
  depth: u32,
) -> String {
  let mut out = String::from("^");
  if match_any_dir {
    out.push_str("(?:.*/)?");
  }
  let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
  let bytes: Vec<char> = pattern.chars().collect();
  let mut i = 0;
  while i < bytes.len() {
    let c = bytes[i];
    match c {
      '*' => {
        if i + 1 < bytes.len() && bytes[i + 1] == '*' {
          // Treat `**/` as zero or more path components so that
          // `**/foo.ts` also matches `foo.ts` at the root, matching
          // gitignore-style user expectations.
          if i + 2 < bytes.len() && bytes[i + 2] == '/' {
            out.push_str("(?:[^/]*/)*");
            i += 3;
            continue;
          }
          out.push_str(".*");
          i += 2;
          continue;
        } else {
          out.push_str("[^/]*");
        }
      }
      '?' => out.push_str("[^/]"),
      '{' => {
        // Find matching '}'.
        let mut brace_depth = 1;
        let mut j = i + 1;
        while j < bytes.len() && brace_depth > 0 {
          match bytes[j] {
            '{' => brace_depth += 1,
            '}' => {
              brace_depth -= 1;
              if brace_depth == 0 {
                break;
              }
            }
            _ => {}
          }
          j += 1;
        }
        if brace_depth == 0 {
          let group: String = bytes[i + 1..j].iter().collect();
          // Numeric range {n..m}. Bounds are parsed as integers, so
          // leading zeros are ignored and numbers are emitted in their
          // natural decimal form, matching the editorconfig reference.
          if let Some((lhs, rhs)) = group.split_once("..")
            && let (Ok(lo), Ok(hi)) =
              (lhs.trim().parse::<i64>(), rhs.trim().parse::<i64>())
          {
            let (a, b) = if lo <= hi { (lo, hi) } else { (hi, lo) };
            // Bound the enumeration so a pathological range such as
            // `{1..1000000000}` cannot exhaust memory while building the
            // regex. Real editorconfig ranges are tiny; a larger span
            // degrades to a literal match (the section simply will not
            // apply) rather than hanging or crashing.
            const MAX_RANGE_SPAN: i64 = 4096;
            let span_ok =
              b.checked_sub(a).is_some_and(|span| span < MAX_RANGE_SPAN);
            if span_ok {
              out.push('(');
              for n in a..=b {
                if n != a {
                  out.push('|');
                }
                for ch in n.to_string().chars() {
                  regex_push_escaped(&mut out, ch);
                }
              }
              out.push(')');
              i = j + 1;
              continue;
            }
            // Span too large: fall through to literal handling below.
          }
          // Comma alternatives
          let alts = split_top_level_commas(&group);
          if alts.len() > 1 && depth < MAX_GLOB_DEPTH {
            out.push_str("(?:");
            for (k, alt) in alts.iter().enumerate() {
              if k > 0 {
                out.push('|');
              }
              out.push_str(&glob_inner_to_regex(alt, depth + 1));
            }
            out.push(')');
            i = j + 1;
            continue;
          }
          // Single literal — fall through to literal
          for ch in group.chars() {
            regex_push_escaped(&mut out, ch);
          }
          i = j + 1;
          continue;
        } else {
          // Unmatched brace - treat literally.
          regex_push_escaped(&mut out, '{');
        }
      }
      '[' => {
        // Character class; pass through, but translate `[!...]` -> `[^...]`.
        let mut j = i + 1;
        let mut negate = false;
        if j < bytes.len() && (bytes[j] == '!' || bytes[j] == '^') {
          negate = true;
          j += 1;
        }
        let mut chars = Vec::new();
        while j < bytes.len() && bytes[j] != ']' {
          chars.push(bytes[j]);
          j += 1;
        }
        if j < bytes.len() {
          out.push('[');
          if negate {
            out.push('^');
          }
          for ch in chars {
            // Inside a char class, escape `\`, `]`, `^`.
            match ch {
              '\\' | ']' => {
                out.push('\\');
                out.push(ch);
              }
              _ => out.push(ch),
            }
          }
          out.push(']');
          i = j + 1;
          continue;
        } else {
          regex_push_escaped(&mut out, '[');
        }
      }
      _ => regex_push_escaped(&mut out, c),
    }
    i += 1;
  }
  out.push('$');
  out
}

fn glob_inner_to_regex(s: &str, depth: u32) -> String {
  // Recursive use for alternatives - reuse the same logic without
  // adding `^`/`$` anchors or the leading any-dir prefix.
  let inner = glob_to_regex_depth(s, false, depth);
  // Strip the surrounding ^...$.
  inner
    .strip_prefix('^')
    .and_then(|t| t.strip_suffix('$'))
    .unwrap_or(&inner)
    .to_string()
}

fn split_top_level_commas(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut current = String::new();
  let mut depth = 0i32;
  for ch in s.chars() {
    match ch {
      '{' => {
        depth += 1;
        current.push(ch);
      }
      '}' => {
        depth -= 1;
        current.push(ch);
      }
      ',' if depth == 0 => {
        out.push(std::mem::take(&mut current));
      }
      _ => current.push(ch),
    }
  }
  if !current.is_empty() || !out.is_empty() {
    out.push(current);
  }
  out
}

fn regex_push_escaped(out: &mut String, ch: char) {
  match ch {
    '.' | '+' | '(' | ')' | '|' | '^' | '$' | '\\' | '[' | ']' | '{' | '}' => {
      out.push('\\');
      out.push(ch);
    }
    _ => out.push(ch),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_str(s: &str) -> ParsedFile {
    parse(s)
  }

  fn match_regex(pattern: &str, text: &str) -> bool {
    Regex::new(pattern)
      .map(|re| re.is_match(text))
      .unwrap_or_else(|err| panic!("invalid regex {pattern:?}: {err}"))
  }

  #[test]
  fn parses_root_and_sections() {
    let f = parse_str(
      r"root = true

[*]
indent_style = space
indent_size = 2

[*.py]
indent_size = 4
",
    );
    assert!(f.root);
    assert_eq!(f.sections.len(), 2);
    let s0_re = f.sections[0].regex.as_ref().unwrap();
    assert!(s0_re.is_match("foo.ts"));
    assert!(s0_re.is_match("a/b/foo.ts"));
    assert_eq!(
      f.sections[0].properties.indent_style,
      Some(IndentStyle::Space)
    );
    assert_eq!(f.sections[0].properties.indent_size, Some(2));
    let s1_re = f.sections[1].regex.as_ref().unwrap();
    assert!(s1_re.is_match("a.py"));
    assert!(!s1_re.is_match("a.ts"));
    assert_eq!(f.sections[1].properties.indent_size, Some(4));
  }

  #[test]
  fn parses_tab_and_max_line_length() {
    let f = parse_str(
      r"[*]
indent_style = tab
tab_width = 4
max_line_length = 100
end_of_line = crlf
",
    );
    let p = &f.sections[0].properties;
    assert_eq!(p.indent_style, Some(IndentStyle::Tab));
    assert_eq!(p.tab_width, Some(4));
    assert_eq!(p.max_line_length, Some(100));
    assert_eq!(p.end_of_line, Some(EndOfLine::Crlf));
  }

  #[test]
  fn strips_comments() {
    let f = parse_str(
      r"; preamble comment
root = true # ignored

[*] # section
indent_size = 2 ; inline
",
    );
    assert!(f.root);
    assert_eq!(f.sections[0].properties.indent_size, Some(2));
  }

  #[test]
  fn max_line_length_off() {
    let f = parse_str(
      r"[*]
max_line_length = off
",
    );
    assert_eq!(f.sections[0].properties.max_line_length, None);
  }

  #[test]
  fn glob_basic_extension() {
    let r = glob_to_regex("*.ts", true);
    assert!(match_regex(&r, "foo.ts"));
    assert!(match_regex(&r, "sub/foo.ts"));
    assert!(!match_regex(&r, "foo.js"));
  }

  #[test]
  fn glob_braces() {
    let r = glob_to_regex("*.{ts,tsx,js}", true);
    assert!(match_regex(&r, "foo.ts"));
    assert!(match_regex(&r, "foo.tsx"));
    assert!(match_regex(&r, "foo.js"));
    assert!(!match_regex(&r, "foo.py"));
  }

  #[test]
  fn glob_double_star() {
    let r = glob_to_regex("**/foo.ts", false);
    assert!(match_regex(&r, "foo.ts"));
    assert!(match_regex(&r, "a/foo.ts"));
    assert!(match_regex(&r, "a/b/foo.ts"));
    assert!(!match_regex(&r, "a/foo.js"));
  }

  #[test]
  fn glob_single_star_no_slash() {
    let r = glob_to_regex("foo/*.ts", false);
    assert!(match_regex(&r, "foo/bar.ts"));
    assert!(!match_regex(&r, "foo/sub/bar.ts"));
  }

  #[test]
  fn glob_with_slash_anchored() {
    // Pattern with '/' is anchored at config dir root.
    let r = glob_to_regex("src/*.ts", false);
    assert!(match_regex(&r, "src/a.ts"));
    assert!(!match_regex(&r, "lib/src/a.ts"));
  }

  #[test]
  fn glob_question_mark() {
    let r = glob_to_regex("?.ts", true);
    assert!(match_regex(&r, "a.ts"));
    assert!(!match_regex(&r, "ab.ts"));
  }

  #[test]
  fn glob_char_class() {
    let r = glob_to_regex("[abc].ts", true);
    assert!(match_regex(&r, "a.ts"));
    assert!(match_regex(&r, "b.ts"));
    assert!(!match_regex(&r, "d.ts"));
  }

  #[test]
  fn glob_negated_char_class() {
    let r = glob_to_regex("[!abc].ts", true);
    assert!(!match_regex(&r, "a.ts"));
    assert!(match_regex(&r, "d.ts"));
  }

  #[test]
  fn glob_numeric_range() {
    let r = glob_to_regex("file{1..3}.txt", true);
    assert!(match_regex(&r, "file1.txt"));
    assert!(match_regex(&r, "file2.txt"));
    assert!(match_regex(&r, "file3.txt"));
    assert!(!match_regex(&r, "file4.txt"));
  }

  #[test]
  fn glob_reversed_numeric_range() {
    let r = glob_to_regex("file{3..1}.txt", true);
    assert!(match_regex(&r, "file1.txt"));
    assert!(match_regex(&r, "file2.txt"));
    assert!(match_regex(&r, "file3.txt"));
  }

  #[test]
  fn glob_leading_zero_range_matches_reference() {
    // Bounds are parsed as integers, so leading zeros are ignored,
    // matching the editorconfig reference implementation.
    let r = glob_to_regex("file{01..03}.txt", true);
    assert!(match_regex(&r, "file1.txt"));
    assert!(match_regex(&r, "file3.txt"));
  }

  #[test]
  fn glob_huge_numeric_range_degrades() {
    // A pathological range must not be expanded into a giant regex; it
    // degrades to a literal that simply does not match normal files.
    let r = glob_to_regex("file{1..1000000000}.txt", true);
    assert!(r.len() < 100, "regex unexpectedly large: {} chars", r.len());
    assert!(!match_regex(&r, "file5.txt"));
  }

  #[test]
  fn glob_deeply_nested_braces_do_not_overflow() {
    // Nest braces well past MAX_GLOB_DEPTH; this must return without
    // overflowing the stack and still produce a valid regex.
    let mut p = String::from("x");
    for _ in 0..5000 {
      p = format!("{{a,{p}}}");
    }
    let r = glob_to_regex(&p, false);
    assert!(r.starts_with('^') && r.ends_with('$'));
    // Must compile rather than blow up.
    assert!(Regex::new(&r).is_ok());
  }

  #[test]
  fn glob_unbalanced_brace_compiles() {
    // An unbalanced brace must still translate to a valid regex.
    let r = glob_to_regex("foo{bar.ts", true);
    assert!(match_regex(&r, "foo{bar.ts"));
  }

  #[test]
  fn parse_saturating_clamps() {
    assert_eq!(parse_saturating_u8("8"), Some(8));
    assert_eq!(parse_saturating_u8("256"), Some(255));
    assert_eq!(parse_saturating_u8("-1"), None);
    assert_eq!(parse_saturating_u8("nope"), None);
    assert_eq!(parse_saturating_u32("99999999999"), Some(u32::MAX));
  }

  #[test]
  fn indent_size_overflow_clamped() {
    let f = parse_str("[*]\nindent_size = 1000\n");
    assert_eq!(f.sections[0].properties.indent_size, Some(255));
  }

  #[test]
  fn max_line_length_overflow_clamped() {
    let f = parse_str("[*]\nmax_line_length = 99999999999\n");
    assert_eq!(f.sections[0].properties.max_line_length, Some(u32::MAX));
  }

  #[test]
  fn apply_indent_tab_with_width() {
    let mut cfg = FmtOptionsConfig::default();
    let props = EditorConfigProperties {
      indent_style: Some(IndentStyle::Tab),
      tab_width: Some(4),
      ..Default::default()
    };
    props.apply_to(&mut cfg);
    assert_eq!(cfg.use_tabs, Some(true));
    assert_eq!(cfg.indent_width, Some(4));
  }

  #[test]
  fn apply_does_not_override_existing() {
    let mut cfg = FmtOptionsConfig {
      use_tabs: Some(false),
      indent_width: Some(2),
      ..Default::default()
    };
    let props = EditorConfigProperties {
      indent_style: Some(IndentStyle::Tab),
      indent_size: Some(8),
      ..Default::default()
    };
    props.apply_to(&mut cfg);
    assert_eq!(cfg.use_tabs, Some(false));
    assert_eq!(cfg.indent_width, Some(2));
  }

  #[test]
  fn apply_end_of_line_maps_to_new_line_kind() {
    let mut cfg = FmtOptionsConfig::default();
    let props = EditorConfigProperties {
      end_of_line: Some(EndOfLine::Crlf),
      ..Default::default()
    };
    props.apply_to(&mut cfg);
    assert_eq!(cfg.new_line_kind, Some(NewLineKind::CarriageReturnLineFeed));
  }
}
