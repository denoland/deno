// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::utils::into_string;
use crate::worker::ExitCode;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use deno_node::NODE_ENV_VAR_ALLOWLIST;
use deno_permissions::PermissionsContainer;
use serde::Serialize;
use std::collections::HashMap;
use std::env;

mod sys_info;

deno_core::extension!(
  deno_os,
  ops = [
    op_env,
    op_exec_path,
    op_exit,
    op_delete_env,
    op_get_env,
    op_gid,
    op_hostname,
    op_loadavg,
    op_network_interfaces,
    op_os_release,
    op_os_uptime,
    op_set_env,
    op_set_exit_code,
    op_get_exit_code,
    op_system_memory_info,
    op_uid,
    op_runtime_memory_usage,
  ],
  options = {
    exit_code: ExitCode,
  },
  state = |state, options| {
    state.put::<ExitCode>(options.exit_code);
  },
);

deno_core::extension!(
  deno_os_worker,
  ops = [
    op_env,
    op_exec_path,
    op_exit,
    op_delete_env,
    op_get_env,
    op_gid,
    op_hostname,
    op_loadavg,
    op_network_interfaces,
    op_os_release,
    op_os_uptime,
    op_set_env,
    op_set_exit_code,
    op_get_exit_code,
    op_system_memory_info,
    op_uid,
    op_runtime_memory_usage,
  ],
  middleware = |op| match op.name {
    "op_exit" | "op_set_exit_code" | "op_get_exit_code" =>
      op.with_implementation_from(&deno_core::op_void_sync()),
    _ => op,
  },
);

#[op2]
#[string]
fn op_exec_path(state: &mut OpState) -> Result<String, AnyError> {
  // Rust has platform-dependent symlink resolution, so we need to be clever
  // if we want to get the called path to the executable.
  // See:
  // https://github.com/rust-lang/rust/issues/43617
  // https://github.com/rivy/rs.coreutils/commit/c457dfbbc469333e7f116971b610a8e3fa1cdf66
  let current_exe = match std::env::args().next() {
    Some(ref s) if !s.is_empty() => {
      let argv0 = std::path::PathBuf::from(s);
      if argv0.is_absolute() {
        argv0
      } else {
        std::env::current_dir().unwrap().join(argv0)
      }
    }
    _ => std::env::current_exe().unwrap(),
  };
  state
    .borrow_mut::<PermissionsContainer>()
    .check_read_blind(&current_exe, "exec_path", "Deno.execPath()")?;
  // normalize path so it doesn't include '.' or '..' components
  let path = normalize_path(current_exe);

  into_string(path.into_os_string())
}

#[op2(fast)]
fn op_set_env(
  state: &mut OpState,
  #[string] key: &str,
  #[string] value: &str,
) -> Result<(), AnyError> {
  state.borrow_mut::<PermissionsContainer>().check_env(key)?;
  if key.is_empty() {
    return Err(type_error("Key is an empty string."));
  }
  if key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error(format!(
      "Key contains invalid characters: {key:?}"
    )));
  }
  if value.contains('\0') {
    return Err(type_error(format!(
      "Value contains invalid characters: {value:?}"
    )));
  }
  env::set_var(key, value);
  Ok(())
}

#[op2]
#[serde]
fn op_env(state: &mut OpState) -> Result<HashMap<String, String>, AnyError> {
  state.borrow_mut::<PermissionsContainer>().check_env_all()?;
  Ok(env::vars().collect())
}

#[op2]
#[string]
fn op_get_env(
  state: &mut OpState,
  #[string] key: String,
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
      "Key contains invalid characters: {key:?}"
    )));
  }

  let r = match env::var(key) {
    Err(env::VarError::NotPresent) => None,
    v => Some(v?),
  };
  Ok(r)
}

#[op2(fast)]
fn op_delete_env(
  state: &mut OpState,
  #[string] key: String,
) -> Result<(), AnyError> {
  state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(type_error("Key contains invalid characters."));
  }
  env::remove_var(key);
  Ok(())
}

#[op2(fast)]
fn op_set_exit_code(state: &mut OpState, #[smi] code: i32) {
  state.borrow_mut::<ExitCode>().set(code);
}

