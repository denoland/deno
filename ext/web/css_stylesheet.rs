// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal native implementation of `CSSStyleSheet` to support CSS module
//! scripts (`import sheet from "./a.css" with { type: "css" }`):
//! https://html.spec.whatwg.org/multipage/webappapis.html#css-module-script
//!
//! Deno has no DOM, so a sheet can't be adopted anywhere; the implementation
//! is backed by the raw CSS text. `cssRules` performs a naive top-level rule
//! split (tracking braces, strings and comments) instead of real CSS parsing,
//! which is enough to read rules back out for SSR-style serialization.
//! Exposed as a global only when `--unstable-raw-imports` is enabled.

use std::borrow::Cow;
use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;

pub struct CSSRule {
  text: String,
}

// SAFETY: we're sure `CSSRule` can be GCed
unsafe impl GarbageCollected for CSSRule {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CSSRule"
  }
}

#[op2]
impl CSSRule {
  // `CSSRule` instances are only created internally (from
  // `CSSStyleSheet.prototype.cssRules`).
  #[constructor]
  #[cppgc]
  fn constructor() -> Result<CSSRule, deno_error::JsErrorBox> {
    Err(deno_error::JsErrorBox::type_error("Illegal constructor"))
  }

  #[getter]
  #[string]
  fn css_text(&self) -> String {
    self.text.clone()
  }
}

/// Splits a style sheet's text into its top-level rules. This is not a real
/// CSS parser: it only tracks brace depth while skipping comments and
/// strings, so each returned chunk is the verbatim text of one top-level
/// rule (or one at-statement like `@import "x";`).
///
/// All delimiters are ASCII, so iterating over bytes is safe for any UTF-8
/// input.
fn split_top_level_rules(text: &str) -> Vec<String> {
  let bytes = text.as_bytes();
  let len = bytes.len();
  let mut chunks = Vec::new();
  let mut depth = 0usize;
  let mut start = 0usize;
  let mut i = 0usize;

  let push_chunk = |from: usize, to: usize, chunks: &mut Vec<String>| {
    let chunk = text[from..to].trim();
    if !chunk.is_empty() {
      chunks.push(chunk.to_string());
    }
  };

  while i < len {
    let c = bytes[i];
    if c == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i += 2;
      continue;
    }
    if c == b'"' || c == b'\'' {
      i += 1;
      while i < len {
        let q = bytes[i];
        if q == b'\\' {
          i += 2;
          continue;
        }
        i += 1;
        if q == c {
          break;
        }
      }
      continue;
    }
    if c == b'{' {
      depth += 1;
    } else if c == b'}' {
      depth = depth.saturating_sub(1);
      if depth == 0 {
        push_chunk(start, i + 1, &mut chunks);
        start = i + 1;
      }
    } else if c == b';' && depth == 0 {
      push_chunk(start, i + 1, &mut chunks);
      start = i + 1;
    }
    i += 1;
  }
  if start < len {
    push_chunk(start, len, &mut chunks);
  }
  chunks
}

/// Returns true if a top-level rule is an `@import` at-rule. At-rule keywords
/// are ASCII case-insensitive, and `@import` is always followed by whitespace,
/// a string quote, or `url(` (i.e. never another identifier character).
fn is_import_rule(rule: &str) -> bool {
  let Some(rest) = rule.strip_prefix('@') else {
    return false;
  };
  let Some((keyword, after)) = rest.split_at_checked(6) else {
    return false;
  };
  keyword.eq_ignore_ascii_case("import")
    && after
      .chars()
      .next()
      .is_none_or(|c| c.is_ascii_whitespace() || c == '"' || c == '\'')
}

/// Re-serializes `text` with its top-level `@import` rules removed. Constructed
/// style sheets disallow `@import`, so `replace()`/`replaceSync()` drop them
/// (matching the CSSOM spec) rather than fetch them.
fn strip_import_rules(text: &str) -> String {
  split_top_level_rules(text)
    .into_iter()
    .filter(|rule| !is_import_rule(rule))
    .collect::<Vec<_>>()
    .join("\n")
}

pub struct CSSStyleSheet {
  text: RefCell<String>,
}

// SAFETY: we're sure `CSSStyleSheet` can be GCed
unsafe impl GarbageCollected for CSSStyleSheet {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CSSStyleSheet"
  }
}

fn make_css_rules<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  text: &str,
) -> v8::Local<'a, v8::Value> {
  let chunks = split_top_level_rules(text);
  let elements: Vec<v8::Local<v8::Value>> = chunks
    .into_iter()
    .map(|text| cppgc::make_cppgc_object(scope, CSSRule { text }).into())
    .collect();
  let arr = v8::Array::new_with_elements(scope, &elements);
  arr.set_integrity_level(scope, v8::IntegrityLevel::Frozen);
  arr.into()
}

