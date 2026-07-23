// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
#[cfg(unix)]
use nix::unistd::Gid;
#[cfg(unix)]
use nix::unistd::Group;
#[cfg(unix)]
use nix::unistd::Uid;
#[cfg(unix)]
use nix::unistd::User;

use crate::ExtNodeSys;

// --- process.title support ---
//
// The argv buffer overwrite technique used here is the standard approach for
// setting the process title visible in `ps`. This is the same technique used by
// Node.js (via libuv's uv_setup_args/uv_set_process_title), nginx, PostgreSQL,
// and many other programs. The OS allocates argv as a contiguous buffer; we
// save its bounds at startup, then overwrite it with the new title.
//
// A mutex guards the write path so concurrent calls from workers don't race
// on the shared argv buffer (matching libuv which also uses a mutex).
//
// References:
// - libuv: https://github.com/libuv/libuv/blob/v1.x/src/unix/proctitle.c
// - Node.js: uses uv_setup_args() in node_main.cc, uv_set_process_title() in node.cc

#[cfg(unix)]
struct ArgvInfo {
  /// Pointer to argv[0] — the start of the contiguous buffer.
  buf_ptr: *mut u8,
  /// Total size of the contiguous argv buffer (argv[0] through end of argv[argc-1]).
  buf_size: usize,
}

// SAFETY: The raw pointer in ArgvInfo points to the process argv buffer which
// is valid for the entire process lifetime and is only accessed under ARGV_MUTEX.
#[cfg(unix)]
unsafe impl Send for ArgvInfo {}
// SAFETY: The raw pointer in ArgvInfo points to the process argv buffer which
// is valid for the entire process lifetime and is only accessed under ARGV_MUTEX.
#[cfg(unix)]
unsafe impl Sync for ArgvInfo {}

/// Saved argv buffer bounds. Initialized once at startup (via .init_array on
/// Linux, or lazily via _NSGetArgv on macOS). Using OnceLock ensures the bounds
/// are captured before any title-set zeroes the buffer.
#[cfg(unix)]
static ARGV_INFO: std::sync::OnceLock<ArgvInfo> = std::sync::OnceLock::new();

/// Mutex guarding argv buffer writes so concurrent workers don't race.
#[cfg(unix)]
static ARGV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
fn overwrite_argv_buffer(title: &str) {
  let Some(info) = ARGV_INFO.get() else {
    return;
  };
  if info.buf_ptr.is_null() || info.buf_size == 0 {
    return;
  }
  let _guard = ARGV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
  // SAFETY: buf_ptr and buf_size were captured once from the OS-provided argv
  // buffer and remain valid for the process lifetime. The mutex ensures
  // exclusive access.
  unsafe {
    let buf = std::slice::from_raw_parts_mut(info.buf_ptr, info.buf_size);
    let title_bytes = title.as_bytes();
    let copy_len = title_bytes.len().min(info.buf_size - 1);
    buf[..copy_len].copy_from_slice(&title_bytes[..copy_len]);
    buf[copy_len..].fill(0);
  }
}

/// Compute the contiguous argv buffer bounds from argv[0]..argv[argc-1].
///
/// # Safety
/// `argv` must be a valid pointer to `argc` C string pointers.
#[cfg(unix)]
unsafe fn compute_argv_info(
  argc: usize,
  argv: *mut *mut libc::c_char,
) -> ArgvInfo {
  // SAFETY: argv is valid and has argc entries (guaranteed by caller).
  unsafe {
    let start = *argv as *mut _;
    let last_arg = *argv.add(argc - 1);
    let last_arg_len = libc::strlen(last_arg);
    let end = last_arg.add(last_arg_len + 1) as *const u8;
    let buf_size = end.offset_from(start) as usize;
    ArgvInfo {
      buf_ptr: start,
      buf_size,
    }
  }
}

#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".init_array")]
static ARGV_INIT_FN: unsafe extern "C" fn(
  libc::c_int,
  *mut *mut libc::c_char,
  *mut *mut libc::c_char,
) = {
  unsafe extern "C" fn init(
    argc: libc::c_int,
    argv: *mut *mut libc::c_char,
    _envp: *mut *mut libc::c_char,
  ) {
    if argv.is_null() || argc <= 0 {
      return;
    }
    // SAFETY: argc and argv are provided by the OS at process init and are valid.
    let info = unsafe { compute_argv_info(argc as usize, argv) };
    let _ = ARGV_INFO.set(info);
  }
  init
};

