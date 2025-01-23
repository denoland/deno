// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(target_os = "windows")]
#[derive(Debug, thiserror::Error)]
#[error("Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK), but got {0}")]
pub struct InvalidSignalStrError(pub String);

#[cfg(any(
  target_os = "android",
  target_os = "linux",
  target_os = "openbsd",
  target_os = "openbsd",
  target_os = "macos",
  target_os = "solaris",
  target_os = "illumos"
))]
#[derive(Debug, thiserror::Error)]
#[error("Invalid signal: {0}")]
pub struct InvalidSignalStrError(pub String);

#[cfg(target_os = "windows")]
#[derive(Debug, thiserror::Error)]
#[error("Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK), but got {0}")]
pub struct InvalidSignalIntError(pub libc::c_int);

#[cfg(any(
  target_os = "android",
  target_os = "linux",
  target_os = "openbsd",
  target_os = "openbsd",
  target_os = "macos",
  target_os = "solaris",
  target_os = "illumos"
))]
#[derive(Debug, thiserror::Error)]
#[error("Invalid signal: {0}")]
pub struct InvalidSignalIntError(pub libc::c_int);

macro_rules! first_literal {
  ($head:literal $(, $tail:literal)*) => {
    $head
  };
}

macro_rules! signal_dict {
  ($(($number:literal, $($name:literal)|+)),*) => {

    pub const SIGNAL_NUMS: &'static [libc::c_int] = &[
      $(
          $number
      ),*
    ];

    pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, InvalidSignalStrError> {
      match s {
        $($($name)|* => Ok($number),)*
        _ => Err(InvalidSignalStrError(s.to_string())),
      }
    }

    pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, InvalidSignalIntError> {
      match s {
        $($number => Ok(first_literal!($($name),+)),)*
        _ => Err(InvalidSignalIntError(s)),
      }
    }
  }
}

#[cfg(target_os = "freebsd")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2"),
  (32, "SIGTHR"),
  (33, "SIGLIBRT")
);

#[cfg(target_os = "openbsd")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2"),
  (32, "SIGTHR")
);

#[cfg(any(target_os = "android", target_os = "linux"))]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGBUS"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGUSR1"),
  (11, "SIGSEGV"),
  (12, "SIGUSR2"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGSTKFLT"),
  (17, "SIGCHLD"),
  (18, "SIGCONT"),
  (19, "SIGSTOP"),
  (20, "SIGTSTP"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGURG"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGIO" | "SIGPOLL"),
  (30, "SIGPWR"),
  (31, "SIGSYS" | "SIGUNUSED")
);

#[cfg(target_os = "macos")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2")
);

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGUSR1"),
  (17, "SIGUSR2"),
  (18, "SIGCHLD"),
  (19, "SIGPWR"),
  (20, "SIGWINCH"),
  (21, "SIGURG"),
  (22, "SIGPOLL"),
  (23, "SIGSTOP"),
  (24, "SIGTSTP"),
  (25, "SIGCONT"),
  (26, "SIGTTIN"),
  (27, "SIGTTOU"),
  (28, "SIGVTALRM"),
  (29, "SIGPROF"),
  (30, "SIGXCPU"),
  (31, "SIGXFSZ"),
  (32, "SIGWAITING"),
  (33, "SIGLWP"),
  (34, "SIGFREEZE"),
  (35, "SIGTHAW"),
  (36, "SIGCANCEL"),
  (37, "SIGLOST"),
  (38, "SIGXRES"),
  (39, "SIGJVM1"),
  (40, "SIGJVM2")
);

#[cfg(target_os = "windows")]
signal_dict!((2, "SIGINT"), (21, "SIGBREAK"));
