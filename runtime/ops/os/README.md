## `os` ops

`loadavg`

| Target family | Syscall      | Description                                                          |
| ------------- | ------------ | -------------------------------------------------------------------- |
| Linux         | `sysinfo`    | -                                                                    |
| Windows       | -            | Returns `DEFAULT_LOADAVG`. There is no concept of loadavg on Windows |
| macOS, BSD    | `getloadavg` | https://www.freebsd.org/cgi/man.cgi?query=getloadavg                 |

`os_release`

| Target family | Syscall                                                                                                    | Description                                     |
| ------------- | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| Linux         | `/proc/sys/kernel/osrelease`                                                                               | -                                               |
| Windows       | [`RtlGetVersion`](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-rtlgetversion) | dwMajorVersion . dwMinorVersion . dwBuildNumber |
| macOS         | `sysctl([CTL_KERN, KERN_OSRELEASE])`                                                                       | -                                               |

`hostname`

| Target family | Syscall                                   | Description |
| ------------- | ----------------------------------------- | ----------- |
| Unix          | `gethostname(sysconf(_SC_HOST_NAME_MAX))` | -           |
| Windows       | `GetHostNameW`                            | -           |

`mem_info`

| Target family | Syscall                                                                                                                                       | Description |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| Linux         | sysinfo and `/proc/meminfo`                                                                                                                   | -           |
| Windows       | `sysinfoapi::GlobalMemoryStatusEx`                                                                                                            | -           |
| macOS         | <br> <pre> sysctl([CTL_HW, HW_MEMSIZE]); <br> sysctl([CTL_VM, VM_SWAPUSAGE]); <br> host_statistics64(mach_host_self(), HOST_VM_INFO64) </pre> | -           |

`cpu_usage`

| Target family | Syscall                              | Description |
| ------------- | ------------------------------------ | ----------- |
| Linux         | getrusage                            | -           |
| Windows       | `processthreadsapi::GetProcessTimes` | -           |
| macOS         | getrusage                            | -           |
