// Copyright 2018-2026 the Deno authors. MIT license.

use std::io;
use std::process::Command;
use std::process::Stdio;

pub fn open_url_detached(url: &str) -> io::Result<()> {
  let mut command = command(url);
  command
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  command.spawn()?;
  Ok(())
}

#[cfg(target_os = "macos")]
fn command(url: &str) -> Command {
  let mut command = Command::new("open");
  command.arg(url);
  command
}

#[cfg(target_os = "windows")]
fn command(url: &str) -> Command {
  let mut command = Command::new("rundll32");
  command.args(["url.dll,FileProtocolHandler", url]);
  command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn command(url: &str) -> Command {
  let mut command = Command::new("xdg-open");
  command.arg(url);
  command
}
