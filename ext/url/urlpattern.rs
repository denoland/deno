// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;

use urlpattern::quirks;
use urlpattern::quirks::MatchInput;
use urlpattern::quirks::StringOrInit;
use urlpattern::quirks::UrlPattern;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct UrlPatternError(urlpattern::Error);

#[op2]
#[serde]
pub fn op_urlpattern_parse(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
  #[serde] options: urlpattern::UrlPatternOptions,
) -> Result<UrlPattern, UrlPatternError> {
  let init =
    quirks::process_construct_pattern_input(input, base_url.as_deref())
      .map_err(UrlPatternError)?;

  let pattern =
    quirks::parse_pattern(init, options).map_err(UrlPatternError)?;

  Ok(pattern)
}

#[op2]
#[serde]
pub fn op_urlpattern_process_match_input(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
) -> Result<Option<(MatchInput, quirks::Inputs)>, UrlPatternError> {
  let res = quirks::process_match_input(input, base_url.as_deref())
    .map_err(UrlPatternError)?;

  let (input, inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  Ok(quirks::parse_match_input(input).map(|input| (input, inputs)))
}
