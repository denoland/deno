// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::colors;
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use crate::version;
use atty;
use deno::*;
use std::collections::HashMap;
use std::env;
use std::io::{Error, ErrorKind};
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
  i.register_op("get_dir", s.core_op(json_op(s.stateful_op(op_get_dir))));
  i.register_op("hostname", s.core_op(json_op(s.stateful_op(op_hostname))));
  i.register_op("start", s.core_op(json_op(s.stateful_op(op_start))));
}

fn op_start(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let gs = &state.global_state;

  Ok(JsonOp::Sync(json!({
    "cwd": deno_fs::normalize_path(&env::current_dir().unwrap()),
    "pid": std::process::id(),
    "argv": gs.flags.argv,
    "mainModule": gs.main_module.as_ref().map(|x| x.to_string()),
    "debugFlag": gs.flags.log_level.map_or(false, |l| l == log::Level::Debug),
    "versionFlag": gs.flags.version,
    "v8Version": version::v8(),
    "denoVersion": version::DENO,
    "tsVersion": version::TYPESCRIPT,
    "noColor": !colors::use_color(),
    "os": BUILD_OS,
    "arch": BUILD_ARCH,
  })))
}

#[derive(Deserialize)]
struct GetDirArgs {
  kind: std::string::String,
}

fn op_get_dir(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_env()?;
  let args: GetDirArgs = serde_json::from_value(args)?;

  let path = match args.kind.as_str() {
    "home" => dirs::home_dir(),
    "config" => dirs::config_dir(),
    "cache" => dirs::cache_dir(),
    "data" => dirs::data_dir(),
    "data_local" => dirs::data_local_dir(),
    "audio" => dirs::audio_dir(),
    "desktop" => dirs::desktop_dir(),
    "document" => dirs::document_dir(),
    "download" => dirs::download_dir(),
    "font" => dirs::font_dir(),
    "picture" => dirs::picture_dir(),
    "public" => dirs::public_dir(),
    "template" => dirs::template_dir(),
    "video" => dirs::video_dir(),
    _ => {
      return Err(ErrBox::from(Error::new(
        ErrorKind::InvalidInput,
        format!("Invalid dir type `{}`", args.kind.as_str()),
      )))
    }
  };

  if path == None {
    Err(ErrBox::from(Error::new(
      ErrorKind::NotFound,
      format!("Could not get user {} directory.", args.kind.as_str()),
    )))
  } else {
    Ok(JsonOp::Sync(json!(path
      .unwrap_or_default()
      .into_os_string()
      .into_string()
      .unwrap_or_default())))
  }
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
