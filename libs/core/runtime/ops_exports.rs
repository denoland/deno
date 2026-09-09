// Copyright 2018-2026 the Deno authors. MIT license.

//! Works out which named exports the `ext:core/ops` synthetic module actually
//! has to provide.
//!
//! `ext:core/ops` used to export one name per registered op. Every export cell
//! of a synthetic module must hold a value at instantiation time, so with the
//! lazy-ops interceptor in place (`Deno.core.ops` no longer holds op functions)
//! the export list became the *only* remaining reference that forces every op
//! function into the snapshot blob.
//!
//! The fix is to export only the names something actually imports. Rather than
//! making every extension declare its imports (which drifts, silently, in the
//! direction of "somebody forgot"), the import lists are read straight out of
//! the extension sources deno_core already holds in memory at snapshot-build
//! time — see [`OpsExportFilter`].
//!
//! The scanner is deliberately **fail-safe**: any use of `ext:core/ops` it
//! cannot prove to be a plain named import (`import * as ops from ...`, a
//! dynamic `import("ext:core/ops")`, an `export * from ...`) switches the whole
//! runtime back to exporting every op. Getting the analysis wrong therefore
//! costs performance, never correctness. In the other direction — a named
//! import the scanner somehow failed to see — the failure is a `SyntaxError` at
//! module instantiation naming the missing export, never a silent `undefined`.

use std::collections::HashSet;

/// Which names `ext:core/ops` should export.
#[derive(Debug)]
pub(crate) enum OpsExportFilter {
  /// Export every registered op. Either an embedder asked for it via
  /// [`crate::RuntimeOptions::export_all_ops_from_virtual_module`], or the
  /// scanner saw something it could not analyse.
  All,
  /// Export only these names.
  Only(HashSet<String>),
}

impl OpsExportFilter {
  pub(crate) fn allows(&self, name: &str) -> bool {
    match self {
      Self::All => true,
      Self::Only(names) => names.contains(name),
    }
  }

  /// Number of names that will be exported, when known.
  #[cfg(test)]
  pub(crate) fn len(&self) -> Option<usize> {
    match self {
      Self::All => None,
      Self::Only(names) => Some(names.len()),
    }
  }
}

/// Accumulates the union of `ext:core/ops` named imports across every source
/// that will ever be instantiated against this runtime's synthetic module.
#[derive(Debug, Default)]
pub(crate) struct OpsImportScan {
  names: HashSet<String>,
  export_all: bool,
}

impl OpsImportScan {
  /// Force "export everything", e.g. because the embedder asked for it.
  pub(crate) fn force_export_all(&mut self) {
    self.export_all = true;
  }

  /// Fold one source file's `ext:core/ops` imports into the union.
  ///
  /// Sources that do not mention `ext:core/ops` at all are rejected by a
  /// substring test before any tokenization happens, so the common case costs a
  /// memchr sweep.
  pub(crate) fn add_source(&mut self, source: &str) {
    if self.export_all || !source.contains(OPS_SPECIFIER) {
      return;
    }
    match scan_source(source) {
      Some(names) => self.names.extend(names),
      None => self.export_all = true,
    }
  }

  pub(crate) fn finish(self) -> OpsExportFilter {
    if self.export_all {
      OpsExportFilter::All
    } else {
      OpsExportFilter::Only(self.names)
    }
  }
}

const OPS_SPECIFIER: &str = "ext:core/ops";

/// Returns the set of names imported from `ext:core/ops`, or `None` if the
/// source uses `ext:core/ops` in a way that needs the full export list.
fn scan_source(source: &str) -> Option<Vec<String>> {
  let tokens = tokenize(source);
  let mut names = Vec::new();
  // Every occurrence of the specifier as a *string literal in code position*
  // has to be accounted for by a recognized import form. Anything else is a
  // bail.
  let mut accounted = 0usize;
  let mut occurrences = 0usize;

  for (i, tok) in tokens.iter().enumerate() {
    if !matches!(tok, Token::Str(s) if s == OPS_SPECIFIER) {
      continue;
    }
    occurrences += 1;
    if let Some(imported) = named_import_before(&tokens, i) {
      accounted += 1;
      names.extend(imported);
    } else if matches!(
      tokens.get(i.wrapping_sub(1)),
      Some(Token::Ident(kw)) if *kw == "import"
    ) {
      // Bare side-effect import `import "ext:core/ops";` needs no exports.
      accounted += 1;
    }
  }

  if occurrences == 0 || accounted != occurrences {
    return None;
  }
  Some(names)
}

