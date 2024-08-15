// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod convert;

#[inline(always)]
pub fn get_v8_flags_from_env() -> Vec<String> {
  crate::args::env::DENO_V8_FLAGS
    .map(|flags| flags.split(',').map(String::from).collect::<Vec<String>>())
    .unwrap_or_default()
}

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

pub fn init_v8_flags(
  default_v8_flags: &[String],
  v8_flags: &[String],
  env_v8_flags: Vec<String>,
) {
  if default_v8_flags.is_empty()
    && v8_flags.is_empty()
    && env_v8_flags.is_empty()
  {
    return;
  }

  let v8_flags_includes_help = env_v8_flags
    .iter()
    .chain(v8_flags)
    .any(|flag| flag == "-help" || flag == "--help");
  // Keep in sync with `standalone.rs`.
  let v8_flags = construct_v8_flags(default_v8_flags, v8_flags, env_v8_flags);
  let unrecognized_v8_flags = deno_core::v8_set_flags(v8_flags)
    .into_iter()
    .skip(1)
    .collect::<Vec<_>>();

  #[allow(clippy::print_stderr)]
  if !unrecognized_v8_flags.is_empty() {
    for f in unrecognized_v8_flags {
      eprintln!("error: V8 did not recognize flag '{f}'");
    }
    eprintln!("\nFor a list of V8 flags, use '--v8-flags=--help'");
    std::process::exit(1);
  }
  if v8_flags_includes_help {
    std::process::exit(0);
  }
}
