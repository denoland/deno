// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use serde::Serialize;
use urlpattern::quirks;
use urlpattern::quirks::StringOrInit;

deno_error::js_error_wrapper!(urlpattern::Error, UrlPatternError, "TypeError");

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
    quirks::process_construct_pattern_input(input, base_url.as_deref())?;

  let pattern = quirks::parse_pattern(init, options)?;

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
