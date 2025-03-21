// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use deno_path_util::normalize_path;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
use once_cell::sync::Lazy;
use serde::Serialize;

mod ops;
pub mod signal;
pub mod sys_info;

pub use ops::signal::SignalError;

pub static NODE_ENV_VAR_ALLOWLIST: Lazy<HashSet<String>> = Lazy::new(|| {
  // The full list of environment variables supported by Node.js is available
  // at https://nodejs.org/api/cli.html#environment-variables
  let mut set = HashSet::new();
  set.insert("NODE_DEBUG".to_string());
  set.insert("NODE_OPTIONS".to_string());
  set
});

#[derive(Clone, Default)]
pub struct ExitCode(Arc<AtomicI32>);

impl ExitCode {
  pub fn get(&self) -> i32 {
    self.0.load(Ordering::Relaxed)
  }

  pub fn set(&mut self, code: i32) {
    self.0.store(code, Ordering::Relaxed);
  }
}

pub fn exit(code: i32) -> ! {
  deno_telemetry::flush();
  #[allow(clippy::disallowed_methods)]
  std::process::exit(code);
}

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
    op_runtime_cpu_usage,
    op_runtime_memory_usage,
    ops::signal::op_signal_bind,
    ops::signal::op_signal_unbind,
    ops::signal::op_signal_poll,
  ],
  esm = ["30_os.js", "40_signals.js"],
  options = {
    exit_code: ExitCode,
  },
  state = |state, options| {
    state.put::<ExitCode>(options.exit_code);
    #[cfg(unix)]
    {
      state.put(ops::signal::SignalState::default());
    }
  }
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
    op_runtime_cpu_usage,
    op_runtime_memory_usage,
    ops::signal::op_signal_bind,
    ops::signal::op_signal_unbind,
    ops::signal::op_signal_poll,
  ],
  esm = ["30_os.js", "40_signals.js"],
  middleware = |op| match op.name {
    "op_exit" | "op_set_exit_code" | "op_get_exit_code" =>
      op.with_implementation_from(&deno_core::op_void_sync()),
    _ => op,
  },
  state = |state| {
    #[cfg(unix)]
    {
      state.put(ops::signal::SignalState::default());
    }
  }
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum OsError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class("InvalidData")]
  #[error("File name or path {0:?} is not valid UTF-8")]
  InvalidUtf8(std::ffi::OsString),
  #[class(type)]
  #[error("Key is an empty string.")]
  EnvEmptyKey,
  #[class(type)]
  #[error("Key contains invalid characters: {0:?}")]
  EnvInvalidKey(String),
  #[class(type)]
  #[error("Value contains invalid characters: {0:?}")]
  EnvInvalidValue(String),
  #[class(inherit)]
  #[error(transparent)]
  Var(#[from] env::VarError),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

#[op2(stack_trace)]
#[string]
fn op_exec_path(state: &mut OpState) -> Result<String, OsError> {
  let current_exe = env::current_exe().unwrap();
  state
    .borrow_mut::<PermissionsContainer>()
    .check_read_blind(&current_exe, "exec_path", "Deno.execPath()")?;
  // normalize path so it doesn't include '.' or '..' components
  let path = normalize_path(current_exe);

  path
    .into_os_string()
    .into_string()
    .map_err(OsError::InvalidUtf8)
}

fn dt_change_notif(isolate: &mut v8::Isolate, key: &str) {
  extern "C" {
    #[cfg(unix)]
    fn tzset();

    #[cfg(windows)]
    fn _tzset();
  }

  if key == "TZ" {
    // SAFETY: tzset/_tzset (libc) is called to update the timezone information
    unsafe {
      #[cfg(unix)]
      tzset();

      #[cfg(windows)]
      _tzset();
    }

    isolate.date_time_configuration_change_notification(
      v8::TimeZoneDetection::Redetect,
    );
  }
}

#[op2(fast, stack_trace)]
fn op_set_env(
  state: &mut OpState,
  scope: &mut v8::HandleScope,
  #[string] key: &str,
  #[string] value: &str,
) -> Result<(), OsError> {
  state.borrow_mut::<PermissionsContainer>().check_env(key)?;
  if key.is_empty() {
    return Err(OsError::EnvEmptyKey);
  }
  if key.contains(&['=', '\0'] as &[char]) {
    return Err(OsError::EnvInvalidKey(key.to_string()));
  }
  if value.contains('\0') {
    return Err(OsError::EnvInvalidValue(value.to_string()));
  }

  env::set_var(key, value);
  dt_change_notif(scope, key);
  Ok(())
}

#[op2(stack_trace)]
#[serde]
fn op_env(
  state: &mut OpState,
) -> Result<HashMap<String, String>, PermissionCheckError> {
  state.borrow_mut::<PermissionsContainer>().check_env_all()?;
  Ok(env::vars().collect())
}

#[op2(stack_trace)]
#[string]
fn op_get_env(
  state: &mut OpState,
  #[string] key: String,
) -> Result<Option<String>, OsError> {
  let skip_permission_check = NODE_ENV_VAR_ALLOWLIST.contains(&key);

  if !skip_permission_check {
    state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
  }

  if key.is_empty() {
    return Err(OsError::EnvEmptyKey);
  }

  if key.contains(&['=', '\0'] as &[char]) {
    return Err(OsError::EnvInvalidKey(key.to_string()));
  }

  let r = match env::var(key) {
    Err(env::VarError::NotPresent) => None,
    v => Some(v?),
  };
  Ok(r)
}

#[op2(fast, stack_trace)]
fn op_delete_env(
  state: &mut OpState,
  #[string] key: String,
) -> Result<(), OsError> {
  state.borrow_mut::<PermissionsContainer>().check_env(&key)?;
  if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
    return Err(OsError::EnvInvalidKey(key.to_string()));
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
  exit(code)
}

#[op2(stack_trace)]
#[serde]
fn op_loadavg(
  state: &mut OpState,
) -> Result<(f64, f64, f64), PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("loadavg", "Deno.loadavg()")?;
  Ok(sys_info::loadavg())
}

