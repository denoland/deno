// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::permissions::PermissionsContainer;
use crate::worker::ExitCode;
use deno_core::error::{type_error, AnyError};
use deno_core::op;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::Extension;
use deno_core::ExtensionBuilder;
use deno_core::OpState;
use deno_node::NODE_ENV_VAR_ALLOWLIST;
use serde::Serialize;
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
    op_os_uptime::decl(),
    op_node_unstable_os_uptime::decl(),
    op_set_env::decl(),
    op_set_exit_code::decl(),
    op_system_memory_info::decl(),
    op_uid::decl(),
    op_runtime_memory_usage::decl(),
  ])
}

pub fn init(exit_code: ExitCode) -> Extension {
  let mut builder = Extension::builder("deno_os");
  init_ops(&mut builder)
    .state(move |state| {
      state.put::<ExitCode>(exit_code.clone());
      Ok(())
    })
    .build()
}

pub fn init_for_worker() -> Extension {
  let mut builder = Extension::builder("deno_os_worker");
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
    .borrow_mut::<PermissionsContainer>()
    .check_read_blind(&current_exe, "exec_path", "Deno.execPath()")?;
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
  state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
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
  state.borrow_mut::<PermissionsContainer>().check_env_all()?;
  Ok(env::vars().collect())
}

#[op]
fn op_get_env(
  state: &mut OpState,
  key: String,
) -> Result<Option<String>, AnyError> {
  let skip_permission_check = NODE_ENV_VAR_ALLOWLIST.contains(&key);

  if !skip_permission_check {
    state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
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
  state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
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
    .borrow_mut::<PermissionsContainer>()
    .check_sys("loadavg", "Deno.loadavg()")?;
  Ok(sys_info::loadavg())
}

#[op]
fn op_hostname(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("hostname", "Deno.hostname()")?;
  Ok(sys_info::hostname())
}

#[op]
fn op_os_release(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("osRelease", "Deno.osRelease()")?;
  Ok(sys_info::os_release())
}

#[op]
fn op_network_interfaces(
  state: &mut OpState,
) -> Result<Vec<NetworkInterface>, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("networkInterfaces", "Deno.networkInterfaces()")?;
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
    .borrow_mut::<PermissionsContainer>()
    .check_sys("systemMemoryInfo", "Deno.systemMemoryInfo()")?;
  Ok(sys_info::mem_info())
}

#[cfg(not(windows))]
#[op]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("gid", "Deno.gid()")?;
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
    .borrow_mut::<PermissionsContainer>()
    .check_sys("gid", "Deno.gid()")?;
  Ok(None)
}

#[cfg(not(windows))]
#[op]
fn op_uid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("uid", "Deno.uid()")?;
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
    .borrow_mut::<PermissionsContainer>()
    .check_sys("uid", "Deno.uid()")?;
  Ok(None)
}

// HeapStats stores values from a isolate.get_heap_statistics() call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryUsage {
  rss: usize,
  heap_total: usize,
  heap_used: usize,
  external: usize,
}

#[op(v8)]
fn op_runtime_memory_usage(scope: &mut v8::HandleScope) -> MemoryUsage {
  let mut s = v8::HeapStatistics::default();
  scope.get_heap_statistics(&mut s);
  MemoryUsage {
    rss: rss(),
    heap_total: s.total_heap_size(),
    heap_used: s.used_heap_size(),
    external: s.external_memory(),
  }
}

#[cfg(target_os = "linux")]
fn rss() -> usize {
  // Inspired by https://github.com/Arc-blroth/memory-stats/blob/5364d0d09143de2a470d33161b2330914228fde9/src/linux.rs

  // Extracts a positive integer from a string that
  // may contain leading spaces and trailing chars.
  // Returns the extracted number and the index of
  // the next character in the string.
  fn scan_int(string: &str) -> (usize, usize) {
    let mut out = 0;
    let mut idx = 0;
    let mut chars = string.chars().peekable();
    while let Some(' ') = chars.next_if_eq(&' ') {
      idx += 1;
    }
    for n in chars {
      idx += 1;
      if ('0'..='9').contains(&n) {
        out *= 10;
        out += n as usize - '0' as usize;
      } else {
        break;
      }
    }
    (out, idx)
  }

  let statm_content = if let Ok(c) = std::fs::read_to_string("/proc/self/statm")
  {
    c
  } else {
    return 0;
  };

  // statm returns the virtual size and rss, in
  // multiples of the page size, as the first
  // two columns of output.
  // SAFETY: libc call
  let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };

  if page_size < 0 {
    return 0;
  }

  let (_total_size_pages, idx) = scan_int(&statm_content);
  let (total_rss_pages, _) = scan_int(&statm_content[idx..]);

  total_rss_pages * page_size as usize
}

#[cfg(target_os = "macos")]
fn rss() -> usize {
  // Inspired by https://github.com/Arc-blroth/memory-stats/blob/5364d0d09143de2a470d33161b2330914228fde9/src/darwin.rs

  let mut task_info =
    std::mem::MaybeUninit::<libc::mach_task_basic_info_data_t>::uninit();
  let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
  // SAFETY: libc calls
  let r = unsafe {
    libc::task_info(
      libc::mach_task_self(),
      libc::MACH_TASK_BASIC_INFO,
      task_info.as_mut_ptr() as libc::task_info_t,
      &mut count as *mut libc::mach_msg_type_number_t,
    )
  };
  // According to libuv this should never fail
  assert_eq!(r, libc::KERN_SUCCESS);
  // SAFETY: we just asserted that it was success
  let task_info = unsafe { task_info.assume_init() };
  task_info.resident_size as usize
}

#[cfg(windows)]
fn rss() -> usize {
  use winapi::shared::minwindef::DWORD;
  use winapi::shared::minwindef::FALSE;
  use winapi::um::processthreadsapi::GetCurrentProcess;
  use winapi::um::psapi::GetProcessMemoryInfo;
  use winapi::um::psapi::PROCESS_MEMORY_COUNTERS;

  // SAFETY: winapi calls
  unsafe {
    // this handle is a constant—no need to close it
    let current_process = GetCurrentProcess();
    let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();

    if GetProcessMemoryInfo(
      current_process,
      &mut pmc,
      std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as DWORD,
    ) != FALSE
    {
      pmc.WorkingSetSize
    } else {
      0
    }
  }
}

fn os_uptime(state: &mut OpState) -> Result<u64, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("osUptime", "Deno.osUptime()")?;
  Ok(sys_info::os_uptime())
}

#[op]
fn op_os_uptime(state: &mut OpState) -> Result<u64, AnyError> {
  super::check_unstable(state, "Deno.osUptime");
  os_uptime(state)
}

#[op]
fn op_node_unstable_os_uptime(state: &mut OpState) -> Result<u64, AnyError> {
  os_uptime(state)
}