#[cfg(target_os = "macos")]
fn init_macos_argv_info() {
  ARGV_INFO.get_or_init(|| {
    // SAFETY: _NSGetArgc/_NSGetArgv are stable macOS APIs that return
    // pointers to the process's argc/argv, valid for the process lifetime.
    unsafe {
      unsafe extern "C" {
        fn _NSGetArgc() -> *mut libc::c_int;
        fn _NSGetArgv() -> *mut *mut *mut libc::c_char;
      }

      let argc = *_NSGetArgc() as usize;
      let argv = *_NSGetArgv();
      if argv.is_null() || argc == 0 {
        return ArgvInfo {
          buf_ptr: std::ptr::null_mut(),
          buf_size: 0,
        };
      }
      compute_argv_info(argc, argv)
    }
  });
}

#[cfg(target_os = "macos")]
fn set_process_title(title: &str) {
  init_macos_argv_info();
  overwrite_argv_buffer(title);

  // Also set the pthread name (visible in Activity Monitor / debugger, 63 char limit)
  let truncated = &title.as_bytes()[..title.len().min(63)];
  if let Ok(c_title) = std::ffi::CString::new(truncated) {
    // SAFETY: c_title is a valid null-terminated C string.
    unsafe {
      libc::pthread_setname_np(c_title.as_ptr());
    }
  }
}

#[cfg(target_os = "linux")]
fn set_process_title(title: &str) {
  overwrite_argv_buffer(title);

  // Also set the kernel thread name via prctl (15 char limit, visible in /proc/self/comm)
  let truncated = &title.as_bytes()[..title.len().min(15)];
  if let Ok(c_title) = std::ffi::CString::new(truncated) {
    // SAFETY: c_title is a valid null-terminated C string.
    unsafe {
      libc::prctl(libc::PR_SET_NAME, c_title.as_ptr() as libc::c_ulong);
    }
  }
}

#[cfg(target_os = "windows")]
fn set_process_title(title: &str) {
  let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
  // SAFETY: FFI call, wide is null-terminated
  unsafe {
    windows_sys::Win32::System::Console::SetConsoleTitleW(wide.as_ptr());
  }
}

#[cfg(not(any(
  target_os = "macos",
  target_os = "linux",
  target_os = "windows"
)))]
fn set_process_title(_title: &str) {
  // No-op on unsupported platforms
}

#[op2(fast)]
pub fn op_node_process_set_title(#[string] title: &str) {
  set_process_title(title);
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ProcessError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(
    #[from]
    #[inherit]
    PermissionCheckError,
  ),
  #[class(generic)]
  #[error("{0} identifier does not exist: {1}")]
  #[property("code" = "ERR_UNKNOWN_CREDENTIAL")]
  UnknownCredential(String, String),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error("Operation not supported on this platform")]
  NotSupported,
  #[class(type)]
  #[error("Invalid {0} parameter")]
  InvalidParam(String),
}

#[cfg(unix)]
impl From<nix::Error> for ProcessError {
  fn from(err: nix::Error) -> Self {
    ProcessError::Io(std::io::Error::from_raw_os_error(err as i32))
  }
}

#[cfg(unix)]
fn kill(pid: i32, sig: i32) -> i32 {
  // SAFETY: FFI call to libc
  if unsafe { libc::kill(pid, sig) } < 0 {
    std::io::Error::last_os_error().raw_os_error().unwrap()
  } else {
    0
  }
}

#[cfg(not(unix))]
fn kill(pid: i32, _sig: i32) -> i32 {
  match deno_subprocess_windows::process_kill(pid, _sig) {
    Ok(_) => 0,
    Err(e) => e.as_uv_error(),
  }
}

#[op2(fast, stack_trace)]
pub fn op_node_process_kill(
  state: &mut OpState,
  #[smi] pid: i32,
  #[smi] sig: i32,
) -> Result<i32, deno_permissions::PermissionCheckError> {
  if pid != std::process::id() as i32 {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_run_all("process.kill")?;
  }
  Ok(kill(pid, sig))
}

