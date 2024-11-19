// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
#[cfg(target_family = "windows")]
use std::sync::Once;

type LoadAvg = (f64, f64, f64);
const DEFAULT_LOADAVG: LoadAvg = (0.0, 0.0, 0.0);

pub fn loadavg() -> LoadAvg {
  #[cfg(any(target_os = "android", target_os = "linux"))]
  {
    use libc::SI_LOAD_SHIFT;

    let mut info = std::mem::MaybeUninit::uninit();
    // SAFETY: `info` is a valid pointer to a `libc::sysinfo` struct.
    let res = unsafe { libc::sysinfo(info.as_mut_ptr()) };
    if res == 0 {
      // SAFETY: `sysinfo` returns 0 on success, and `info` is initialized.
      let info = unsafe { info.assume_init() };
      (
        info.loads[0] as f64 / (1 << SI_LOAD_SHIFT) as f64,
        info.loads[1] as f64 / (1 << SI_LOAD_SHIFT) as f64,
        info.loads[2] as f64 / (1 << SI_LOAD_SHIFT) as f64,
      )
    } else {
      DEFAULT_LOADAVG
    }
  }
  #[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  {
    let mut l: [f64; 3] = [0.; 3];
    // SAFETY: `&mut l` is a valid pointer to an array of 3 doubles
    if unsafe { libc::getloadavg(&mut l as *mut f64, l.len() as _) } < 3 {
      DEFAULT_LOADAVG
    } else {
      (l[0], l[1], l[2])
    }
  }
  #[cfg(target_os = "windows")]
  {
    DEFAULT_LOADAVG
  }
}

pub fn os_release() -> String {
  #[cfg(target_os = "linux")]
  {
    #[allow(clippy::disallowed_methods)]
    match std::fs::read_to_string("/proc/sys/kernel/osrelease") {
      Ok(mut s) => {
        s.pop(); // pop '\n'
        s
      }
      _ => String::from(""),
    }
  }
  #[cfg(target_os = "android")]
  {
    let mut info = std::mem::MaybeUninit::uninit();
    // SAFETY: `info` is a valid pointer to a `libc::utsname` struct.
    let res = unsafe { libc::uname(info.as_mut_ptr()) };
    if res != 0 {
      return String::from("");
    }
    // SAFETY: `uname` returns 0 on success, and `info` is initialized.
    let mut info = unsafe { info.assume_init() };
    let len = info.release.len();
    info.release[len - 1] = 0;
    // SAFETY: `info.release` is a valid pointer and NUL-terminated.
    let c_str = unsafe { std::ffi::CStr::from_ptr(info.release.as_ptr()) };
    c_str.to_string_lossy().into_owned()
  }
  #[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  {
    let mut s = [0u8; 256];
    let mut mib = [libc::CTL_KERN, libc::KERN_OSRELEASE];
    // 256 is enough.
    let mut len = s.len();
    // SAFETY: `sysctl` is thread-safe.
    // `s` is only accessed if sysctl() succeeds and agrees with the `len` set
    // by sysctl().
    if unsafe {
      libc::sysctl(
        mib.as_mut_ptr(),
        mib.len() as _,
        s.as_mut_ptr() as _,
        &mut len,
        std::ptr::null_mut(),
        0,
      )
    } == -1
    {
      return String::from("Unknown");
    }

    // without the NUL terminator
    return String::from_utf8_lossy(&s[..len - 1]).to_string();
  }
  #[cfg(target_family = "windows")]
  {
    use ntapi::ntrtl::RtlGetVersion;
    use winapi::shared::ntdef::NT_SUCCESS;
    use winapi::um::winnt::RTL_OSVERSIONINFOEXW;

    let mut version_info =
      std::mem::MaybeUninit::<RTL_OSVERSIONINFOEXW>::uninit();
    // SAFETY: we need to initialize dwOSVersionInfoSize.
    unsafe {
      (*version_info.as_mut_ptr()).dwOSVersionInfoSize =
        std::mem::size_of::<RTL_OSVERSIONINFOEXW>() as u32;
    }
    // SAFETY: `version_info` is pointer to a valid `RTL_OSVERSIONINFOEXW` struct and
    // dwOSVersionInfoSize  is set to the size of RTL_OSVERSIONINFOEXW.
    if !NT_SUCCESS(unsafe {
      RtlGetVersion(version_info.as_mut_ptr() as *mut _)
    }) {
      String::from("")
    } else {
      // SAFETY: we assume that RtlGetVersion() initializes the fields.
      let version_info = unsafe { version_info.assume_init() };
      format!(
        "{}.{}.{}",
        version_info.dwMajorVersion,
        version_info.dwMinorVersion,
        version_info.dwBuildNumber
      )
    }
  }
}

#[cfg(target_family = "windows")]
static WINSOCKET_INIT: Once = Once::new();

