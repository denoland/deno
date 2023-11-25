// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
#[cfg(target_vendor = "apple")]
use libc;
#[cfg(target_family = "windows")]
use std::sync::Once;
#[cfg(not(target_vendor = "apple"))]
use sysinfo::Pid;
#[cfg(not(target_vendor = "apple"))]
use sysinfo::ProcessExt;
#[cfg(not(target_vendor = "apple"))]
use sysinfo::ProcessRefreshKind;
#[cfg(not(target_vendor = "apple"))]
use sysinfo::System;
#[cfg(not(target_vendor = "apple"))]
use sysinfo::SystemExt;

type LoadAvg = (f64, f64, f64);
const DEFAULT_LOADAVG: LoadAvg = (0.0, 0.0, 0.0);

pub fn loadavg() -> LoadAvg {
  #[cfg(target_os = "linux")]
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
  #[cfg(target_os = "linux")]
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
      mem_info.buffers = info.bufferram * mem_unit;
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
      mem_info.total /= 1024;

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
      mem_info.total = stat.ullTotalPhys / 1024;
      mem_info.available = 0;
      mem_info.free = stat.ullAvailPhys / 1024;
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

  #[cfg(target_os = "linux")]
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

#[cfg(target_vendor = "apple")]
#[derive(Debug)]
struct ProcessorCpuLoadInfo {
  cpu_load: libc::processor_cpu_load_info_t,
  cpu_count: libc::natural_t,
}

#[cfg(target_vendor = "apple")]
impl ProcessorCpuLoadInfo {
  fn new(port: libc::mach_port_t) -> Option<Self> {
    let mut info_size =
      std::mem::size_of::<libc::processor_cpu_load_info_t>() as _;
    let mut cpu_count = 0;
    let mut cpu_load: libc::processor_cpu_load_info_t = std::ptr::null_mut();

    // SAFETY: Vendored code.
    unsafe {
      if libc::host_processor_info(
        port,
        libc::PROCESSOR_CPU_LOAD_INFO,
        &mut cpu_count,
        &mut cpu_load as *mut _ as *mut _,
        &mut info_size,
      ) != 0
        || cpu_count < 1
        || cpu_load.is_null()
      {
        None
      } else {
        Some(Self {
          cpu_load,
          cpu_count,
        })
      }
    }
  }
}

#[cfg(target_vendor = "apple")]
#[derive(Debug)]
struct SystemTimeInfo {
  timebase_to_ns: f64,
  clock_per_sec: f64,
  old_cpu_info: ProcessorCpuLoadInfo,
}

#[cfg(target_vendor = "apple")]
impl SystemTimeInfo {
  fn new(port: libc::mach_port_t) -> Option<Self> {
    // SAFETY: Vendored code.
    unsafe {
      let clock_ticks_per_sec = libc::sysconf(libc::_SC_CLK_TCK);

      #[allow(deprecated)]
      let mut info = libc::mach_timebase_info_data_t { numer: 0, denom: 0 };
      #[allow(deprecated)]
      if libc::mach_timebase_info(&mut info) != libc::KERN_SUCCESS {
        // mach_timebase_info failed, using default value of 1
        info.numer = 1;
        info.denom = 1;
      }

      let old_cpu_info = match ProcessorCpuLoadInfo::new(port) {
        Some(cpu_info) => cpu_info,
        None => {
          // host_processor_info failed, using old CPU tick measure system
          return None;
        }
      };

      let nano_per_seconds = 1_000_000_000.;
      Some(Self {
        #[allow(deprecated)]
        timebase_to_ns: info.numer as f64 / info.denom as f64,
        clock_per_sec: nano_per_seconds / clock_ticks_per_sec as f64,
        old_cpu_info,
      })
    }
  }

  pub fn get_time_interval(&mut self, port: libc::mach_port_t) -> f64 {
    let mut total = 0;
    let new_cpu_info = match ProcessorCpuLoadInfo::new(port) {
      Some(cpu_info) => cpu_info,
      None => return 0.,
    };
    let cpu_count =
      std::cmp::min(self.old_cpu_info.cpu_count, new_cpu_info.cpu_count);
    // SAFETY: Vendored code.
    unsafe {
      for i in 0..cpu_count {
        let new_load: &libc::processor_cpu_load_info =
          &*new_cpu_info.cpu_load.offset(i as _);
        let old_load: &libc::processor_cpu_load_info =
          &*self.old_cpu_info.cpu_load.offset(i as _);
        for (new, old) in
          new_load.cpu_ticks.iter().zip(old_load.cpu_ticks.iter())
        {
          if new > old {
            total += new.saturating_sub(*old);
          }
        }
      }

      self.old_cpu_info = new_cpu_info;

      // Now we convert the ticks to nanoseconds (if the interval is less than
      // `MINIMUM_CPU_UPDATE_INTERVAL`, we replace it with it instead):
      const MINIMUM_CPU_UPDATE_INTERVAL: std::time::Duration =
        std::time::Duration::from_millis(200);
      let base_interval = total as f64 / cpu_count as f64 * self.clock_per_sec;
      let smallest =
        MINIMUM_CPU_UPDATE_INTERVAL.as_secs_f64() * 1_000_000_000.0;
      if base_interval < smallest {
        smallest
      } else {
        base_interval / self.timebase_to_ns
      }
    }
  }
}

#[derive(Debug)]
pub struct CpuUsageState {
  #[cfg(not(target_vendor = "apple"))]
  system: System,
  #[cfg(target_vendor = "apple")]
  port: libc::mach_port_t,
  #[cfg(target_vendor = "apple")]
  clock_info: Option<SystemTimeInfo>,
  #[cfg(target_vendor = "apple")]
  old_utime: u64,
  #[cfg(target_vendor = "apple")]
  old_stime: u64,
  #[cfg(target_vendor = "apple")]
  cpu_usage: f32,
}

#[allow(clippy::derivable_impls)]
impl Default for CpuUsageState {
  fn default() -> Self {
    #[cfg(target_vendor = "apple")]
    // SAFETY: Vendored code.
    let port = unsafe { libc::mach_host_self() };
    Self {
      #[cfg(not(target_vendor = "apple"))]
      system: Default::default(),
      #[cfg(target_vendor = "apple")]
      port,
      #[cfg(target_vendor = "apple")]
      clock_info: SystemTimeInfo::new(port),
      #[cfg(target_vendor = "apple")]
      old_utime: 0,
      #[cfg(target_vendor = "apple")]
      old_stime: 0,
      #[cfg(target_vendor = "apple")]
      cpu_usage: 0.0,
    }
  }
}

// SAFETY: Only used on the LSP CPU watchdog thread after init.
unsafe impl Send for CpuUsageState {}

impl CpuUsageState {
  pub fn refresh_cpu_usage(&mut self) -> f32 {
    let pid = std::process::id();
    #[cfg(not(target_vendor = "apple"))]
    {
      let pid = Pid::from(pid as usize);
      self
        .system
        .refresh_process_specifics(pid, ProcessRefreshKind::new().with_cpu());
      if let Some(process) = self.system.process(pid) {
        process.cpu_usage()
      } else {
        0.0
      }
    }
    // This code path and its dependencies are vendored from `sysinfo`, to avoid
    // a shared library dependency introduced by that crate on macos.
    #[cfg(target_vendor = "apple")]
    {
      // SAFETY: This will be written below.
      let mut task_info = unsafe { std::mem::zeroed::<libc::proc_taskinfo>() };
      // SAFETY: Vendored code.
      unsafe {
        libc::proc_pidinfo(
          pid as _,
          libc::PROC_PIDTASKINFO,
          0,
          &mut task_info as *mut libc::proc_taskinfo as *mut libc::c_void,
          std::mem::size_of::<libc::proc_taskinfo>() as _,
        )
      };
      // SAFETY: This will be written below.
      let mut thread_info =
        unsafe { std::mem::zeroed::<libc::proc_threadinfo>() };
      // SAFETY: Vendored code.
      let (user_time, system_time) = if unsafe {
        libc::proc_pidinfo(
          pid as _,
          libc::PROC_PIDTHREADINFO,
          0,
          &mut thread_info as *mut libc::proc_threadinfo as *mut libc::c_void,
          std::mem::size_of::<libc::proc_threadinfo>() as _,
        )
      } != 0
      {
        (thread_info.pth_user_time, thread_info.pth_system_time)
      } else {
        return 0.0;
      };
      let time_interval = self
        .clock_info
        .as_mut()
        .map(|c| c.get_time_interval(self.port));
      if let Some(time_interval) = time_interval {
        let total_existing_time = self.old_stime.saturating_add(self.old_utime);
        let mut updated_cpu_usage = false;
        if time_interval > 0.000001 && total_existing_time > 0 {
          let total_current_time = task_info
            .pti_total_system
            .saturating_add(task_info.pti_total_user);
          let total_time_diff =
            total_current_time.saturating_sub(total_existing_time);
          if total_time_diff > 0 {
            self.cpu_usage =
              (total_time_diff as f64 / time_interval * 100.) as f32;
            updated_cpu_usage = true;
          }
        }
        if !updated_cpu_usage {
          self.cpu_usage = 0.0;
        }
        self.old_stime = task_info.pti_total_system;
        self.old_utime = task_info.pti_total_user;
      } else {
        // SAFETY: Vendored code.
        unsafe {
          // This is the "backup way" of CPU computation.
          #[allow(deprecated)]
          let time = libc::mach_absolute_time();
          let task_time = user_time
            .saturating_add(system_time)
            .saturating_add(task_info.pti_total_user)
            .saturating_add(task_info.pti_total_system);

          let system_time_delta = if task_time < self.old_utime {
            task_time
          } else {
            task_time.saturating_sub(self.old_utime)
          };
          let time_delta = if time < self.old_stime {
            time
          } else {
            time.saturating_sub(self.old_stime)
          };
          self.old_utime = task_time;
          self.old_stime = time;
          self.cpu_usage = if time_delta == 0 {
            0f32
          } else {
            (system_time_delta as f64 * 100f64 / time_delta as f64) as f32
          };
        }
      }
      self.cpu_usage
    }
  }
}