/// Given that `tokens[str_idx]` is the specifier string, walk backwards over
/// `from { a, b as c } import|export` and return the imported names.
fn named_import_before(
  tokens: &[Token],
  str_idx: usize,
) -> Option<Vec<String>> {
  // ... from
  let from = str_idx.checked_sub(1)?;
  match tokens.get(from)? {
    Token::Ident(k) if *k == "from" => {}
    _ => return None,
  }
  // ... }
  let close = from.checked_sub(1)?;
  match tokens.get(close)? {
    Token::Punct('}') => {}
    _ => return None,
  }
  // Walk back to the matching `{`. Import clauses cannot nest braces, so the
  // first `{` going backwards is the opener; anything else in between that is
  // not an identifier, `,` or `as` means this is not an import clause.
  let mut open = close;
  let mut names = Vec::new();
  let mut expect_name = true;
  loop {
    open = open.checked_sub(1)?;
    match tokens.get(open)? {
      Token::Punct('{') => break,
      Token::Punct(',') => expect_name = true,
      Token::Ident(id) => {
        // Reading right-to-left, an element is `name` or `name as alias`; the
        // *last* identifier we see before the `{` or a `,` is the imported
        // name.
        if *id == "as" {
          // The alias we just recorded is not the imported name; the next
          // identifier to the left is.
          names.pop();
          expect_name = true;
        } else if expect_name {
          names.push((*id).to_string());
          expect_name = false;
        } else {
          // Two adjacent identifiers with no `as`: not an import clause.
          return None;
        }
      }
      _ => return None,
    }
  }
  // ... import | export
  match tokens.get(open.checked_sub(1)?)? {
    Token::Ident(k) if *k == "import" || *k == "export" => {}
    // `import defaultName, { ... } from` — a default import of `ext:core/ops`
    // is meaningless, and we would rather not guess.
    _ => return None,
  }
  // Validate that every name looks like a plain identifier.
  if names.iter().any(|n| !is_ident(n)) {
    return None;
  }
  Some(names)
}

fn is_ident(s: &str) -> bool {
  let mut chars = s.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// A very small JS token stream — just enough to find import declarations
/// without being fooled by comments, strings, template literals or regexes.
#[derive(Debug, PartialEq)]
enum Token<'a> {
  Ident(&'a str),
  Str(String),
  Punct(char),
  /// Numbers, template literals, regexes, and anything else we do not care
  /// about but must not mistake for one of the above.
  Other,
}

fn tokenize(src: &str) -> Vec<Token<'_>> {
  let b = src.as_bytes();
  let mut i = 0;
  let mut out: Vec<Token> = Vec::new();
  while i < b.len() {
    let c = b[i];
    match c {
      b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => i += 1,
      // Comments. `//` and `/*` are unambiguous: an empty regex `//` and a
      // regex starting `/*` are both syntax errors, so a `/` followed by `/`
      // or `*` always begins a comment.
      b'/' if b.get(i + 1) == Some(&b'/') => {
        i += 2;
        while i < b.len() && b[i] != b'\n' {
          i += 1;
        }
      }
      b'/' if b.get(i + 1) == Some(&b'*') => {
        i += 2;
        while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
          i += 1;
        }
        i = (i + 2).min(b.len());
      }
      b'/' => {
        // Divide or regex. Use the classic previous-token heuristic; either way
        // the result is `Token::Other`, so all this decides is how far to skip.
        if regex_allowed(out.last()) {
          i = skip_regex(b, i);
        } else {
          i += 1;
        }
        out.push(Token::Other);
      }
      b'"' | b'\'' => {
        let (s, next) = read_string(b, i);
        i = next;
        match s {
          Some(s) => out.push(Token::Str(s)),
          None => out.push(Token::Other),
        }
      }
      b'`' => {
        i = skip_template(b, i);
        out.push(Token::Other);
      }
      b'0'..=b'9' => {
        i += 1;
        while i < b.len()
          && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_')
        {
          i += 1;
        }
        out.push(Token::Other);
      }
      _ if is_ident_start(c) => {
        let start = i;
        i += 1;
        while i < b.len() && is_ident_part(b[i]) {
          i += 1;
        }
        out.push(Token::Ident(&src[start..i]));
      }
      _ => {
        out.push(Token::Punct(c as char));
        i += 1;
      }
    }
  }
  out
}

