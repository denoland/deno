// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(unix)]
use std::ffi::OsStr;
use std::io;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

pub fn open_url_detached(url: &str) -> io::Result<()> {
  #[cfg(windows)]
  {
    shell_execute_open(url)
  }

  #[cfg(target_os = "macos")]
  {
    spawn_detached("/usr/bin/open", &[OsStr::new(url)])
  }

  #[cfg(all(unix, not(target_os = "macos")))]
  {
    open_url_detached_linux(url)
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_url_detached_linux(url: &str) -> io::Result<()> {
  let mut last_err = None;

  for (program, args) in linux_open_commands(url) {
    match spawn_detached(program, &args) {
      Ok(()) => return Ok(()),
      Err(err) => last_err = Some(err),
    }
  }

  Err(last_err.expect("at least one launcher should be configured"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_open_commands(url: &str) -> Vec<(&'static str, Vec<&OsStr>)> {
  let mut commands = Vec::with_capacity(6);
  if is_wsl() {
    commands.push(("wslview", vec![OsStr::new(url)]));
    commands.push(("explorer.exe", vec![OsStr::new(url)]));
  }

  commands.push(("xdg-open", vec![OsStr::new(url)]));
  commands.push(("gio", vec![OsStr::new("open"), OsStr::new(url)]));
  commands.push(("gnome-open", vec![OsStr::new(url)]));
  commands.push(("kde-open", vec![OsStr::new(url)]));
  commands
}

#[cfg(all(unix, not(target_os = "macos")))]
fn is_wsl() -> bool {
  std::env::var_os("WSL_DISTRO_NAME").is_some()
    || std::env::var_os("WSL_INTEROP").is_some()
}

#[cfg(unix)]
fn spawn_detached(program: &str, args: &[&OsStr]) -> io::Result<()> {
  let mut command = Command::new(program);
  command
    .args(args.iter().copied())
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  // SAFETY: this runs in the child process immediately before exec to detach
  // the launcher from the Deno process group.
  unsafe {
    use std::os::unix::process::CommandExt;

    command.pre_exec(move || {
      match libc::fork() {
        -1 => return Err(io::Error::last_os_error()),
        0 => (),
        _ => libc::_exit(0),
      }

      if libc::setsid() == -1 {
        return Err(io::Error::last_os_error());
      }

      Ok(())
    });
  }

  let mut child = command.spawn()?;
  let status = child.wait()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::other(format!(
      "failed to detach opener process: {status}"
    )))
  }
}

#[cfg(windows)]
fn shell_execute_open(url: &str) -> io::Result<()> {
  use std::os::windows::ffi::OsStrExt;

  const SW_SHOW: i32 = 5;
  static OPEN: [u16; 5] = [111, 112, 101, 110, 0];

  fn wide(input: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    input
      .as_ref()
      .encode_wide()
      .chain(std::iter::once(0))
      .collect()
  }

  let file = wide(url);

  // SAFETY: ShellExecuteW receives valid null-terminated UTF-16 strings and
  // null optional parameters.
  let result = unsafe {
    ShellExecuteW(
      std::ptr::null_mut(),
      OPEN.as_ptr(),
      file.as_ptr(),
      std::ptr::null(),
      std::ptr::null(),
      SW_SHOW,
    )
  };

  if result as isize > 32 {
    Ok(())
  } else {
    Err(io::Error::last_os_error())
  }
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
  fn ShellExecuteW(
    hwnd: *mut std::ffi::c_void,
    lpoperation: *const u16,
    lpfile: *const u16,
    lpparameters: *const u16,
    lpdirectory: *const u16,
    nshowcmd: i32,
  ) -> isize;
}

#[cfg(test)]
#[cfg(all(unix, not(target_os = "macos")))]
mod tests {
  use super::*;

  #[test]
  fn linux_open_commands_keep_url_verbatim() {
    let commands = linux_open_commands("http://localhost:8000/");
    assert!(commands.iter().any(|(program, args)| {
      *program == "xdg-open"
        && args == &vec![OsStr::new("http://localhost:8000/")]
    }));
  }
}
