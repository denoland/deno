// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::ErrBox;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde_derive::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use url::Url;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_exit", op_exit);
  super::reg_json_sync(rt, "op_env", op_env);
  super::reg_json_sync(rt, "op_exec_path", op_exec_path);
  super::reg_json_sync(rt, "op_set_env", op_set_env);
  super::reg_json_sync(rt, "op_get_env", op_get_env);
  super::reg_json_sync(rt, "op_delete_env", op_delete_env);
  super::reg_json_sync(rt, "op_hostname", op_hostname);
  super::reg_json_sync(rt, "op_loadavg", op_loadavg);
  super::reg_json_sync(rt, "op_os_release", op_os_release);
  super::reg_json_sync(rt, "op_system_memory_info", op_system_memory_info);
}

fn op_exec_path(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let current_exe = env::current_exe().unwrap();
  let cli_state = super::cli_state(state);
  cli_state.check_read_blind(&current_exe, "exec_path")?;
  // Now apply URL parser to current exe to get fully resolved path, otherwise
  // we might get `./` and `../` bits in `exec_path`
  let exe_url = Url::from_file_path(current_exe).unwrap();
  let path = exe_url.to_file_path().unwrap();
  Ok(json!(path))
}

#[derive(Deserialize)]
struct SetEnv {
  key: String,
  value: String,
}

fn op_set_env(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: SetEnv = serde_json::from_value(args)?;
  let cli_state = super::cli_state(state);
  cli_state.check_env()?;
  env::set_var(args.key, args.value);
  Ok(json!({}))
}

fn op_env(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let cli_state = super::cli_state(state);
  cli_state.check_env()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(json!(v))
}

#[derive(Deserialize)]
struct GetEnv {
  key: String,
}

fn op_get_env(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: GetEnv = serde_json::from_value(args)?;
  let cli_state = super::cli_state(state);
  cli_state.check_env()?;
  let r = match env::var(args.key) {
    Err(env::VarError::NotPresent) => json!([]),
    v => json!([v?]),
  };
  Ok(r)
}

#[derive(Deserialize)]
struct DeleteEnv {
  key: String,
}

fn op_delete_env(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: DeleteEnv = serde_json::from_value(args)?;
  let cli_state = super::cli_state(state);
  cli_state.check_env()?;
  env::remove_var(args.key);
  Ok(json!({}))
}

#[derive(Deserialize)]
struct Exit {
  code: i32,
}

fn op_exit(
  _state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: Exit = serde_json::from_value(args)?;
  std::process::exit(args.code)
}

fn op_loadavg(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let cli_state = super::cli_state(state);
  cli_state.check_unstable("Deno.loadavg");
  cli_state.check_env()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok(json!([loadavg.one, loadavg.five, loadavg.fifteen])),
    Err(_) => Ok(json!([0f64, 0f64, 0f64])),
  }
}

fn op_hostname(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let cli_state = super::cli_state(state);
  cli_state.check_unstable("Deno.hostname");
  cli_state.check_env()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(json!(hostname))
}

fn op_os_release(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let cli_state = super::cli_state(state);
  cli_state.check_unstable("Deno.osRelease");
  cli_state.check_env()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(json!(release))
}

fn op_system_memory_info(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let cli_state = super::cli_state(state);
  cli_state.check_unstable("Deno.systemMemoryInfo");
  cli_state.check_env()?;
  match sys_info::mem_info() {
    Ok(info) => Ok(json!({
      "total": info.total,
      "free": info.free,
      "available": info.avail,
      "buffers": info.buffers,
      "cached": info.cached,
      "swapTotal": info.swap_total,
      "swapFree": info.swap_free
    })),
    Err(_) => Ok(json!({})),
  }
}
