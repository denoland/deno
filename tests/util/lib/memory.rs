// Copyright 2018-2026 the Deno authors. MIT license.

/// Minimal memory info for monitoring available system memory.
pub struct MemInfo {
  pub total: u64,
  pub available: u64,
}

/// Returns current system memory info, or None if unavailable.
/// Adapted from ext/os/sys_info.rs.
pub fn mem_info() -> Option<MemInfo> {
  mem_info_impl()
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn mem_info_impl() -> Option<MemInfo> {
  let mut total = 0u64;
  let mut available = 0u64;

  // try libc::sysinfo first for total memory
  let mut info = std::mem::MaybeUninit::uninit();
  // SAFETY: `info` is a valid pointer to a `libc::sysinfo` struct.
  let res = unsafe { libc::sysinfo(info.as_mut_ptr()) };
  if res == 0 {
    // SAFETY: `sysinfo` initializes the struct.
    let info = unsafe { info.assume_init() };
    let mem_unit = info.mem_unit as u64;
    total = info.totalram * mem_unit;
    available = info.freeram * mem_unit;
  }

  // /proc/meminfo has more accurate MemAvailable
  #[allow(clippy::disallowed_methods)]
  if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
    for line in meminfo.lines() {
      if line.starts_with("MemTotal:")
        && let Some(kb) = line.split_whitespace().nth(1)
        && let Ok(kb) = kb.parse::<u64>()
      {
        total = kb * 1024;
      } else if line.starts_with("MemAvailable:")
        && let Some(kb) = line.split_whitespace().nth(1)
        && let Ok(kb) = kb.parse::<u64>()
      {
        available = kb * 1024;
      }
    }
  }

  if total > 0 {
    Some(MemInfo { total, available })
  } else {
    None
  }
}

#[cfg(target_vendor = "apple")]
fn mem_info_impl() -> Option<MemInfo> {
  // SAFETY: all calls use valid pointers and check return values.
  unsafe {
    let mut total = 0u64;
    let mut mib: [i32; 2] = [libc::CTL_HW, libc::HW_MEMSIZE];
    let mut size = std::mem::size_of::<u64>();
    libc::sysctl(
      mib.as_mut_ptr(),
      2,
      &mut total as *mut _ as *mut libc::c_void,
      &mut size,
      std::ptr::null_mut(),
      0,
    );

    if total == 0 {
      return None;
    }

    unsafe extern "C" {
      fn mach_host_self() -> std::ffi::c_uint;
    }

    let mut count: u32 = libc::HOST_VM_INFO64_COUNT as _;
    let mut stat = std::mem::zeroed::<libc::vm_statistics64>();
    let available = if libc::host_statistics64(
      mach_host_self(),
      libc::HOST_VM_INFO64,
      &mut stat as *mut libc::vm_statistics64 as *mut _,
      &mut count,
    ) == libc::KERN_SUCCESS
    {
      let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;
      (stat.free_count as u64 + stat.inactive_count as u64) * page_size
    } else {
      0
    };

    Some(MemInfo { total, available })
  }
}

#[cfg(target_family = "windows")]
fn mem_info_impl() -> Option<MemInfo> {
  // SAFETY: `mem_status` is properly initialized with dwLength set.
  unsafe {
    use winapi::shared::minwindef;
    use winapi::um::sysinfoapi;

    let mut mem_status =
      std::mem::MaybeUninit::<sysinfoapi::MEMORYSTATUSEX>::uninit();
    (*mem_status.as_mut_ptr()).dwLength =
      std::mem::size_of::<sysinfoapi::MEMORYSTATUSEX>() as minwindef::DWORD;

    if sysinfoapi::GlobalMemoryStatusEx(mem_status.as_mut_ptr()) != 0 {
      let stat = mem_status.assume_init();
      Some(MemInfo {
        total: stat.ullTotalPhys,
        available: stat.ullAvailPhys,
      })
    } else {
      None
    }
  }
}

#[cfg(not(any(
  target_os = "android",
  target_os = "linux",
  target_vendor = "apple",
  target_family = "windows",
)))]
fn mem_info_impl() -> Option<MemInfo> {
  None
}
