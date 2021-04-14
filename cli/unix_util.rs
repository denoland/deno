/// Raise soft file descriptor limit to hard file descriptor limit.
/// This is the difference between `ulimit -n` and `ulimit -n -H`.
pub fn raise_fd_limit() {
  #[cfg(unix)]
  unsafe {
    let mut limits = libc::rlimit {
      rlim_cur: 0,
      rlim_max: 0,
    };

    if 0 != libc::getrlimit(libc::RLIMIT_NOFILE, &mut limits) {
      return;
    }

    if limits.rlim_cur == libc::RLIM_INFINITY {
      return;
    }

    // No hard limit? Do a binary search for the effective soft limit.
    if limits.rlim_max == libc::RLIM_INFINITY {
      let mut min = limits.rlim_cur;
      let mut max = 1 << 20;

      while min + 1 < max {
        limits.rlim_cur = min + (max - min) / 2;
        match libc::setrlimit(libc::RLIMIT_NOFILE, &limits) {
          0 => min = limits.rlim_cur,
          _ => max = limits.rlim_cur,
        }
      }

      return;
    }

    // Raise the soft limit to the hard limit.
    if limits.rlim_cur < limits.rlim_max {
      limits.rlim_cur = limits.rlim_max;
      libc::setrlimit(libc::RLIMIT_NOFILE, &limits);
    }
  }
}