#[op2(fast)]
pub fn op_process_abort() {
  std::process::abort();
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
enum Id {
  Number(u32),
  Name(String),
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn get_group_id(name: &str) -> Result<Gid, ProcessError> {
  let group = Group::from_name(name)?;

  if let Some(group) = group {
    Ok(group.gid)
  } else {
    Err(ProcessError::UnknownCredential(
      "Group".to_string(),
      name.to_string(),
    ))
  }
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn serialize_id<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
) -> Result<Id, ProcessError> {
  if value.is_number() {
    let num = value.uint32_value(scope).unwrap();
    return Ok(Id::Number(num));
  }

  if value.is_string() {
    let name = value.to_string(scope).unwrap();
    return Ok(Id::Name(name.to_rust_string_lossy(scope)));
  }

  Err(ProcessError::InvalidParam("id".to_string()))
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setegid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setegid", "node:process.setegid")?;
  }

  let gid = match serialize_id(scope, id)? {
    Id::Number(number) => Gid::from_raw(number),
    Id::Name(name) => get_group_id(&name)?,
  };

  nix::unistd::setegid(gid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setegid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn get_user_id(name: &str) -> Result<Uid, ProcessError> {
  let user = User::from_name(name)?;

  if let Some(user) = user {
    Ok(user.uid)
  } else {
    Err(ProcessError::UnknownCredential(
      "User".to_string(),
      name.to_string(),
    ))
  }
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_seteuid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("seteuid", "node:process.seteuid")?;
  }

  let uid = match serialize_id(scope, id)? {
    Id::Number(number) => Uid::from_raw(number),
    Id::Name(name) => get_user_id(&name)?,
  };

  nix::unistd::seteuid(uid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_seteuid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setgid", "node:process.setgid")?;
  }

  let gid = match serialize_id(scope, id)? {
    Id::Number(number) => Gid::from_raw(number),
    Id::Name(name) => get_group_id(&name)?,
  };

  nix::unistd::setgid(gid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setuid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setuid", "node:process.setuid")?;
  }

  let uid = match serialize_id(scope, id)? {
    Id::Number(number) => Uid::from_raw(number),
    Id::Name(name) => get_user_id(&name)?,
  };

  nix::unistd::setuid(uid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setuid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgroups<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  groups: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setgroups", "node:process.setgroups")?;
  }

  if !groups.is_array() {
    return Err(ProcessError::InvalidParam("groups".to_string()));
  }

  let arr = v8::Local::<v8::Array>::try_from(groups)
    .map_err(|_| ProcessError::InvalidParam("groups".to_string()))?;
  let len = arr.length();
  let mut gids: Vec<Gid> = Vec::with_capacity(len as usize);

  for i in 0..len {
    let elem = arr
      .get_index(scope, i)
      .ok_or_else(|| ProcessError::InvalidParam("groups".to_string()))?;
    let gid = match serialize_id(scope, elem)? {
      Id::Number(n) => Gid::from_raw(n),
      Id::Name(name) => get_group_id(&name)?,
    };
    gids.push(gid);
  }

  let raw: Vec<libc::gid_t> = gids.iter().map(|g| g.as_raw()).collect();
  // SAFETY: raw holds valid gid_t values; pointer is valid for the call duration.
  if unsafe { libc::setgroups(raw.len() as _, raw.as_ptr()) } != 0 {
    return Err(std::io::Error::last_os_error().into());
  }

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgroups(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _groups: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

/// Fills `out` with the 16 fields of `process.resourceUsage()`, in the same
/// order as the returned object:
///
/// 0 userCPUTime, 1 systemCPUTime, 2 maxRSS, 3 sharedMemorySize,
/// 4 unsharedDataSize, 5 unsharedStackSize, 6 minorPageFault, 7 majorPageFault,
/// 8 swappedOut, 9 fsRead, 10 fsWrite, 11 ipcSent, 12 ipcReceived,
/// 13 signalsCount, 14 voluntaryContextSwitches, 15 involuntaryContextSwitches.
///
/// CPU times are in microseconds. This mirrors libuv's `uv_getrusage`, which
/// Node.js uses: on Unix the values come straight from `getrusage(2)`, and on
/// platforms that don't provide a field it is reported as `0`.
#[op2(fast)]
pub fn op_node_process_resource_usage(#[buffer] out: &mut [f64]) {
  if out.len() < 16 {
    return;
  }
  let usage = get_resource_usage();
  out[..16].copy_from_slice(&usage);
}

#[cfg(unix)]
fn get_resource_usage() -> [f64; 16] {
  let mut rusage = std::mem::MaybeUninit::uninit();

  // SAFETY: libc call, rusage is initialized on success.
  let ret = unsafe { libc::getrusage(libc::RUSAGE_SELF, rusage.as_mut_ptr()) };
  if ret != 0 {
    return [0.0; 16];
  }

  // SAFETY: getrusage returned 0, so rusage is initialized.
  let r = unsafe { rusage.assume_init() };

  let micros = |tv: libc::timeval| -> f64 {
    (tv.tv_sec as f64) * 1_000_000.0 + (tv.tv_usec as f64)
  };

  [
    micros(r.ru_utime),   // userCPUTime
    micros(r.ru_stime),   // systemCPUTime
    r.ru_maxrss as f64,   // maxRSS
    r.ru_ixrss as f64,    // sharedMemorySize
    r.ru_idrss as f64,    // unsharedDataSize
    r.ru_isrss as f64,    // unsharedStackSize
    r.ru_minflt as f64,   // minorPageFault
    r.ru_majflt as f64,   // majorPageFault
    r.ru_nswap as f64,    // swappedOut
    r.ru_inblock as f64,  // fsRead
    r.ru_oublock as f64,  // fsWrite
    r.ru_msgsnd as f64,   // ipcSent
    r.ru_msgrcv as f64,   // ipcReceived
    r.ru_nsignals as f64, // signalsCount
    r.ru_nvcsw as f64,    // voluntaryContextSwitches
    r.ru_nivcsw as f64,   // involuntaryContextSwitches
  ]
}

#[cfg(windows)]
fn get_resource_usage() -> [f64; 16] {
  use windows_sys::Win32::Foundation::FILETIME;
  use windows_sys::Win32::System::ProcessStatus::GetProcessMemoryInfo;
  use windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS;
  use windows_sys::Win32::System::Threading::GetCurrentProcess;
  use windows_sys::Win32::System::Threading::GetProcessTimes;

  let mut usage = [0.0f64; 16];

  // CPU times via GetProcessTimes. FILETIME is in 100-nanosecond intervals.
  let filetime_micros = |ft: FILETIME| -> f64 {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    (ticks as f64) / 10.0
  };

  let mut creation_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut exit_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut kernel_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut user_time = std::mem::MaybeUninit::<FILETIME>::uninit();

  // SAFETY: Win32 call with valid out-pointers.
  let ret = unsafe {
    GetProcessTimes(
      GetCurrentProcess(),
      creation_time.as_mut_ptr(),
      exit_time.as_mut_ptr(),
      kernel_time.as_mut_ptr(),
      user_time.as_mut_ptr(),
    )
  };
  if ret != 0 {
    // SAFETY: GetProcessTimes succeeded, both are initialized.
    unsafe {
      usage[0] = filetime_micros(user_time.assume_init()); // userCPUTime
      usage[1] = filetime_micros(kernel_time.assume_init()); // systemCPUTime
    }
  }

  // Memory counters via GetProcessMemoryInfo, matching libuv's uv_getrusage.
  let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::uninit();
  // SAFETY: Win32 call with a properly sized PROCESS_MEMORY_COUNTERS buffer.
  let mem_ret = unsafe {
    GetProcessMemoryInfo(
      GetCurrentProcess(),
      counters.as_mut_ptr(),
      std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
    )
  };
  if mem_ret != 0 {
    // SAFETY: GetProcessMemoryInfo succeeded, so counters is initialized.
    let counters = unsafe { counters.assume_init() };
    usage[2] = (counters.PeakWorkingSetSize / 1024) as f64; // maxRSS (KB)
    usage[7] = counters.PageFaultCount as f64; // majorPageFault
  }

  usage
}

#[cfg(not(any(unix, windows)))]
fn get_resource_usage() -> [f64; 16] {
  [0.0; 16]
}

/// Returns the cgroup-constrained memory limit, or 0 if unconstrained.
/// This matches Node.js `process.constrainedMemory()` semantics.
#[op2(fast)]
#[number]
pub fn op_node_process_constrained_memory<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
) -> u64 {
  #[cfg(any(target_os = "android", target_os = "linux"))]
  {
    let sys = state.borrow::<TSys>();
    cgroup::cgroup_memory_limit(sys).unwrap_or(0)
  }
  #[cfg(not(any(target_os = "android", target_os = "linux")))]
  {
    let _ = state;
    0
  }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod cgroup {
  pub enum CgroupVersion<'a> {
    V1 { cgroup_relpath: &'a str },
    V2 { cgroup_relpath: &'a str },
    None,
  }

  pub fn parse_self_cgroup(self_cgroup_content: &str) -> CgroupVersion<'_> {
    let mut cgroup_version = CgroupVersion::None;

    for line in self_cgroup_content.lines() {
      let split = line.split(":").collect::<Vec<_>>();

      match &split[..] {
        // cgroup v1 memory controller — takes priority, break immediately
        [_, "memory", cgroup_v1_relpath] => {
          cgroup_version = CgroupVersion::V1 {
            cgroup_relpath: cgroup_v1_relpath
              .strip_prefix("/")
              .unwrap_or(cgroup_v1_relpath),
          };
          break;
        }
        // cgroup v2 (but keep looking for v1 memory in hybrid mode)
        ["0", "", cgroup_v2_relpath] => {
          cgroup_version = CgroupVersion::V2 {
            cgroup_relpath: cgroup_v2_relpath
              .strip_prefix("/")
              .unwrap_or(cgroup_v2_relpath),
          };
        }
        _ => {}
      }
    }

    cgroup_version
  }

  /// Read the cgroup memory limit from the filesystem.
  /// Returns `None` if cgroup info cannot be read or parsed.
  pub fn cgroup_memory_limit<TSys: sys_traits::FsRead>(
    sys: &TSys,
  ) -> Option<u64> {
    let self_cgroup = sys.fs_read_to_string("/proc/self/cgroup").ok()?;

    match parse_self_cgroup(&self_cgroup) {
      CgroupVersion::V1 { cgroup_relpath } => {
        let limit_path = std::path::Path::new("/sys/fs/cgroup/memory")
          .join(cgroup_relpath)
          .join("memory.limit_in_bytes");
        sys
          .fs_read_to_string(limit_path)
          .ok()
          .and_then(|s| s.trim().parse::<u64>().ok())
      }
      CgroupVersion::V2 { cgroup_relpath } => {
        let limit_path = std::path::Path::new("/sys/fs/cgroup")
          .join(cgroup_relpath)
          .join("memory.max");
        sys
          .fs_read_to_string(limit_path)
          .ok()
          .and_then(|s| s.trim().parse::<u64>().ok())
      }
      CgroupVersion::None => None,
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn test_parse_self_cgroup_v2() {
      let self_cgroup = "0::/user.slice/user-1000.slice/session-3.scope";
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V2 { cgroup_relpath } if cgroup_relpath == "user.slice/user-1000.slice/session-3.scope"
      ));
    }

    #[test]
    fn test_parse_self_cgroup_hybrid() {
      let self_cgroup = r#"12:rdma:/
11:blkio:/user.slice
10:devices:/user.slice
9:cpu,cpuacct:/user.slice
8:pids:/user.slice/user-1000.slice/session-3.scope
7:memory:/user.slice/user-1000.slice/session-3.scope
6:perf_event:/
5:freezer:/
4:net_cls,net_prio:/
3:hugetlb:/
2:cpuset:/
1:name=systemd:/user.slice/user-1000.slice/session-3.scope
0::/user.slice/user-1000.slice/session-3.scope
"#;
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V1 { cgroup_relpath } if cgroup_relpath == "user.slice/user-1000.slice/session-3.scope"
      ));
    }

    #[test]
    fn test_parse_self_cgroup_v1() {
      let self_cgroup = r#"11:hugetlb:/
10:pids:/user.slice/user-1000.slice
9:perf_event:/
8:devices:/user.slice
7:net_cls,net_prio:/
6:memory:/
5:blkio:/
4:cpuset:/
3:cpu,cpuacct:/
2:freezer:/
1:name=systemd:/user.slice/user-1000.slice/session-2.scope
"#;
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V1 { cgroup_relpath } if cgroup_relpath.is_empty()
      ));
    }
  }
}
