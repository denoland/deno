use deno_core::error::type_error;
use deno_core::error::AnyError;

use urlpattern::quirks;
use urlpattern::quirks::MatchInput;
use urlpattern::quirks::StringOrInit;
use urlpattern::quirks::UrlPattern;

pub fn op_urlpattern_parse(
  _state: &mut deno_core::OpState,
  input: StringOrInit,
  base_url: Option<String>,
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

pub fn op_urlpattern_process_match_input(
  _state: &mut deno_core::OpState,
  input: StringOrInit,
  base_url: Option<String>,
) -> Result<Option<(MatchInput, quirks::Inputs)>, AnyError> {
  let res = urlpattern::quirks::process_match_input(input, base_url.as_deref())
    .map_err(|e| type_error(e.to_string()))?;

  let (input, inputs) = match res {
    Some((input, inputs)) => (input, inputs),
    None => return Ok(None),
  };

  Ok(urlpattern::quirks::parse_match_input(input).map(|input| (input, inputs)))
}
