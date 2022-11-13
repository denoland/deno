// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::permissions::Permissions;
use crate::worker::ExitCode;
use deno_core::error::{type_error, AnyError};
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::{op, ExtensionBuilder};
use deno_node::NODE_ENV_VAR_ALLOWLIST;
use std::collections::HashMap;
use std::env;

mod sys_info;

fn init_ops(builder: &mut ExtensionBuilder) -> &mut ExtensionBuilder {
  builder.ops(vec![
    op_env::decl(),
    op_exec_path::decl(),
    op_exit::decl(),
    op_delete_env::decl(),
    op_get_env::decl(),
    op_gid::decl(),
    op_hostname::decl(),
    op_loadavg::decl(),
    op_network_interfaces::decl(),
    op_os_release::decl(),
    op_set_env::decl(),
    op_set_exit_code::decl(),
    op_system_memory_info::decl(),
    op_uid::decl(),
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
  state.borrow_mut::<Permissions>().read.check_blind(
    &current_exe,
    "exec_path",
    "Deno.execPath()",
  )?;
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
  if key.is_empty() {
    return Err(type_error("Key is an empty string."));
  }
  if key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error(format!(
      "Key contains invalid characters: {:?}",
      key
    )));
  }
  if value.contains('\0') {
    return Err(type_error(format!(
      "Value contains invalid characters: {:?}",
      value
    )));
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
  let skip_permission_check = NODE_ENV_VAR_ALLOWLIST.contains(&key);

  if !skip_permission_check {
    state.borrow_mut::<Permissions>().env.check(&key)?;
  }

  if key.is_empty() {
    return Err(type_error("Key is an empty string."));
  }

  if key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error(format!(
      "Key contains invalid characters: {:?}",
      key
    )));
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
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("loadavg", Some("Deno.loadavg()"))?;
  Ok(sys_info::loadavg())
}

#[op]
fn op_hostname(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("hostname", Some("Deno.hostname()"))?;
  Ok(sys_info::hostname())
}

#[op]
fn op_os_release(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("osRelease", Some("Deno.osRelease()"))?;
  Ok(sys_info::os_release())
}

#[op]
fn op_network_interfaces(
  state: &mut OpState,
) -> Result<Vec<NetworkInterface>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("networkInterfaces", Some("Deno.networkInterfaces()"))?;
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

#[op]
fn op_system_memory_info(
  state: &mut OpState,
) -> Result<Option<sys_info::MemInfo>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("systemMemoryInfo", Some("Deno.systemMemoryInfo()"))?;
  Ok(sys_info::mem_info())
}

#[cfg(not(windows))]
#[op]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("gid", Some("Deno.gid()"))?;
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    Ok(Some(libc::getgid()))
  }
}

#[cfg(windows)]
#[op]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("gid", Some("Deno.gid()"))?;
  Ok(None)
}

#[cfg(not(windows))]
#[op]
fn op_uid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("uid", Some("Deno.uid()"))?;
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    Ok(Some(libc::getuid()))
  }
}

#[cfg(windows)]
#[op]
fn op_uid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<Permissions>()
    .sys
    .check("uid", Some("Deno.uid()"))?;
  Ok(None)
}
