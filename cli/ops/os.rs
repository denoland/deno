// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::colors;
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use crate::version;
use atty;
use deno::*;
use log;
use std::collections::HashMap;
use std::env;
use sys_info;
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

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("exit", s.core_op(json_op(s.stateful_op(op_exit))));
  i.register_op("is_tty", s.core_op(json_op(s.stateful_op(op_is_tty))));
  i.register_op("env", s.core_op(json_op(s.stateful_op(op_env))));
  i.register_op("exec_path", s.core_op(json_op(s.stateful_op(op_exec_path))));
  i.register_op("set_env", s.core_op(json_op(s.stateful_op(op_set_env))));
  i.register_op("get_env", s.core_op(json_op(s.stateful_op(op_get_env))));
  i.register_op("home_dir", s.core_op(json_op(s.stateful_op(op_home_dir))));
  i.register_op("hostname", s.core_op(json_op(s.stateful_op(op_hostname))));
  i.register_op("start", s.core_op(json_op(s.stateful_op(op_start))));
}

fn op_start(
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
    "tsVersion": version::TYPESCRIPT,
    "noColor": !colors::use_color(),
    "os": BUILD_OS,
    "arch": BUILD_ARCH,
  })))
}

fn op_home_dir(
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

fn op_exec_path(
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

fn op_set_env(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::set_var(args.key, args.value);
  Ok(JsonOp::Sync(json!({})))
}

fn op_env(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(JsonOp::Sync(json!(v)))
}

#[derive(Deserialize)]
struct GetEnv {
  key: String,
}

fn op_get_env(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: GetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  let r = match env::var(args.key) {
    Err(env::VarError::NotPresent) => json!([]),
    v => json!([v?]),
  };
  Ok(JsonOp::Sync(r))
}

#[derive(Deserialize)]
struct Exit {
  code: i32,
}

fn op_exit(
  _s: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: Exit = serde_json::from_value(args)?;
  std::process::exit(args.code)
}

fn op_is_tty(
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

fn op_hostname(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_owned());
  Ok(JsonOp::Sync(json!(hostname)))
}