#[op2(stack_trace, stack_trace)]
#[string]
fn op_hostname(state: &mut OpState) -> Result<String, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("hostname", "Deno.hostname()")?;
  Ok(sys_info::hostname())
}

#[op2(stack_trace)]
#[string]
fn op_os_release(state: &mut OpState) -> Result<String, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("osRelease", "Deno.osRelease()")?;
  Ok(sys_info::os_release())
}

#[op2(stack_trace)]
#[serde]
fn op_network_interfaces(
  state: &mut OpState,
) -> Result<Vec<NetworkInterface>, OsError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("networkInterfaces", "Deno.networkInterfaces()")?;
  Ok(netif::up()?.map(NetworkInterface::from).collect())
}

#[derive(Serialize)]
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

#[op2(stack_trace)]
#[serde]
fn op_system_memory_info(
  state: &mut OpState,
) -> Result<Option<sys_info::MemInfo>, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("systemMemoryInfo", "Deno.systemMemoryInfo()")?;
  Ok(sys_info::mem_info())
}

#[cfg(not(windows))]
#[op2(stack_trace)]
#[smi]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, PermissionCheckError> {
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
#[op2(stack_trace)]
#[smi]
fn op_gid(state: &mut OpState) -> Result<Option<u32>, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("gid", "Deno.gid()")?;
  Ok(None)
}

#[cfg(not(windows))]
#[op2(stack_trace)]
#[smi]
fn op_uid(state: &mut OpState) -> Result<Option<u32>, PermissionCheckError> {
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
#[op2(stack_trace)]
#[smi]
fn op_uid(state: &mut OpState) -> Result<Option<u32>, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("uid", "Deno.uid()")?;
  Ok(None)
}

#[op2]
#[serde]
fn op_runtime_cpu_usage() -> (usize, usize) {
  let (sys, user) = get_cpu_usage();
  (sys.as_micros() as usize, user.as_micros() as usize)
}

