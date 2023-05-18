// https://github.com/denoland/deno/issues/16979
fn op_string_length(string: &str) -> Result<u32, AnyError> {
  Ok(string.len() as u32)
}
