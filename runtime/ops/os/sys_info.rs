// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

type LoadAvg = (f64, f64, f64);
const DEFAULT_LOADAVG: LoadAvg = (0.0, 0.0, 0.0);

pub fn loadavg() -> LoadAvg {
  #[cfg(target_os = "linux")]
  {
    let mut s = String::new();
    File::open("/proc/loadavg")?.read_to_string(&mut s)?;
    let loads = s
      .trim()
      .split(' ')
      .take(3)
      .map(|val| val.parse::<f64>().unwrap())
      .collect::<Vec<f64>>();
    (loads[0], loads[1], loads[2])
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
    let mut s = String::new();
    File::open("/proc/sys/kernel/osrelease")?.read_to_string(&mut s)?;
    s.pop(); // pop '\n'
    s
  }
  #[cfg(target_vendor = "apple")]
  {
    let mut s = [0u8; 20];
    let mut mib = [libc::CTL_KERN, libc::KERN_OSRELEASE];
    let mut len = 20;
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

    let mut version_info: RTL_OSVERSIONINFOEXW =
      unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

    version_info.dwOSVersionInfoSize =
      std::mem::size_of::<RTL_OSVERSIONINFOEXW>() as u32;
    if !NT_SUCCESS(unsafe {
      RtlGetVersion(&mut version_info as *mut RTL_OSVERSIONINFOEXW as *mut _)
    }) {
      String::from("")
    } else {
      format!(
        "{}.{}.{}",
        version_info.dwMajorVersion,
        version_info.dwMinorVersion,
        version_info.dwBuildNumber
      )
    }
  }
}

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
    use std::os::windows::ffi::OsStringExt;
    use winapi::um::winsock2::GetHostNameW;

    let namelen = 256;
    let mut name: Vec<u16> = std::iter::repeat(0).take(namelen).collect();
    let err =
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

  #[cfg(any(target_vendor = "apple"))]
  {
    let mut mib: [i32; 2] = [0, 0];
    mib[0] = libc::CTL_HW;
    mib[1] = libc::HW_MEMSIZE;
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
    }
    mem_info.total /= 1024;

    let mut xs: libc::xsw_usage =
      unsafe { std::mem::zeroed::<libc::xsw_usage>() };
    mib[0] = libc::CTL_VM;
    mib[1] = libc::VM_SWAPUSAGE;
    unsafe {
      let mut size = std::mem::size_of::<libc::xsw_usage>();
      libc::sysctl(
        mib.as_mut_ptr(),
        mib.len() as _,
        &mut xs as *mut _ as *mut libc::c_void,
        &mut size,
        std::ptr::null_mut(),
        0,
      );
    }
    mem_info.swap_total = xs.xsu_total;
    mem_info.swap_free = xs.xsu_avail;

    let mut count: u32 = libc::HOST_VM_INFO64_COUNT as _;
    let mut stat = unsafe { std::mem::zeroed::<libc::vm_statistics64>() };
    // SAFETY: We assume that `mach_host_self` always returns a valid value.
    if unsafe {
      libc::host_statistics64(
        // TODO(@littledivy): Put this in a once_cell.
        libc::mach_host_self(),
        libc::HOST_VM_INFO64,
        &mut stat as *mut libc::vm_statistics64 as *mut _,
        &mut count,
      )
    } == libc::KERN_SUCCESS
    {
      // TODO(@littledivy): Put this in a once_cell
      let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
      mem_info.available =
        (stat.free_count as u64 + stat.inactive_count as u64) * page_size
          / 1024;
      mem_info.free = (stat.free_count as u64 - stat.speculative_count as u64)
        * page_size
        / 1024;
    }
  }
  #[cfg(target_family = "windows")]
  unsafe {
    use std::mem;
    use winapi::shared::minwindef;
    use winapi::um::sysinfoapi;

    let mut mem_status =
      mem::MaybeUninit::<sysinfoapi::MEMORYSTATUSEX>::uninit();
    let length =
      mem::size_of::<sysinfoapi::MEMORYSTATUSEX>() as minwindef::DWORD;
    (*mem_status.as_mut_ptr()).dwLength = length;

    let result = sysinfoapi::GlobalMemoryStatusEx(mem_status.as_mut_ptr());
    if result != 0 {
      let stat = mem_status.assume_init();
      mem_info.total = stat.ullTotalPhys / 1024;
      mem_info.available = 0;
      mem_info.free = stat.ullAvailPhys / 1024;
      mem_info.cached = 0;
      mem_info.buffers = 0;
      mem_info.swap_total = (stat.ullTotalPageFile - stat.ullTotalPhys) / 1024;
      mem_info.swap_free = (stat.ullAvailPageFile - stat.ullAvailPhys) / 1024;
      if mem_info.swap_free > mem_info.swap_total {
        mem_info.swap_free = mem_info.swap_total;
      }
    }
  }

  Some(mem_info)
}
