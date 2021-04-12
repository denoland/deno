// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::permissions::Permissions;
use deno_core::error::{type_error, AnyError};
use deno_core::url::Url;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;
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
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let current_exe = env::current_exe().unwrap();
  state
    .borrow_mut::<Permissions>()
    .read
    .check_blind(&current_exe, "exec_path")?;
  // Now apply URL parser to current exe to get fully resolved path, otherwise
  // we might get `./` and `../` bits in `exec_path`
  let exe_url = Url::from_file_path(current_exe).unwrap();
  let path = exe_url.to_file_path().unwrap();

  into_string(path.into_os_string())
}

#[derive(Deserialize)]
pub struct SetEnv {
  key: String,
  value: String,
}

fn op_set_env(
  state: &mut OpState,
  args: SetEnv,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  state.borrow_mut::<Permissions>().env.check(&args.key)?;
  let invalid_key =
    args.key.is_empty() || args.key.contains(&['=', '\0'] as &[char]);
  let invalid_value = args.value.contains('\0');
  if invalid_key || invalid_value {
    return Err(type_error("Key or value contains invalid characters."));
  }
  env::set_var(args.key, args.value);
  Ok(())
}

fn op_env(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<HashMap<String, String>, AnyError> {
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(env::vars().collect())
}

fn op_get_env(
  state: &mut OpState,
  key: String,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Option<String>, AnyError> {
  state.borrow_mut::<Permissions>().env.check(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  let r = match env::var(key) {
    Err(env::VarError::NotPresent) => None,
    v => Some(v?),
  };
  Ok(r)
}

fn op_delete_env(
  state: &mut OpState,
  key: String,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  state.borrow_mut::<Permissions>().env.check(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  env::remove_var(key);
  Ok(())
}

fn op_exit(
  _state: &mut OpState,
  code: i32,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  std::process::exit(code)
}

fn op_loadavg(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(f64, f64, f64), AnyError> {
  super::check_unstable(state, "Deno.loadavg");
  state.borrow_mut::<Permissions>().env.check_all()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok((loadavg.one, loadavg.five, loadavg.fifteen)),
    Err(_) => Ok((0.0, 0.0, 0.0)),
  }
}

fn op_hostname(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.hostname");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(hostname)
}

fn op_os_release(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.osRelease");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(release)
}

// Copied from sys-info/lib.rs (then tweaked)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemInfo {
  pub total: u64,
  pub free: u64,
  pub available: u64,
  pub buffers: u64,
  pub cached: u64,
  pub swap_total: u64,
  pub swap_free: u64,
}

fn op_system_memory_info(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Option<MemInfo>, AnyError> {
  super::check_unstable(state, "Deno.systemMemoryInfo");
  state.borrow_mut::<Permissions>().env.check_all()?;
  match sys_info::mem_info() {
    Ok(info) => Ok(Some(MemInfo {
      total: info.total,
      free: info.free,
      available: info.avail,
      buffers: info.buffers,
      cached: info.cached,
      swap_total: info.swap_total,
      swap_free: info.swap_free,
    })),
    Err(_) => Ok(None),
  }
}

#[derive(Serialize)]
struct CpuInfo {
  cores: Option<u32>,
  speed: Option<u64>,
}

fn op_system_cpu_info(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<CpuInfo, AnyError> {
  super::check_unstable(state, "Deno.systemCpuInfo");
  state.borrow_mut::<Permissions>().env.check_all()?;

  let cores = sys_info::cpu_num().ok();
  let speed = sys_info::cpu_speed().ok();

  Ok(CpuInfo { cores, speed })
}
