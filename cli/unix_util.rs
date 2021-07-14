// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// Raise soft file descriptor limit to hard file descriptor limit.
/// This is the difference between `ulimit -n` and `ulimit -n -H`.
pub fn raise_fd_limit() {
  // as high as possible
  rlimit::utils::increase_nofile_limit(u64::MAX).ok();
}
