// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;

use urlpattern::quirks::MatchInput;
use urlpattern::quirks::StringOrInit;
use urlpattern::quirks::UrlPattern;
use urlpattern::quirks::UrlPatternInit;

#[op2]
#[serde]
pub fn op_urlpattern_parse(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
) -> Result<UrlPattern, AnyError> {
  let init = urlpattern::quirks::process_construct_pattern_input(
    input,
    base_url.as_deref(),
  )
  .map_err(|e| type_error(e.to_string()))?;

  let pattern = urlpattern::quirks::parse_pattern(init)
    .map_err(|e| type_error(e.to_string()))?;

  Ok(pattern)
}

fn input_to_vec(input: MatchInput) -> Vec<String> {
  vec![
    input.protocol,
    input.username,
    input.password,
    input.hostname,
    input.port,
    input.pathname,
    input.search,
    input.hash,
  ]
}

fn url_pattern_init_to_vec(input: UrlPatternInit) -> Vec<Option<String>> {
  vec![
    input.protocol,
    input.username,
    input.password,
    input.hostname,
    input.port,
    input.pathname,
    input.search,
    input.hash,
    input.base_url,
  ]
}

type OpUrlPatternProcessMatchInputRet =
  (Vec<String>, Vec<Option<String>>, Option<String>);

#[op2]
#[serde]
pub fn op_urlpattern_process_match_input(
  #[serde] input: StringOrInit,
  #[string] base_url: Option<String>,
) -> Result<Option<OpUrlPatternProcessMatchInputRet>, AnyError> {
  let res = urlpattern::quirks::process_match_input(input, base_url.as_deref())
    .map_err(|e| type_error(e.to_string()))?;

  let (input, inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  let inputs_v = match inputs.0 {
    StringOrInit::String(s) => vec![Some(s)],
    StringOrInit::Init(init) => url_pattern_init_to_vec(init),
  };
  let inputs_maybe_s = inputs.1;

  Ok(
    urlpattern::quirks::parse_match_input(input)
      .map(|input| (input_to_vec(input), inputs_v, inputs_maybe_s)),
  )
}

#[op2]
#[serde]
pub fn op_urlpattern_process_match_input_test(
  #[serde] input: UrlPatternInit,
  #[string] base_url: Option<String>,
) -> Result<Option<Vec<String>>, AnyError> {
  let res = urlpattern::quirks::process_match_input(
    StringOrInit::Init(input),
    base_url.as_deref(),
  )
  .map_err(|e| type_error(e.to_string()))?;

  let (input, _inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  Ok(urlpattern::quirks::parse_match_input(input).map(input_to_vec))
}

#[op2]
#[serde]
pub fn op_urlpattern_process_match_input_test_string(
  #[string] input: String,
  #[string] base_url: Option<String>,
) -> Result<Option<Vec<String>>, AnyError> {
  let res = urlpattern::quirks::process_match_input(
    StringOrInit::String(input),
    base_url.as_deref(),
  )
  .map_err(|e| type_error(e.to_string()))?;

  let (input, _inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  Ok(urlpattern::quirks::parse_match_input(input).map(input_to_vec))
}
