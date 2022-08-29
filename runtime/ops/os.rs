// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::permissions::Permissions;
use crate::worker::ExitCode;
use deno_core::error::{type_error, AnyError};
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::{op, ExtensionBuilder};
use serde::Serialize;
use std::collections::HashMap;
use std::env;

fn init_ops(builder: &mut ExtensionBuilder) -> &mut ExtensionBuilder {
  builder.ops(vec![
    op_env::decl(),
    op_exec_path::decl(),
    op_exit::decl(),
    op_delete_env::decl(),
    op_get_env::decl(),
    op_getgid::decl(),
    op_getuid::decl(),
    op_hostname::decl(),
    op_loadavg::decl(),
    op_network_interfaces::decl(),
    op_os_release::decl(),
    op_set_env::decl(),
    op_set_exit_code::decl(),
    op_system_memory_info::decl(),
  ])
}

pub fn init(exit_code: ExitCode) -> Extension {
  let mut builder = Extension::builder();
  init_ops(&mut builder)
    .state(move |state| {
      state.put::<ExitCode>(exit_code.clone());
      Ok(())
    })
    .build()
}

pub fn init_for_worker() -> Extension {
  let mut builder = Extension::builder();
  init_ops(&mut builder)
    .middleware(|op| match op.name {
      "op_exit" => noop_op::decl(),
      "op_set_exit_code" => noop_op::decl(),
      _ => op,
    })
    .build()
}

#[op]
fn noop_op() -> Result<(), AnyError> {
  Ok(())
}

#[op]
fn op_exec_path(state: &mut OpState) -> Result<String, AnyError> {
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

#[op]
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

#[op]
fn op_env(state: &mut OpState) -> Result<HashMap<String, String>, AnyError> {
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(env::vars().collect())
}

#[op]
fn op_get_env(
  state: &mut OpState,
  key: String,
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

#[op]
fn op_delete_env(state: &mut OpState, key: String) -> Result<(), AnyError> {
  state.borrow_mut::<Permissions>().env.check(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  env::remove_var(key);
  Ok(())
}

#[op]
fn op_set_exit_code(state: &mut OpState, code: i32) {
  state.borrow_mut::<ExitCode>().set(code);
}

#[op]
fn op_exit(state: &mut OpState) {
  let code = state.borrow::<ExitCode>().get();
  std::process::exit(code)
}

#[op]
fn op_loadavg(state: &mut OpState) -> Result<(f64, f64, f64), AnyError> {
  super::check_unstable(state, "Deno.loadavg");
  state.borrow_mut::<Permissions>().env.check_all()?;
  match sys_info::loadavg() {
    Ok(loadavg) => Ok((loadavg.one, loadavg.five, loadavg.fifteen)),
    Err(_) => Ok((0.0, 0.0, 0.0)),
  }
}

#[op]
fn op_hostname(state: &mut OpState) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.hostname");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let hostname = sys_info::hostname().unwrap_or_else(|_| "".to_string());
  Ok(hostname)
}

#[op]
fn op_os_release(state: &mut OpState) -> Result<String, AnyError> {
  super::check_unstable(state, "Deno.osRelease");
  state.borrow_mut::<Permissions>().env.check_all()?;
  let release = sys_info::os_release().unwrap_or_else(|_| "".to_string());
  Ok(release)
}

#[op]
fn op_network_interfaces(
  state: &mut OpState,
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

#[op]
fn op_system_memory_info(
  state: &mut OpState,
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
#[op]
fn op_getgid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getGid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    Ok(Some(libc::getgid()))
  }
}

#[cfg(windows)]
#[op]
fn op_getgid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getGid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(None)
}

#[cfg(not(windows))]
#[op]
fn op_getuid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getUid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    Ok(Some(libc::getuid()))
  }
}

#[cfg(windows)]
#[op]
fn op_getuid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  super::check_unstable(state, "Deno.getUid");
  state.borrow_mut::<Permissions>().env.check_all()?;
  Ok(None)
}
