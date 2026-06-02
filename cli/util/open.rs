// Copyright 2018-2026 the Deno authors. MIT license.

use std::io;

#[cfg(any(unix, test))]
use std::ffi::OsString;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

#[cfg(any(unix, test))]
#[derive(Debug, PartialEq, Eq)]
struct CommandSpec {
  program: OsString,
  args: Vec<OsString>,
}

#[cfg(any(unix, test))]
impl CommandSpec {
  fn new(
    program: impl Into<OsString>,
    args: impl IntoIterator<Item = impl Into<OsString>>,
  ) -> Self {
    Self {
      program: program.into(),
      args: args.into_iter().map(Into::into).collect(),
    }
  }
}

pub fn open_url_detached(url: &str) -> io::Result<()> {
  #[cfg(windows)]
  {
    return shell_execute_open(url);
  }

  #[cfg(target_os = "macos")]
  {
    return spawn_first_detached(open_url_commands(
      url,
      Platform::Macos,
      std::env::var_os("BROWSER"),
    ));
  }

  #[cfg(all(unix, not(target_os = "macos")))]
  {
    spawn_first_detached(open_url_commands(
      url,
      Platform::Unix { is_wsl: is_wsl() },
      std::env::var_os("BROWSER"),
    ))
  }
}

#[cfg(unix)]
fn spawn_first_detached(
  commands: impl IntoIterator<Item = CommandSpec>,
) -> io::Result<()> {
  let mut last_err = None;
  for spec in commands {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    match spawn_detached(&mut command) {
      Ok(()) => return Ok(()),
      Err(err) => last_err = Some(err),
    }
  }
  Err(last_err.expect("at least one launcher should be configured"))
}

#[cfg(unix)]
fn spawn_detached(command: &mut Command) -> io::Result<()> {
  command
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

  command.spawn().map(|_| ())
}

#[cfg(any(unix, test))]
enum Platform {
  #[cfg(any(target_os = "macos", test))]
  Macos,
  #[cfg(any(all(unix, not(target_os = "macos")), test))]
  Unix { is_wsl: bool },
  #[cfg(test)]
  Windows,
}

#[cfg(any(unix, test))]
fn open_url_commands(
  url: &str,
  platform: Platform,
  browser: Option<OsString>,
) -> Vec<CommandSpec> {
  #[cfg(all(target_os = "macos", not(test)))]
  let _ = browser;

  match platform {
    #[cfg(any(target_os = "macos", test))]
    Platform::Macos => vec![CommandSpec::new("/usr/bin/open", [url])],
    #[cfg(any(all(unix, not(target_os = "macos")), test))]
    Platform::Unix { is_wsl } => {
      let mut commands = Vec::new();
      if let Some(browser) = browser {
        commands.extend(browser_commands(browser, url));
      }
      if is_wsl {
        commands.push(CommandSpec::new("wslview", [url]));
        commands.push(CommandSpec::new("explorer.exe", [url]));
      }
      commands.extend([
        CommandSpec::new("xdg-open", [url]),
        CommandSpec::new("gio", ["open", url]),
        CommandSpec::new("gnome-open", [url]),
        CommandSpec::new("kde-open", [url]),
      ]);
      commands
    }
    #[cfg(test)]
    Platform::Windows => vec![CommandSpec::new("ShellExecuteW", ["open", url])],
  }
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn browser_commands(browser: OsString, url: &str) -> Vec<CommandSpec> {
  browser
    .to_string_lossy()
    .split(':')
    .filter_map(|entry| browser_command(entry, url))
    .collect()
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn browser_command(entry: &str, url: &str) -> Option<CommandSpec> {
  let mut parts = entry.split_whitespace();
  let program = parts.next()?;
  let mut args: Vec<OsString> = parts
    .map(|part| OsString::from(part.replace("%s", url)))
    .collect();
  if !args.iter().any(|arg| arg.to_string_lossy().contains(url)) {
    args.push(OsString::from(url));
  }
  Some(CommandSpec::new(program, args))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn is_wsl() -> bool {
  std::env::var_os("WSL_DISTRO_NAME").is_some()
    || std::env::var_os("WSL_INTEROP").is_some()
    || std::fs::read_to_string("/proc/version")
      .map(|version| version.to_ascii_lowercase().contains("microsoft"))
      .unwrap_or(false)
}

#[cfg(windows)]
fn shell_execute_open(url: &str) -> io::Result<()> {
  use std::os::windows::ffi::OsStrExt;

  use windows_sys::Win32::UI::Shell::ShellExecuteW;

  const SW_SHOW: i32 = 5;

  fn wide(input: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    input
      .as_ref()
      .encode_wide()
      .chain(std::iter::once(0))
      .collect()
  }

  let operation = wide("open");
  let file = wide(url);

  // SAFETY: ShellExecuteW receives valid null-terminated UTF-16 strings and
  // null optional parameters.
  let result = unsafe {
    ShellExecuteW(
      std::ptr::null_mut(),
      operation.as_ptr(),
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

#[cfg(test)]
mod tests {
  use super::*;

  fn strings(spec: &CommandSpec) -> (&str, Vec<&str>) {
    (
      spec.program.to_str().unwrap(),
      spec.args.iter().map(|arg| arg.to_str().unwrap()).collect(),
    )
  }

  #[test]
  fn macos_uses_open() {
    let commands = open_url_commands(
      "https://example.com/a b?x=1&y=2",
      Platform::Macos,
      None,
    );
    assert_eq!(commands.len(), 1);
    assert_eq!(
      strings(&commands[0]),
      ("/usr/bin/open", vec!["https://example.com/a b?x=1&y=2"])
    );
  }

  #[test]
  fn unix_uses_browser_then_desktop_openers() {
    let commands = open_url_commands(
      "https://example.com/a b?x=1&y=2",
      Platform::Unix { is_wsl: false },
      Some(OsString::from("firefox")),
    );
    assert_eq!(
      strings(&commands[0]),
      ("firefox", vec!["https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[1]),
      ("xdg-open", vec!["https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[2]),
      ("gio", vec!["open", "https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[3]),
      ("gnome-open", vec!["https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[4]),
      ("kde-open", vec!["https://example.com/a b?x=1&y=2"])
    );
  }

  #[test]
  fn unix_browser_supports_percent_substitution() {
    let commands = open_url_commands(
      "https://example.com/?q=a&b=c",
      Platform::Unix { is_wsl: false },
      Some(OsString::from("browser --new-window %s")),
    );
    assert_eq!(
      strings(&commands[0]),
      (
        "browser",
        vec!["--new-window", "https://example.com/?q=a&b=c"]
      )
    );
  }

  #[test]
  fn wsl_prefers_wsl_launchers_before_linux_desktop_openers() {
    let commands = open_url_commands(
      "https://example.com/a b?x=1&y=2",
      Platform::Unix { is_wsl: true },
      None,
    );
    assert_eq!(
      strings(&commands[0]),
      ("wslview", vec!["https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[1]),
      ("explorer.exe", vec!["https://example.com/a b?x=1&y=2"])
    );
    assert_eq!(
      strings(&commands[2]),
      ("xdg-open", vec!["https://example.com/a b?x=1&y=2"])
    );
  }

  #[test]
  fn windows_uses_shell_execute_spec_for_tests() {
    let commands = open_url_commands(
      "https://example.com/a b?x=1&y=2",
      Platform::Windows,
      None,
    );
    assert_eq!(
      strings(&commands[0]),
      (
        "ShellExecuteW",
        vec!["open", "https://example.com/a b?x=1&y=2"]
      )
    );
  }
}
