// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::ffi::OsString;

use once_cell::sync::Lazy;

pub static DENO_UNSTABLE_BARE_NODE_BUILTINS: Lazy<bool> =
  Lazy::new(|| std::env::var_os("DENO_UNSTABLE_BARE_NODE_BUILTINS").is_some());

pub static DENO_UNSTABLE_BYONM: Lazy<bool> =
  Lazy::new(|| std::env::var_os("DENO_UNSTABLE_BYONM").is_some());

pub static DENO_UNSTABLE_SLOPPY_IMPORTS: Lazy<bool> =
  Lazy::new(|| std::env::var_os("DENO_UNSTABLE_SLOPPY_IMPORTS").is_some());

pub static DENO_AUTH_TOKENS: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_AUTH_TOKENS").ok());

pub static DENO_TLS_CA_STORE: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_TLS_CA_STORE").ok());

pub static DENO_CERT: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_CERT").ok());

pub static DENO_DIR: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_DIR").ok());

pub static DENO_INSTALL_ROOT: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_INSTALL_ROOT").ok());

pub static DENO_REPL_HISTORY: Lazy<Option<OsString>> =
  Lazy::new(|| std::env::var_os("DENO_REPL_HISTORY"));

pub static DENO_JOBS: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_JOBS").ok());

// TODO(bartlomieju): move to `ext/webgpu` or refactor to pass it explicitly
pub static DENO_WEBGPU_TRACE: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_WEBGPU_TRACE").ok());

pub static DENO_V8_FLAGS: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("DENO_V8_FLAGS").ok());

pub static NPM_CONFIG_REGISTRY: Lazy<Option<String>> =
  Lazy::new(|| std::env::var("NPM_CONFIG_REGISTRY").ok());

//   <g>DENO_NO_PACKAGE_JSON</> Disables auto-resolution of package.json

//     <g>DENO_NO_PROMPT</>       Set to disable permission prompts on access
//                          (alternative to passing --no-prompt on invocation)

//     <g>DENO_NO_UPDATE_CHECK</> Set to disable checking if a newer Deno version is
//                          available

//     <g>HTTP_PROXY</>           Proxy address for HTTP requests
//                          (module downloads, fetch)

//     <g>HTTPS_PROXY</>          Proxy address for HTTPS requests
//                          (module downloads, fetch)

//     <g>NO_COLOR</>             Set to disable color

//     <g>NO_PROXY</>             Comma-separated list of hosts which do not use a proxy
//                          (module downloads, fetch)"#