#[op2]
impl CSSStyleSheet {
  // `options` (`media`, `disabled`, `baseURL`) are not supported by this
  // minimal implementation.
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor() -> CSSStyleSheet {
    CSSStyleSheet {
      text: RefCell::new(String::new()),
    }
  }

  /// Note: returns a frozen array of `CSSRule` instead of a live
  /// `CSSRuleList`, and a fresh array is created on each access.
  #[getter]
  fn css_rules<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    make_css_rules(scope, &self.text.borrow())
  }

  #[required(1)]
  fn replace_sync(&self, #[webidl] text: String) {
    *self.text.borrow_mut() = strip_import_rules(&text);
  }

  // Intentionally not `#[required(1)]`: `replace` returns a promise, and WebIDL
  // requires the "not enough arguments" `TypeError` of a promise-returning
  // operation to be turned into a rejected promise rather than thrown
  // synchronously. The argument count is checked manually below so the missing
  // and invalid argument cases both reject. (`replaceSync` keeps
  // `#[required(1)]` since it does throw synchronously.)
  fn replace<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) -> v8::Local<'a, v8::Promise> {
    let resolver = v8::PromiseResolver::new(scope).unwrap();
    let promise = resolver.get_promise(scope);
    let num_args = args.map(|args| args.length()).unwrap_or(0);
    if num_args < 1 {
      // Match the message `#[required(1)]` would have produced.
      let msg = format!(
        "Failed to execute 'replace' on 'CSSStyleSheet': 1 argument required, but only {} present",
        num_args,
      );
      let msg = v8::String::new(scope, &msg).unwrap();
      let exception = v8::Exception::type_error(scope, msg);
      resolver.reject(scope, exception);
      return promise;
    }
    let args = args.expect("checked above that an argument is present");
    match String::convert(
      scope,
      args.get(0),
      Cow::Borrowed("Failed to execute 'replace' on 'CSSStyleSheet'"),
      ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
      &Default::default(),
    ) {
      Ok(text) => {
        *self.text.borrow_mut() = strip_import_rules(&text);
        let this: v8::Local<v8::Value> = args.this().into();
        resolver.resolve(scope, this);
      }
      Err(err) => {
        let exception = deno_core::error::to_v8_error(scope, &err);
        resolver.reject(scope, exception);
      }
    }
    promise
  }
}

/// Creates a `CSSStyleSheet` instance containing the given text. Used by the
/// custom module evaluation callback to back `with { type: "css" }` imports.
pub fn create_css_style_sheet<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  text: String,
) -> v8::Local<'a, v8::Object> {
  cppgc::make_cppgc_object(
    scope,
    CSSStyleSheet {
      text: RefCell::new(text),
    },
  )
}

#[cfg(test)]
mod tests {
  use super::split_top_level_rules;
  use super::strip_import_rules;

  #[test]
  fn strips_import_rules() {
    // Drops top-level `@import` (case-insensitively), keeps everything else,
    // including `@import` nested inside another rule's block.
    assert_eq!(
      strip_import_rules(
        "@import \"a.css\"; @IMPORT 'b.css';\nbody { color: red; }\n@media (x) { @import \"c\"; }",
      ),
      "body { color: red; }\n@media (x) { @import \"c\"; }"
    );
    // `@import-like` names are not `@import` rules and stay.
    assert_eq!(strip_import_rules("@imports { a: b }"), "@imports { a: b }");
  }

  #[test]
  fn splits_rules() {
    let rules = split_top_level_rules(
      "/* comment ; { */ body { color: red; } @import \"x;y.css\"; @media (x) { a { b: c } }",
    );
    // Comments are only skipped for delimiter tracking; each chunk is the
    // verbatim text of one top-level rule, so a leading comment stays
    // attached to the rule that follows it.
    assert_eq!(
      rules,
      vec![
        "/* comment ; { */ body { color: red; }",
        "@import \"x;y.css\";",
        "@media (x) { a { b: c } }",
      ]
    );
  }

  #[test]
  fn handles_strings_and_escapes() {
    let rules =
      split_top_level_rules("a::before { content: \"}\\\"{\" } b { c: 'd}' }");
    assert_eq!(
      rules,
      vec!["a::before { content: \"}\\\"{\" }", "b { c: 'd}' }"]
    );
  }

  #[test]
  fn handles_trailing_rest_and_empty() {
    assert_eq!(split_top_level_rules("  \n "), Vec::<String>::new());
    assert_eq!(
      split_top_level_rules("@charset \"utf-8\""),
      vec!["@charset \"utf-8\""]
    );
  }
}
