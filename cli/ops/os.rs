// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::env;
use std::io::{Error, ErrorKind};
use url::Url;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_exit", s.stateful_json_op(op_exit));
  i.register_op("op_env", s.stateful_json_op(op_env));
  i.register_op("op_exec_path", s.stateful_json_op(op_exec_path));
  i.register_op("op_set_env", s.stateful_json_op(op_set_env));
  i.register_op("op_get_env", s.stateful_json_op(op_get_env));
  i.register_op("op_delete_env", s.stateful_json_op(op_delete_env));
  i.register_op("op_get_dir", s.stateful_json_op(op_get_dir));
  i.register_op("op_hostname", s.stateful_json_op(op_hostname));
  i.register_op("op_loadavg", s.stateful_json_op(op_loadavg));
  i.register_op("op_os_release", s.stateful_json_op(op_os_release));
}

#[derive(Deserialize)]
struct GetDirArgs {
  kind: std::string::String,
}

fn op_get_dir(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.dir");
  state.check_env()?;
  let args: GetDirArgs = serde_json::from_value(args)?;

  let path = match args.kind.as_str() {
    "home" => dirs::home_dir(),
    "config" => dirs::config_dir(),
    "cache" => dirs::cache_dir(),
    "executable" => dirs::executable_dir(),
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
    "tmp" => Some(std::env::temp_dir()),
    "video" => dirs::video_dir(),
    _ => {
      return Err(
        Error::new(
          ErrorKind::InvalidInput,
          format!("Invalid dir type `{}`", args.kind.as_str()),
        )
        .into(),
      )
    }
  };

  if path == None {
    Err(OpError::not_found(format!(
      "Could not get user {} directory.",
      args.kind.as_str()
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
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let current_exe = env::current_exe().unwrap();
  state.check_read_blind(&current_exe, "exec_path")?;
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
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: SetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::set_var(args.key, args.value);
  Ok(JsonOp::Sync(json!({})))
}

fn op_env(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_env()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(JsonOp::Sync(json!(v)))
}

#[derive(Deserialize)]
struct GetEnv {
  key: String,
}

fn op_get_env(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: GetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  let r = match env::var(args.key) {
    Err(env::VarError::NotPresent) => json!([]),
    v => json!([v?]),
  };
  Ok(JsonOp::Sync(r))
}

#[derive(Deserialize)]
struct DeleteEnv {
  key: String,
}

fn op_delete_env(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: DeleteEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::remove_var(args.key);
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct Exit {
  code: i32,
}

fn op_exit(
  _s: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: Exit = serde_json::from_value(args)?;
  std::process::exit(args.code)
}

fn op_loadavg(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.loadavg");
  state.check_env()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok(JsonOp::Sync(json!([
      loadavg.one,
      loadavg.five,
      loadavg.fifteen
    ]))),
    Err(_) => Ok(JsonOp::Sync(json!([0f64, 0f64, 0f64]))),
  }
}

fn op_hostname(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.hostname");
  state.check_env()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(JsonOp::Sync(json!(hostname)))
}

fn op_os_release(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.osRelease");
  state.check_env()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(JsonOp::Sync(json!(release)))
}
