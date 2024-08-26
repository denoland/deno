// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;

use urlpattern::quirks;
use urlpattern::quirks::MatchInput;
use urlpattern::quirks::StringOrInit;
use urlpattern::quirks::UrlPattern;

deno_core::js_error_wrapper!(urlpattern::Error, JsUrlPatternError, "TypeError");

#[op2]
#[serde]
pub fn op_urlpattern_parse(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
  #[serde] options: urlpattern::UrlPatternOptions,
) -> Result<UrlPattern, AnyError> {
  let init = urlpattern::quirks::process_construct_pattern_input(
    input,
    base_url.as_deref(),
  )
  .map_err(JsUrlPatternError)?;

  let pattern = urlpattern::quirks::parse_pattern(init, options)
    .map_err(JsUrlPatternError)?;

  Ok(pattern)
}

#[op2]
#[serde]
pub fn op_urlpattern_process_match_input(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
) -> Result<Option<(MatchInput, quirks::Inputs)>, AnyError> {
  let res = urlpattern::quirks::process_match_input(input, base_url.as_deref())
    .map_err(JsUrlPatternError)?;

  let (input, inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  Ok(urlpattern::quirks::parse_match_input(input).map(|input| (input, inputs)))
}
