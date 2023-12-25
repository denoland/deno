use deno_core::serde::Serialize;

#[derive(Debug, Default, Serialize, Clone)]
pub struct CpuTimes {
  pub user: u64,
  pub nice: u64,
  pub sys: u64,
  pub idle: u64,
  pub irq: u64,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct CpuInfo {
  pub model: String,
  pub speed: u64,
  pub times: CpuTimes,
}

impl CpuInfo {
  pub fn new() -> Self {
    Self::default()
  }
}

#[cfg(target_os = "macos")]
pub fn cpu_info() -> Option<Vec<CpuInfo>> {
  let mut model: [u8; 512] = [0; 512];
  let mut size = std::mem::size_of_val(&model);

  // Safety: Assumes correct behavior of platform-specific syscalls and data structures.
  // Relies on specific sysctl names and sysconf parameter existence.
  unsafe {
    let ticks = libc::sysconf(libc::_SC_CLK_TCK);
    let multiplier = 1000u64 / ticks as u64;
    if libc::sysctlbyname(
      "machdep.cpu.brand_string\0".as_ptr() as *const libc::c_char,
      model.as_mut_ptr() as _,
      &mut size,
      std::ptr::null_mut(),
      0,
    ) != 0
      && libc::sysctlbyname(
        "hw.model\0".as_ptr() as *const libc::c_char,
        model.as_mut_ptr() as _,
        &mut size,
        std::ptr::null_mut(),
        0,
      ) != 0
    {
      return None;
    }

    let mut cpu_speed: u64 = 0;
    let mut cpu_speed_size = std::mem::size_of_val(&cpu_speed);

    libc::sysctlbyname(
      "hw.cpufrequency\0".as_ptr() as *const libc::c_char,
      &mut cpu_speed as *mut _ as *mut libc::c_void,
      &mut cpu_speed_size,
      std::ptr::null_mut(),
      0,
    );

    if cpu_speed == 0 {
      // use a fixed value for frequency on arm64
      cpu_speed = 2400000000;
    }

    let mut num_cpus: libc::natural_t = 0;
    let mut info: *mut libc::processor_cpu_load_info_data_t =
      std::ptr::null_mut();
    let mut msg_type: libc::mach_msg_type_number_t = 0;
    if libc::host_processor_info(
      libc::mach_host_self(),
      libc::PROCESSOR_CPU_LOAD_INFO,
      &mut num_cpus,
      &mut info as *mut _ as *mut libc::processor_info_array_t,
      &mut msg_type,
    ) != 0
    {
      return None;
    }

    let mut cpus = vec![CpuInfo::new(); num_cpus as usize];

    let info = std::slice::from_raw_parts(info, num_cpus as usize);
    let model = std::ffi::CStr::from_ptr(model.as_ptr() as _)
      .to_string_lossy()
      .into_owned();
    for (i, cpu) in cpus.iter_mut().enumerate() {
      cpu.times.user =
        info[i].cpu_ticks[libc::CPU_STATE_USER as usize] as u64 * multiplier;
      cpu.times.nice =
        info[i].cpu_ticks[libc::CPU_STATE_NICE as usize] as u64 * multiplier;
      cpu.times.sys =
        info[i].cpu_ticks[libc::CPU_STATE_SYSTEM as usize] as u64 * multiplier;
      cpu.times.idle =
        info[i].cpu_ticks[libc::CPU_STATE_IDLE as usize] as u64 * multiplier;

      cpu.times.irq = 0;

      cpu.model = model.clone();
      cpu.speed = cpu_speed / 1000000;
    }

    libc::vm_deallocate(
      libc::mach_task_self(),
      info.as_ptr() as libc::vm_address_t,
      msg_type as _,
    );

    Some(cpus)
  }
}

#[cfg(target_os = "windows")]
pub fn cpu_info() -> Option<Vec<CpuInfo>> {
  None
}