pub fn hostname() -> String {
  #[cfg(target_family = "unix")]
  // SAFETY: `sysconf` returns a system constant.
  unsafe {
    let buf_size = libc::sysconf(libc::_SC_HOST_NAME_MAX) as usize;
    let mut buf = vec![0u8; buf_size + 1];
    let len = buf.len();
    if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, len) < 0 {
      return String::from("");
    }
    // ensure NUL termination
    buf[len - 1] = 0;
    std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char)
      .to_string_lossy()
      .to_string()
  }
  #[cfg(target_family = "windows")]
  {
    use std::ffi::OsString;
    use std::mem;
    use std::os::windows::ffi::OsStringExt;
    use winapi::shared::minwindef::MAKEWORD;
    use winapi::um::winsock2::GetHostNameW;
    use winapi::um::winsock2::WSAStartup;

    let namelen = 256;
    let mut name: Vec<u16> = vec![0u16; namelen];
    // Start winsock to make `GetHostNameW` work correctly
    // https://github.com/retep998/winapi-rs/issues/296
    // SAFETY: winapi call
    WINSOCKET_INIT.call_once(|| unsafe {
      let mut data = mem::zeroed();
      let wsa_startup_result = WSAStartup(MAKEWORD(2, 2), &mut data);
      if wsa_startup_result != 0 {
        panic!("Failed to start winsocket");
      }
    });
    let err =
      // SAFETY: length of wide string is 256 chars or less.
      // https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-gethostnamew
      unsafe { GetHostNameW(name.as_mut_ptr(), namelen as libc::c_int) };

    if err == 0 {
      // TODO(@littledivy): Probably not the most efficient way.
      let len = name.iter().take_while(|&&c| c != 0).count();
      OsString::from_wide(&name[..len])
        .to_string_lossy()
        .into_owned()
    } else {
      String::from("")
    }
  }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemInfo {
  pub total: u64,
  pub free: u64,
  pub available: u64,
  pub buffers: u64,
  pub cached: u64,
  pub swap_total: u64,
  pub swap_free: u64,
}

