// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::{type_error, AnyError};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;

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
  super::reg_json_sync(rt, "op_system_cpu_info", op_system_cpu_info);
}

fn op_exec_path(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let current_exe = env::current_exe().unwrap();
  state
    .borrow_mut::<Permissions>()
    .read
    .check_blind(&current_exe, "exec_path")?;
  // Now apply URL parser to current exe to get fully resolved path, otherwise
  // we might get `./` and `../` bits in `exec_path`
  let exe_url = Url::from_file_path(current_exe).unwrap();
  let path = exe_url.to_file_path().unwrap();
  Ok(json!(path))
}

#[derive(Deserialize)]
pub struct SetEnv {
  key: String,
  value: String,
}

fn op_set_env(
  state: &mut OpState,
  args: SetEnv,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow_mut::<Permissions>().env.check()?;
  let invalid_key =
    args.key.is_empty() || args.key.contains(&['=', '\0'] as &[char]);
  let invalid_value = args.value.contains('\0');
  if invalid_key || invalid_value {
    return Err(type_error("Key or value contains invalid characters."));
  }
  env::set_var(args.key, args.value);
  Ok(json!({}))
}

fn op_env(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow_mut::<Permissions>().env.check()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(json!(v))
}

#[derive(Deserialize)]
pub struct GetEnv {
  key: String,
}

fn op_get_env(
  state: &mut OpState,
  args: GetEnv,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow_mut::<Permissions>().env.check()?;
  if args.key.is_empty() || args.key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  let r = match env::var(args.key) {
    Err(env::VarError::NotPresent) => json!([]),
    v => json!([v?]),
  };
  Ok(r)
}

#[derive(Deserialize)]
pub struct DeleteEnv {
  key: String,
}

fn op_delete_env(
  state: &mut OpState,
  args: DeleteEnv,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow_mut::<Permissions>().env.check()?;
  if args.key.is_empty() || args.key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  env::remove_var(args.key);
  Ok(json!({}))
}

#[derive(Deserialize)]
pub struct Exit {
  code: i32,
}

fn op_exit(
  _state: &mut OpState,
  args: Exit,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  std::process::exit(args.code)
}

fn op_loadavg(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.loadavg");
  state.borrow_mut::<Permissions>().env.check()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok(json!([loadavg.one, loadavg.five, loadavg.fifteen])),
    Err(_) => Ok(json!([0f64, 0f64, 0f64])),
  }
}

fn op_hostname(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.hostname");
  state.borrow_mut::<Permissions>().env.check()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(json!(hostname))
}

fn op_os_release(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.osRelease");
  state.borrow_mut::<Permissions>().env.check()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(json!(release))
}

fn op_system_memory_info(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.systemMemoryInfo");
  state.borrow_mut::<Permissions>().env.check()?;
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

fn op_system_cpu_info(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.systemCpuInfo");
  state.borrow_mut::<Permissions>().env.check()?;

  let cores = sys_info::cpu_num().ok();
  let speed = sys_info::cpu_speed().ok();

  Ok(json!({
    "cores": cores,
    "speed": speed
  }))
}
