// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::permissions::Permissions;
use deno_core::error::{type_error, AnyError};
use deno_core::op_sync;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::OpState;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

pub fn init(maybe_exit_code: Option<Arc<AtomicI32>>) -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_env", op_sync(op_env)),
      ("op_exec_path", op_sync(op_exec_path)),
      ("op_exit", op_sync(op_exit)),
      ("op_delete_env", op_sync(op_delete_env)),
      ("op_get_env", op_sync(op_get_env)),
      ("op_getuid", op_sync(op_getuid)),
      ("op_hostname", op_sync(op_hostname)),
      ("op_loadavg", op_sync(op_loadavg)),
      ("op_network_interfaces", op_sync(op_network_interfaces)),
      ("op_os_release", op_sync(op_os_release)),
      ("op_set_env", op_sync(op_set_env)),
      ("op_set_exit_code", op_sync(op_set_exit_code)),
      ("op_system_memory_info", op_sync(op_system_memory_info)),
    ])
    .state(move |state| {
      let exit_code = maybe_exit_code.clone().unwrap_or_default();
      state.put::<Arc<AtomicI32>>(exit_code);
      Ok(())
    })
    .build()
}

fn op_exec_path(state: &mut OpState, _: (), _: ()) -> Result<String, AnyError> {
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

fn op_set_env(
  state: &mut OpState,
  key: String,
  value: String,
) -> Result<(), AnyError> {
  state.borrow_mut::<Permissions>().env.check(&key)?;
  let invalid_key = key.is_empty() || key.contains(&['=', '\0'] as &[char]);
  let invalid_value = value.contains('\0');
  if invalid_key || invalid_value {
    return Err(type_error("Key or value contains invalid characters."));
  }
  env::set_var(key, value);
  Ok(())
}

fn op_env(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<HashMap<String, String>, AnyError> {
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(env::vars().collect())
}

fn op_get_env(
  state: &mut OpState,
  key: String,
  _: (),
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
  _: (),
) -> Result<(), AnyError> {
  state.borrow_mut::<Permissions>().env.check(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  env::remove_var(key);
  Ok(())
}

fn op_set_exit_code(
  state: &mut OpState,
  code: i32,
  _: (),
) -> Result<(), AnyError> {
  state.borrow_mut::<Arc<AtomicI32>>().store(code, Relaxed);
  Ok(())
}

fn op_exit(state: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  let code = state.borrow::<Arc<AtomicI32>>().load(Relaxed);
  std::process::exit(code)
}

fn op_loadavg(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<(f64, f64, f64), AnyError> {
  super::check_unstable(state, "Deno.loadavg");
  state.borrow_mut::<Permissions>().env.check_all()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok((loadavg.one, loadavg.five, loadavg.fifteen)),
    Err(_) => Ok((0.0, 0.0, 0.0)),
  }
}

fn op_hostname(state: &mut OpState, _: (), _: ()) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.hostname");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(hostname)
}

fn op_os_release(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.osRelease");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(release)
}

fn op_network_interfaces(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Vec<NetworkInterface>, AnyError> {
  super::check_unstable(state, "Deno.networkInterfaces");
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(netif::up()?.map(NetworkInterface::from).collect())
}

#[derive(serde::Serialize)]
struct NetworkInterface {
  family: &'static str,
  name: String,
  address: String,
  netmask: String,
  scopeid: Option<u32>,
  cidr: String,
  mac: String,
}

impl From<netif::Interface> for NetworkInterface {
  fn from(ifa: netif::Interface) -> Self {
    let family = match ifa.address() {
      std::net::IpAddr::V4(_) => "IPv4",
      std::net::IpAddr::V6(_) => "IPv6",
    };

    let (address, range) = ifa.cidr();
    let cidr = format!("{:?}/{}", address, range);

    let name = ifa.name().to_owned();
    let address = format!("{:?}", ifa.address());
    let netmask = format!("{:?}", ifa.netmask());
    let scopeid = ifa.scope_id();

    let [b0, b1, b2, b3, b4, b5] = ifa.mac();
    let mac = format!(
      "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
      b0, b1, b2, b3, b4, b5
    );

    Self {
      family,
      name,
      address,
      netmask,
      scopeid,
      cidr,
      mac,
    }
  }
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
  _: (),
  _: (),
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

#[cfg(not(windows))]
fn op_getuid(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getUid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  unsafe { Ok(Some(libc::getuid())) }
}

#[cfg(windows)]
fn op_getuid(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getUid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(None)
}
