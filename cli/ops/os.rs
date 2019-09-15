// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::colors;
use crate::fs as deno_fs;
use crate::state::ThreadSafeState;
use crate::version;
use atty;
use deno::*;
use log;
use std::collections::HashMap;
use std::env;
use url::Url;

/// BUILD_OS and BUILD_ARCH match the values in Deno.build. See js/build.ts.
#[cfg(target_os = "macos")]
static BUILD_OS: &str = "mac";
#[cfg(target_os = "linux")]
static BUILD_OS: &str = "linux";
#[cfg(target_os = "windows")]
static BUILD_OS: &str = "win";
#[cfg(target_arch = "x86_64")]
static BUILD_ARCH: &str = "x64";

pub fn op_start(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  Ok(JsonOp::Sync(json!({
    "cwd": deno_fs::normalize_path(&env::current_dir().unwrap()),
    "pid": std::process::id(),
    "argv": state.argv,
    "mainModule": state.main_module().map(|x| x.as_str().to_string()),
    "debugFlag": state
      .flags
      .log_level
      .map_or(false, |l| l == log::Level::Debug),
    "versionFlag": state.flags.version,
    "v8Version": version::v8(),
    "denoVersion": version::DENO,
    "tsVersion": version::typescript(),
    "noColor": !colors::use_color(),
    "xevalDelim": state.flags.xeval_delim.clone(),
    "os": BUILD_OS,
    "arch": BUILD_ARCH,
  })))
}

pub fn op_home_dir(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let path = dirs::home_dir()
    .unwrap_or_default()
    .into_os_string()
    .into_string()
    .unwrap_or_default();
  Ok(JsonOp::Sync(json!(path)))
}

pub fn op_exec_path(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let current_exe = env::current_exe().unwrap();
  // Now apply URL parser to current exe to get fully resolved path, otherwise
  // we might get `./` and `../` bits in `exec_path`
  let exe_url = Url::from_file_path(current_exe).unwrap();
  let path = exe_url.to_file_path().unwrap();
  Ok(JsonOp::Sync(json!(path)))
}

#[derive(Deserialize)]
struct SetEnv {
  key: String,
  value: String,
}

pub fn op_set_env(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::set_var(args.key, args.value);
  Ok(JsonOp::Sync(json!({})))
}

pub fn op_env(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(JsonOp::Sync(json!(v)))
}

#[derive(Deserialize)]
struct Exit {
  code: i32,
}

pub fn op_exit(
  _s: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: Exit = serde_json::from_value(args)?;
  std::process::exit(args.code)
}

pub fn op_is_tty(
  _s: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  Ok(JsonOp::Sync(json!({
    "stdin": atty::is(atty::Stream::Stdin),
    "stdout": atty::is(atty::Stream::Stdout),
    "stderr": atty::is(atty::Stream::Stderr),
  })))
}
