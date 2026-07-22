// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use serde::Serialize;
use urlpattern::quirks;
use urlpattern::quirks::StringOrInit;
use urlpattern::quirks::UrlPatternInit;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[error("{0}")]
pub struct UrlPatternError(String);

impl From<urlpattern::Error> for UrlPatternError {
  fn from(err: urlpattern::Error) -> Self {
    UrlPatternError(err.to_string())
  }
}

/// Turns an opaque `urlpattern` crate error into an actionable message by
/// echoing the offending pattern, pointing a caret at the failing character,
/// and adding a hint for the most common mistake (a `:` that does not start a
/// valid named group, e.g. file-router syntax like `[:slug]`).
fn enrich_url_pattern_error(
  err: urlpattern::Error,
  input: &StringOrInit,
) -> UrlPatternError {
  let message = err.to_string();

  // Tokenizer errors carry the (char) position of the offending token.
  let pos = match err {
    urlpattern::Error::Tokenizer(_, pos) => Some(pos),
    _ => None,
  };

  // Determine which pattern string the error refers to, and whether the
  // reported position reliably indexes into it. The crate parses each URL
  // component separately, so `pos` is relative to a single component string.
  // For an init object with exactly one component set we can attribute the
  // error to it and render a caret. For a constructor string `pos` is relative
  // to whichever component the string expanded to, not the whole string, so we
  // echo the input but cannot place a caret.
  let (pattern, caret_pos) = match input {
    StringOrInit::String(s) => (Some(("URLPattern", s.as_str())), None),
    StringOrInit::Init(init) => (single_init_component(init), pos),
  };

  let Some((name, pattern)) = pattern else {
    return UrlPatternError(message);
  };

  let mut out = format!("Failed to parse {name} from \"{pattern}\": {message}");

  if let Some(pos) = caret_pos {
    out.push_str("\n\n  ");
    out.push_str(pattern);
    out.push_str("\n  ");
    // `pos` is a count of chars; pattern inputs are practically ASCII, so a
    // space per preceding char aligns the caret under the offending one.
    for _ in 0..pos {
      out.push(' ');
    }
    out.push('^');
  }

  if message.contains("invalid name") {
    out.push_str(
      "\n\n  hint: \":\" starts a named group and must be followed by a name \
       (a letter or \"_\", then letters, digits or \"_\"). To match a literal \
       \":\", escape it as \"\\:\".",
    );
  }

  UrlPatternError(out)
}

/// Returns the single set component of a `UrlPatternInit` (with its field name),
/// or `None` if zero or more than one component is set.
fn single_init_component(
  init: &UrlPatternInit,
) -> Option<(&'static str, &str)> {
  let components: [(&'static str, &Option<String>); 8] = [
    ("protocol", &init.protocol),
    ("username", &init.username),
    ("password", &init.password),
    ("hostname", &init.hostname),
    ("port", &init.port),
    ("pathname", &init.pathname),
    ("search", &init.search),
    ("hash", &init.hash),
  ];
  let mut set = components
    .iter()
    .filter_map(|(name, value)| value.as_deref().map(|value| (*name, value)));
  match (set.next(), set.next()) {
    (Some(only), None) => Some(only),
    _ => None,
  }
}

/// Lean version of UrlPatternComponent that excludes the unused `matcher` field.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UrlPatternComponent {
  pattern_string: String,
  regexp_string: String,
  group_name_list: Vec<String>,
}

impl From<urlpattern::quirks::UrlPatternComponent> for UrlPatternComponent {
  fn from(c: urlpattern::quirks::UrlPatternComponent) -> Self {
    Self {
      pattern_string: c.pattern_string,
      regexp_string: c.regexp_string,
      group_name_list: c.group_name_list,
    }
  }
}

/// Lean version of UrlPattern that uses our trimmed UrlPatternComponent.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UrlPatternResult {
  protocol: UrlPatternComponent,
  username: UrlPatternComponent,
  password: UrlPatternComponent,
  hostname: UrlPatternComponent,
  port: UrlPatternComponent,
  pathname: UrlPatternComponent,
  search: UrlPatternComponent,
  hash: UrlPatternComponent,
  has_regexp_groups: bool,
}

#[op2]
#[serde]
pub fn op_urlpattern_parse(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
  #[serde] options: urlpattern::UrlPatternOptions,
) -> Result<UrlPatternResult, UrlPatternError> {
  let init =
    quirks::process_construct_pattern_input(input.clone(), base_url.as_deref())
      .map_err(|e| enrich_url_pattern_error(e, &input))?;

  let pattern = quirks::parse_pattern(init, options)
    .map_err(|e| enrich_url_pattern_error(e, &input))?;

  Ok(UrlPatternResult {
    protocol: pattern.protocol.into(),
    username: pattern.username.into(),
    password: pattern.password.into(),
    hostname: pattern.hostname.into(),
    port: pattern.port.into(),
    pathname: pattern.pathname.into(),
    search: pattern.search.into(),
    hash: pattern.hash.into(),
    has_regexp_groups: pattern.has_regexp_groups,
  })
}

/// Processes match input and returns a concatenated string of all 8 URL
/// component values, writing their offsets into `buf`.
///
/// Returns `None` if the input doesn't parse as a valid URL.
///
/// Buffer layout (9 u32 values):
///   buf[0..8] = cumulative start offset for each component
///   buf[8]    = total byte length (end offset of last component)
///
/// The returned string is the concatenation of:
///   protocol + username + password + hostname + port + pathname + search + hash
///
/// To extract component `i`: `str.slice(buf[i], buf[i+1])`
#[op2]
#[string]
pub fn op_urlpattern_process_match_input(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
  #[buffer] buf: &mut [u32],
) -> Result<Option<String>, UrlPatternError> {
  let res = quirks::process_match_input(input, base_url.as_deref())?;

  let (input, _inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  let match_input = match quirks::parse_match_input(input) {
    Some(mi) => mi,
    None => return Ok(None),
  };

  let fields = [
    &match_input.protocol,
    &match_input.username,
    &match_input.password,
    &match_input.hostname,
    &match_input.port,
    &match_input.pathname,
    &match_input.search,
    &match_input.hash,
  ];

  let total_len: usize = fields.iter().map(|f| f.len()).sum();
  let mut concat = String::with_capacity(total_len);
  let mut offset = 0u32;

  for (i, field) in fields.iter().enumerate() {
    buf[i] = offset;
    offset += field.len() as u32;
    concat.push_str(field);
  }
  buf[8] = offset;

  Ok(Some(concat))
}
