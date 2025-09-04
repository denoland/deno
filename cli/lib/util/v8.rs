// Copyright 2018-2025 the Deno authors. MIT license.

#[inline(always)]
pub fn construct_v8_flags(
  default_v8_flags: &[String],
  v8_flags: &[String],
  env_v8_flags: Vec<String>,
) -> Vec<String> {
  std::iter::once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
    .chain(default_v8_flags.iter().cloned())
    .chain(env_v8_flags)
    .chain(v8_flags.iter().cloned())
    .collect::<Vec<_>>()
}