fn is_ident_start(c: u8) -> bool {
  c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c >= 0x80
}

fn is_ident_part(c: u8) -> bool {
  c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

/// After which tokens may a `/` start a regex literal?
fn regex_allowed(prev: Option<&Token>) -> bool {
  match prev {
    None => true,
    Some(Token::Str(_)) | Some(Token::Other) => false,
    Some(Token::Punct(c)) => !matches!(c, ')' | ']' | '}'),
    Some(Token::Ident(id)) => matches!(
      *id,
      "return"
        | "typeof"
        | "instanceof"
        | "in"
        | "of"
        | "new"
        | "delete"
        | "void"
        | "throw"
        | "case"
        | "do"
        | "else"
        | "yield"
        | "await"
    ),
  }
}

/// Reads a quoted string starting at `b[i]`. Returns the decoded contents when
/// the literal contains no escapes (which is all we need — the specifier has
/// none) and the index just past the closing quote.
fn read_string(b: &[u8], i: usize) -> (Option<String>, usize) {
  let quote = b[i];
  let mut j = i + 1;
  let start = j;
  let mut escaped = false;
  while j < b.len() {
    match b[j] {
      b'\\' => {
        escaped = true;
        j += 2;
        continue;
      }
      c if c == quote => {
        let s = if escaped {
          None
        } else {
          std::str::from_utf8(&b[start..j]).ok().map(str::to_string)
        };
        return (s, j + 1);
      }
      // Unterminated string literal (newline). Give up on this token.
      b'\n' => return (None, j + 1),
      _ => j += 1,
    }
  }
  (None, b.len())
}

fn skip_template(b: &[u8], i: usize) -> usize {
  // Track `${ ... }` substitutions so a backtick inside one does not terminate
  // the outer literal early. Nested templates are handled by the depth stack.
  let mut j = i + 1;
  let mut brace_depth: Vec<usize> = Vec::new();
  let mut cur: usize = 0;
  while j < b.len() {
    match b[j] {
      b'\\' => j += 2,
      b'$' if b.get(j + 1) == Some(&b'{') => {
        brace_depth.push(cur);
        cur = 1;
        j += 2;
      }
      b'{' if cur > 0 => {
        cur += 1;
        j += 1;
      }
      b'}' if cur > 0 => {
        cur -= 1;
        if cur == 0 {
          cur = brace_depth.pop().unwrap_or(0);
        }
        j += 1;
      }
      b'`' if cur == 0 => return j + 1,
      _ => j += 1,
    }
  }
  b.len()
}

fn skip_regex(b: &[u8], i: usize) -> usize {
  let mut j = i + 1;
  let mut in_class = false;
  while j < b.len() {
    match b[j] {
      b'\\' => j += 2,
      b'[' => {
        in_class = true;
        j += 1;
      }
      b']' => {
        in_class = false;
        j += 1;
      }
      b'/' if !in_class => {
        j += 1;
        while j < b.len() && b[j].is_ascii_alphabetic() {
          j += 1;
        }
        return j;
      }
      // A newline means it was not a regex after all; treat the `/` as one
      // character of punctuation.
      b'\n' => return i + 1,
      _ => j += 1,
    }
  }
  i + 1
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names(src: &str) -> Option<Vec<String>> {
    let mut v = scan_source(src)?;
    v.sort();
    Some(v)
  }

  #[test]
  fn single_line_named_import() {
    assert_eq!(
      names(r#"import { op_a, op_b } from "ext:core/ops";"#),
      Some(vec!["op_a".to_string(), "op_b".to_string()])
    );
  }

  #[test]
  fn multiline_named_import() {
    let src = r#"
// Copyright.
import {
  op_a,
  op_b,
  ImageBitmap,
} from "ext:core/ops";
import { core } from "ext:core/mod.js";
"#;
    assert_eq!(
      names(src),
      Some(vec![
        "ImageBitmap".to_string(),
        "op_a".to_string(),
        "op_b".to_string()
      ])
    );
  }

  #[test]
  fn aliased_import_records_the_exported_name() {
    assert_eq!(
      names(r#"import { op_a as a, op_b as b } from "ext:core/ops";"#),
      Some(vec!["op_a".to_string(), "op_b".to_string()])
    );
  }

  #[test]
  fn single_quotes_and_reexport() {
    assert_eq!(
      names("export { op_a } from 'ext:core/ops';"),
      Some(vec!["op_a".to_string()])
    );
  }

  #[test]
  fn mention_in_a_line_comment_is_not_an_import() {
    // This exact shape exists in deno's `cli/js/40_test.js`. It must not be
    // mistaken for an import, and it must not force export-all either.
    let src = r#"
// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
import { op_a } from "ext:core/ops";
"#;
    assert_eq!(names(src), Some(vec!["op_a".to_string()]));
  }

  #[test]
  fn mention_in_a_block_comment_is_not_an_import() {
    let src = r#"
/* import { op_x } from "ext:core/ops"; */
import { op_a } from "ext:core/ops";
"#;
    assert_eq!(names(src), Some(vec!["op_a".to_string()]));
  }

  #[test]
  fn namespace_import_forces_export_all() {
    assert_eq!(names(r#"import * as ops from "ext:core/ops";"#), None);
  }

  #[test]
  fn dynamic_import_forces_export_all() {
    assert_eq!(names(r#"const m = await import("ext:core/ops");"#), None);
  }

  #[test]
  fn star_reexport_forces_export_all() {
    assert_eq!(names(r#"export * from "ext:core/ops";"#), None);
  }

  #[test]
  fn default_plus_named_forces_export_all() {
    assert_eq!(names(r#"import d, { op_a } from "ext:core/ops";"#), None);
  }

  #[test]
  fn specifier_inside_a_string_forces_export_all() {
    // We cannot tell what an embedder does with this, so bail.
    assert_eq!(names(r#"const s = "ext:core/ops";"#), None);
  }

  #[test]
  fn side_effect_import_needs_no_exports() {
    assert_eq!(names(r#"import "ext:core/ops";"#), Some(vec![]));
  }

  #[test]
  fn regex_containing_quotes_does_not_confuse_the_scanner() {
    let src = r#"
import { op_a } from "ext:core/ops";
const re = /["']ext:core\/ops["']/;
const d = 4 / 2 / 1;
"#;
    assert_eq!(names(src), Some(vec!["op_a".to_string()]));
  }

  #[test]
  fn template_literal_with_substitution() {
    let src = r#"
import { op_a } from "ext:core/ops";
const t = `a ${ `b ${ 1 } c` } "ext:core/ops" d`;
"#;
    assert_eq!(names(src), Some(vec!["op_a".to_string()]));
  }

  #[test]
  fn scan_accumulates_and_is_sticky() {
    let mut scan = OpsImportScan::default();
    scan.add_source(r#"import { op_a } from "ext:core/ops";"#);
    scan.add_source(r#"import { op_b } from "ext:core/ops";"#);
    scan.add_source("const x = 1;");
    let f = scan.finish();
    assert_eq!(f.len(), Some(2));
    assert!(f.allows("op_a"));
    assert!(f.allows("op_b"));
    assert!(!f.allows("op_c"));

    let mut scan = OpsImportScan::default();
    scan.add_source(r#"import { op_a } from "ext:core/ops";"#);
    scan.add_source(r#"import * as ops from "ext:core/ops";"#);
    let f = scan.finish();
    assert_eq!(f.len(), None);
    assert!(f.allows("anything_at_all"));
  }

  #[test]
  fn forced_export_all_short_circuits() {
    let mut scan = OpsImportScan::default();
    scan.force_export_all();
    scan.add_source(r#"import { op_a } from "ext:core/ops";"#);
    assert_eq!(scan.finish().len(), None);
  }
}
