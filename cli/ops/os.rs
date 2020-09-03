// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, Value};
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;
use url::Url;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  let t = &CoreIsolate::state(i).borrow().resource_table.clone();

  i.register_op("op_exit", s.stateful_json_op_sync(t, op_exit));
  i.register_op("op_env", s.stateful_json_op_sync(t, op_env));
  i.register_op("op_exec_path", s.stateful_json_op_sync(t, op_exec_path));
  i.register_op("op_set_env", s.stateful_json_op_sync(t, op_set_env));
  i.register_op("op_get_env", s.stateful_json_op_sync(t, op_get_env));
  i.register_op("op_delete_env", s.stateful_json_op_sync(t, op_delete_env));
  i.register_op("op_hostname", s.stateful_json_op_sync(t, op_hostname));
  i.register_op("op_loadavg", s.stateful_json_op_sync(t, op_loadavg));
  i.register_op("op_os_release", s.stateful_json_op_sync(t, op_os_release));
}

fn op_exec_path(
  state: &State,
  _resource_table: &mut ResourceTable,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let current_exe = env::current_exe().unwrap();
  state.check_read_blind(&current_exe, "exec_path")?;
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
  state: &State,
  _resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: SetEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::set_var(args.key, args.value);
  Ok(json!({}))
}

fn op_env(
  state: &State,
  _resource_table: &mut ResourceTable,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_env()?;
  let v = env::vars().collect::<HashMap<String, String>>();
  Ok(json!(v))
}

#[derive(Deserialize)]
struct GetEnv {
  key: String,
}

fn op_get_env(
  state: &State,
  _resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: GetEnv = serde_json::from_value(args)?;
  state.check_env()?;
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
  state: &State,
  _resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: DeleteEnv = serde_json::from_value(args)?;
  state.check_env()?;
  env::remove_var(args.key);
  Ok(json!({}))
}

#[derive(Deserialize)]
struct Exit {
  code: i32,
}

fn op_exit(
  _state: &State,
  _resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: Exit = serde_json::from_value(args)?;
  std::process::exit(args.code)
}

fn op_loadavg(
  state: &State,
  _resource_table: &mut ResourceTable,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.loadavg");
  state.check_env()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok(json!([loadavg.one, loadavg.five, loadavg.fifteen])),
    Err(_) => Ok(json!([0f64, 0f64, 0f64])),
  }
}

fn op_hostname(
  state: &State,
  _resource_table: &mut ResourceTable,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.hostname");
  state.check_env()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(json!(hostname))
}

fn op_os_release(
  state: &State,
  _resource_table: &mut ResourceTable,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.osRelease");
  state.check_env()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(json!(release))
}