#[op2(fast)]
#[smi]
fn op_get_exit_code(state: &mut OpState) -> i32 {
  state.borrow_mut::<ExitCode>().get()
}

#[op2(fast)]
fn op_exit(state: &mut OpState) {
  let code = state.borrow::<ExitCode>().get();
  std::process::exit(code)
}

#[op2]
#[serde]
fn op_loadavg(state: &mut OpState) -> Result<(f64, f64, f64), AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("loadavg", "Deno.loadavg()")?;
  Ok(sys_info::loadavg())
}

#[op2]
#[string]
fn op_hostname(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("hostname", "Deno.hostname()")?;
  Ok(sys_info::hostname())
}

#[op2]
#[string]
fn op_os_release(state: &mut OpState) -> Result<String, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("osRelease", "Deno.osRelease()")?;
  Ok(sys_info::os_release())
}

#[op2]
#[serde]
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
    let cidr = format!("{address:?}/{range}");

    let name = ifa.name().to_owned();
    let address = format!("{:?}", ifa.address());
    let netmask = format!("{:?}", ifa.netmask());
    let scopeid = ifa.scope_id();

    let [b0, b1, b2, b3, b4, b5] = ifa.mac();
    let mac = format!("{b0:02x}:{b1:02x}:{b2:02x}:{b3:02x}:{b4:02x}:{b5:02x}");

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

#[op2]
#[serde]
fn op_system_memory_info(
  state: &mut OpState,
) -> Result<Option<sys_info::MemInfo>, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("systemMemoryInfo", "Deno.systemMemoryInfo()")?;
  Ok(sys_info::mem_info())
}

#[cfg(not(windows))]
#[op2]
#[smi]
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
#[op2]
#[smi]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("gid", "Deno.gid()")?;
  Ok(None)
}

#[cfg(not(windows))]
#[op2]
#[smi]
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
#[op2]
#[smi]
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

#[op2]
#[serde]
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

#[cfg(any(target_os = "android", target_os = "linux"))]
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
      if n.is_ascii_digit() {
        out *= 10;
        out += n as usize - '0' as usize;
      } else {
        break;
      }
    }
    (out, idx)
  }

  #[allow(clippy::disallowed_methods)]
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

#[cfg(target_os = "openbsd")]
fn rss() -> usize {
  // Uses OpenBSD's KERN_PROC_PID sysctl(2)
  // to retrieve information about the current
  // process, part of which is the RSS (p_vm_rssize)

  // SAFETY: libc call (get PID of own process)
  let pid = unsafe { libc::getpid() };
  // SAFETY: libc call (get system page size)
  let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
  // KERN_PROC_PID returns a struct libc::kinfo_proc
  let mut kinfoproc = std::mem::MaybeUninit::<libc::kinfo_proc>::uninit();
  let mut size = std::mem::size_of_val(&kinfoproc) as libc::size_t;
  let mut mib = [
    libc::CTL_KERN,
    libc::KERN_PROC,
    libc::KERN_PROC_PID,
    pid,
    // mib is an array of integers, size is of type size_t
    // conversion is safe, because the size of a libc::kinfo_proc
    // structure will not exceed i32::MAX
    size.try_into().unwrap(),
    1,
  ];
  // SAFETY: libc call, mib has been statically initialized,
  // kinfoproc is a valid pointer to a libc::kinfo_proc struct
  let res = unsafe {
    libc::sysctl(
      mib.as_mut_ptr(),
      mib.len() as _,
      kinfoproc.as_mut_ptr() as *mut libc::c_void,
      &mut size,
      std::ptr::null_mut(),
      0,
    )
  };

  if res == 0 {
    // SAFETY: sysctl returns 0 on success and kinfoproc is initialized
    // p_vm_rssize contains size in pages -> multiply with pagesize to
    // get size in bytes.
    pagesize * unsafe { (*kinfoproc.as_mut_ptr()).p_vm_rssize as usize }
  } else {
    0
  }
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

#[op2(fast)]
#[number]
fn op_os_uptime(state: &mut OpState) -> Result<u64, AnyError> {
  os_uptime(state)
}