pub fn mem_info() -> Option<MemInfo> {
  let mut mem_info = MemInfo {
    total: 0,
    free: 0,
    available: 0,
    buffers: 0,
    cached: 0,
    swap_total: 0,
    swap_free: 0,
  };
  #[cfg(any(target_os = "android", target_os = "linux"))]
  {
    let mut info = std::mem::MaybeUninit::uninit();
    // SAFETY: `info` is a valid pointer to a `libc::sysinfo` struct.
    let res = unsafe { libc::sysinfo(info.as_mut_ptr()) };
    if res == 0 {
      // SAFETY: `sysinfo` initializes the struct.
      let info = unsafe { info.assume_init() };
      let mem_unit = info.mem_unit as u64;
      mem_info.swap_total = info.totalswap * mem_unit;
      mem_info.swap_free = info.freeswap * mem_unit;
      mem_info.total = info.totalram * mem_unit;
      mem_info.free = info.freeram * mem_unit;
      mem_info.available = mem_info.free;
      mem_info.buffers = info.bufferram * mem_unit;
    }

    // Gets the available memory from /proc/meminfo in linux for compatibility
    #[allow(clippy::disallowed_methods)]
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
      let line = meminfo.lines().find(|l| l.starts_with("MemAvailable:"));
      if let Some(line) = line {
        let mem = line.split_whitespace().nth(1);
        let mem = mem.and_then(|v| v.parse::<u64>().ok());
        mem_info.available = mem.unwrap_or(0) * 1024;
      }
    }
  }
  #[cfg(target_vendor = "apple")]
  {
    let mut mib: [i32; 2] = [0, 0];
    mib[0] = libc::CTL_HW;
    mib[1] = libc::HW_MEMSIZE;
    // SAFETY:
    //  - We assume that `mach_host_self` always returns a valid value.
    //  - sysconf returns a system constant.
    unsafe {
      let mut size = std::mem::size_of::<u64>();
      libc::sysctl(
        mib.as_mut_ptr(),
        mib.len() as _,
        &mut mem_info.total as *mut _ as *mut libc::c_void,
        &mut size,
        std::ptr::null_mut(),
        0,
      );

      let mut xs: libc::xsw_usage = std::mem::zeroed::<libc::xsw_usage>();
      mib[0] = libc::CTL_VM;
      mib[1] = libc::VM_SWAPUSAGE;

      let mut size = std::mem::size_of::<libc::xsw_usage>();
      libc::sysctl(
        mib.as_mut_ptr(),
        mib.len() as _,
        &mut xs as *mut _ as *mut libc::c_void,
        &mut size,
        std::ptr::null_mut(),
        0,
      );

      mem_info.swap_total = xs.xsu_total;
      mem_info.swap_free = xs.xsu_avail;

      let mut count: u32 = libc::HOST_VM_INFO64_COUNT as _;
      let mut stat = std::mem::zeroed::<libc::vm_statistics64>();
      if libc::host_statistics64(
        // TODO(@littledivy): Put this in a once_cell.
        libc::mach_host_self(),
        libc::HOST_VM_INFO64,
        &mut stat as *mut libc::vm_statistics64 as *mut _,
        &mut count,
      ) == libc::KERN_SUCCESS
      {
        // TODO(@littledivy): Put this in a once_cell
        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;
        mem_info.available =
          (stat.free_count as u64 + stat.inactive_count as u64) * page_size
            / 1024;
        mem_info.free =
          (stat.free_count as u64 - stat.speculative_count as u64) * page_size
            / 1024;
      }
    }
  }
  #[cfg(target_family = "windows")]
  // SAFETY:
  //   - `mem_status` is a valid pointer to a `libc::MEMORYSTATUSEX` struct.
  //   - `dwLength` is set to the size of the struct.
  unsafe {
    use std::mem;
    use winapi::shared::minwindef;
    use winapi::um::psapi::GetPerformanceInfo;
    use winapi::um::psapi::PERFORMANCE_INFORMATION;
    use winapi::um::sysinfoapi;

    let mut mem_status =
      mem::MaybeUninit::<sysinfoapi::MEMORYSTATUSEX>::uninit();
    let length =
      mem::size_of::<sysinfoapi::MEMORYSTATUSEX>() as minwindef::DWORD;
    (*mem_status.as_mut_ptr()).dwLength = length;

    let result = sysinfoapi::GlobalMemoryStatusEx(mem_status.as_mut_ptr());
    if result != 0 {
      let stat = mem_status.assume_init();
      mem_info.total = stat.ullTotalPhys;
      mem_info.available = 0;
      mem_info.free = stat.ullAvailPhys;
      mem_info.cached = 0;
      mem_info.buffers = 0;

      // `stat.ullTotalPageFile` is reliable only from GetPerformanceInfo()
      //
      // See https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex
      // and https://github.com/GuillaumeGomez/sysinfo/issues/534

      let mut perf_info = mem::MaybeUninit::<PERFORMANCE_INFORMATION>::uninit();
      let result = GetPerformanceInfo(
        perf_info.as_mut_ptr(),
        mem::size_of::<PERFORMANCE_INFORMATION>() as minwindef::DWORD,
      );
      if result == minwindef::TRUE {
        let perf_info = perf_info.assume_init();
        let swap_total = perf_info.PageSize
          * perf_info
            .CommitLimit
            .saturating_sub(perf_info.PhysicalTotal);
        let swap_free = perf_info.PageSize
          * perf_info
            .CommitLimit
            .saturating_sub(perf_info.PhysicalTotal)
            .saturating_sub(perf_info.PhysicalAvailable);
        mem_info.swap_total = (swap_total / 1000) as u64;
        mem_info.swap_free = (swap_free / 1000) as u64;
      }
    }
  }

  Some(mem_info)
}

pub fn os_uptime() -> u64 {
  let uptime: u64;

  #[cfg(any(target_os = "android", target_os = "linux"))]
  {
    let mut info = std::mem::MaybeUninit::uninit();
    // SAFETY: `info` is a valid pointer to a `libc::sysinfo` struct.
    let res = unsafe { libc::sysinfo(info.as_mut_ptr()) };
    uptime = if res == 0 {
      // SAFETY: `sysinfo` initializes the struct.
      let info = unsafe { info.assume_init() };
      info.uptime as u64
    } else {
      0
    }
  }

  #[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  {
    use std::mem;
    use std::time::Duration;
    use std::time::SystemTime;
    let mut request = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    // SAFETY: `boottime` is only accessed if sysctl() succeeds
    // and agrees with the `size` set by sysctl().
    let mut boottime: libc::timeval = unsafe { mem::zeroed() };
    let mut size: libc::size_t = mem::size_of_val(&boottime) as libc::size_t;
    // SAFETY: `sysctl` is thread-safe.
    let res = unsafe {
      libc::sysctl(
        &mut request[0],
        2,
        &mut boottime as *mut libc::timeval as *mut libc::c_void,
        &mut size,
        std::ptr::null_mut(),
        0,
      )
    };
    uptime = if res == 0 {
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| {
          (d - Duration::new(
            boottime.tv_sec as u64,
            boottime.tv_usec as u32 * 1000,
          ))
          .as_secs()
        })
        .unwrap_or_default()
    } else {
      0
    }
  }

  #[cfg(target_family = "windows")]
  // SAFETY: windows API usage
  unsafe {
    // Windows is the only one that returns `uptime` in millisecond precision,
    // so we need to get the seconds out of it to be in sync with other envs.
    uptime = winapi::um::sysinfoapi::GetTickCount64() / 1000;
  }

  uptime
}