#[cfg(unix)]
fn get_cpu_usage() -> (std::time::Duration, std::time::Duration) {
  let mut rusage = std::mem::MaybeUninit::uninit();

  // Uses POSIX getrusage from libc
  // to retrieve user and system times
  // SAFETY: libc call
  let ret = unsafe { libc::getrusage(libc::RUSAGE_SELF, rusage.as_mut_ptr()) };
  if ret != 0 {
    return Default::default();
  }

  // SAFETY: already checked the result
  let rusage = unsafe { rusage.assume_init() };

  let sys = std::time::Duration::from_micros(rusage.ru_stime.tv_usec as u64)
    + std::time::Duration::from_secs(rusage.ru_stime.tv_sec as u64);
  let user = std::time::Duration::from_micros(rusage.ru_utime.tv_usec as u64)
    + std::time::Duration::from_secs(rusage.ru_utime.tv_sec as u64);

  (sys, user)
}

#[cfg(windows)]
fn get_cpu_usage() -> (std::time::Duration, std::time::Duration) {
  use winapi::shared::minwindef::FALSE;
  use winapi::shared::minwindef::FILETIME;
  use winapi::shared::minwindef::TRUE;
  use winapi::um::minwinbase::SYSTEMTIME;
  use winapi::um::processthreadsapi::GetCurrentProcess;
  use winapi::um::processthreadsapi::GetProcessTimes;
  use winapi::um::timezoneapi::FileTimeToSystemTime;

  fn convert_system_time(system_time: SYSTEMTIME) -> std::time::Duration {
    std::time::Duration::from_secs(
      system_time.wHour as u64 * 3600
        + system_time.wMinute as u64 * 60
        + system_time.wSecond as u64,
    ) + std::time::Duration::from_millis(system_time.wMilliseconds as u64)
  }

  let mut creation_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut exit_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut kernel_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut user_time = std::mem::MaybeUninit::<FILETIME>::uninit();

  // SAFETY: winapi calls
  let ret = unsafe {
    GetProcessTimes(
      GetCurrentProcess(),
      creation_time.as_mut_ptr(),
      exit_time.as_mut_ptr(),
      kernel_time.as_mut_ptr(),
      user_time.as_mut_ptr(),
    )
  };

  if ret != TRUE {
    return std::default::Default::default();
  }

  let mut kernel_system_time = std::mem::MaybeUninit::<SYSTEMTIME>::uninit();
  let mut user_system_time = std::mem::MaybeUninit::<SYSTEMTIME>::uninit();

  // SAFETY: convert to system time
  unsafe {
    let sys_ret = FileTimeToSystemTime(
      kernel_time.assume_init_mut(),
      kernel_system_time.as_mut_ptr(),
    );
    let user_ret = FileTimeToSystemTime(
      user_time.assume_init_mut(),
      user_system_time.as_mut_ptr(),
    );

    match (sys_ret, user_ret) {
      (TRUE, TRUE) => (
        convert_system_time(kernel_system_time.assume_init()),
        convert_system_time(user_system_time.assume_init()),
      ),
      (TRUE, FALSE) => (
        convert_system_time(kernel_system_time.assume_init()),
        Default::default(),
      ),
      (FALSE, TRUE) => (
        Default::default(),
        convert_system_time(user_system_time.assume_init()),
      ),
      (_, _) => Default::default(),
    }
  }
}

#[cfg(not(any(windows, unix)))]
fn get_cpu_usage() -> (std::time::Duration, std::time::Duration) {
  Default::default()
}

#[op2]
#[serde]
fn op_runtime_memory_usage(
  scope: &mut v8::HandleScope,
) -> (usize, usize, usize, usize) {
  let s = scope.get_heap_statistics();

  let (rss, heap_total, heap_used, external) = (
    rss(),
    s.total_heap_size(),
    s.used_heap_size(),
    s.external_memory(),
  );

  (rss, heap_total, heap_used, external)
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
    extern "C" {
      static mut mach_task_self_: std::ffi::c_uint;
    }
    libc::task_info(
      mach_task_self_,
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

fn os_uptime(state: &mut OpState) -> Result<u64, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("osUptime", "Deno.osUptime()")?;
  Ok(sys_info::os_uptime())
}

#[op2(fast, stack_trace)]
#[number]
fn op_os_uptime(state: &mut OpState) -> Result<u64, PermissionCheckError> {
  os_uptime(state)
}
