// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_runtime::deno_telemetry::OtelRuntimeConfig;

pub fn has_trace_permissions_enabled() -> bool {
  has_flag_env_var("DENO_TRACE_PERMISSIONS")
}

pub fn has_flag_env_var(name: &str) -> bool {
  let value = std::env::var(name);
  matches!(value.as_ref().map(|s| s.as_str()), Ok("1"))
}
