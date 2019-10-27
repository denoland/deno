// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::{permission_denied_msg, type_error};
use crate::flags::DenoFlags;
use ansi_term::Style;
use deno::ErrBox;
use log;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use url::Url;

const PERMISSION_EMOJI: &str = "⚠️";

/// Tri-state value for storing permission state
#[derive(PartialEq)]
pub enum PermissionAccessorState {
  Allow = 0,
  Ask = 1,
  Deny = 2,
}

impl From<usize> for PermissionAccessorState {
  fn from(val: usize) -> Self {
    match val {
      0 => PermissionAccessorState::Allow,
      1 => PermissionAccessorState::Ask,
      2 => PermissionAccessorState::Deny,
      _ => unreachable!(),
    }
  }
}

impl From<bool> for PermissionAccessorState {
  fn from(val: bool) -> Self {
    if val {
      PermissionAccessorState::Allow
    } else {
      PermissionAccessorState::Ask
    }
  }
}

impl fmt::Display for PermissionAccessorState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PermissionAccessorState::Allow => f.pad("granted"),
      PermissionAccessorState::Ask => f.pad("prompt"),
      PermissionAccessorState::Deny => f.pad("denied"),
    }
  }
}

#[derive(Debug)]
pub struct PermissionAccessor {
  state: Arc<AtomicUsize>,
}

impl PermissionAccessor {
  pub fn new(state: PermissionAccessorState) -> Self {
    Self {
      state: Arc::new(AtomicUsize::new(state as usize)),
    }
  }

  pub fn is_allow(&self) -> bool {
    match self.get_state() {
      PermissionAccessorState::Allow => true,
      _ => false,
    }
  }

  /// If the state is "Allow" walk it back to the default "Ask"
  /// Don't do anything if state is "Deny"
  pub fn revoke(&self) {
    if self.is_allow() {
      self.ask();
    }
  }

  pub fn allow(&self) {
    self.set_state(PermissionAccessorState::Allow)
  }

  pub fn ask(&self) {
    self.set_state(PermissionAccessorState::Ask)
  }

  #[inline]
  pub fn get_state(&self) -> PermissionAccessorState {
    self.state.load(Ordering::SeqCst).into()
  }
  fn set_state(&self, state: PermissionAccessorState) {
    self.state.store(state as usize, Ordering::SeqCst)
  }
}

impl From<bool> for PermissionAccessor {
  fn from(val: bool) -> Self {
    Self::new(PermissionAccessorState::from(val))
  }
}

impl Default for PermissionAccessor {
  fn default() -> Self {
    Self {
      state: Arc::new(AtomicUsize::new(PermissionAccessorState::Ask as usize)),
    }
  }
}

#[derive(Debug, Default)]
pub struct DenoPermissions {
  // Keep in sync with cli/js/permissions.ts
  pub allow_read: PermissionAccessor,
  pub read_whitelist: Arc<HashSet<String>>,
  pub allow_write: PermissionAccessor,
  pub write_whitelist: Arc<HashSet<String>>,
  pub allow_net: PermissionAccessor,
  pub net_whitelist: Arc<HashSet<String>>,
  pub allow_env: PermissionAccessor,
  pub allow_run: PermissionAccessor,
  pub allow_hrtime: PermissionAccessor,
}

impl DenoPermissions {
  pub fn from_flags(flags: &DenoFlags) -> Self {
    Self {
      allow_read: PermissionAccessor::from(flags.allow_read),
      read_whitelist: Arc::new(flags.read_whitelist.iter().cloned().collect()),
      allow_write: PermissionAccessor::from(flags.allow_write),
      write_whitelist: Arc::new(
        flags.write_whitelist.iter().cloned().collect(),
      ),
      allow_net: PermissionAccessor::from(flags.allow_net),
      net_whitelist: Arc::new(flags.net_whitelist.iter().cloned().collect()),
      allow_env: PermissionAccessor::from(flags.allow_env),
      allow_run: PermissionAccessor::from(flags.allow_run),
      allow_hrtime: PermissionAccessor::from(flags.allow_hrtime),
    }
  }

  /** Checks the permission state and returns the result. */
  fn check_permission_state(
    &self,
    state: PermissionAccessorState,
    msg: &str,
    err_msg: &str,
  ) -> Result<(), ErrBox> {
    if state == PermissionAccessorState::Allow {
      self.log_perm_access(msg);
      return Ok(());
    }
    Err(permission_denied_msg(err_msg.to_string()))
  }

  pub fn check_run(&self) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.allow_run.get_state(),
      "access to run a subprocess",
      "run again with the --allow-run flag",
    )
  }

  fn get_state_read(&self, filename: &Option<&str>) -> PermissionAccessorState {
    if check_path_white_list(filename, &self.read_whitelist) {
      return PermissionAccessorState::Allow;
    }
    self.allow_read.get_state()
  }

  pub fn check_read(&self, filename: &str) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.get_state_read(&Some(filename)),
      &format!("read access to \"{}\"", filename),
      "run again with the --allow-read flag",
    )
  }

  fn get_state_write(
    &self,
    filename: &Option<&str>,
  ) -> PermissionAccessorState {
    if check_path_white_list(filename, &self.write_whitelist) {
      return PermissionAccessorState::Allow;
    }
    self.allow_write.get_state()
  }

  pub fn check_write(&self, filename: &str) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.get_state_write(&Some(filename)),
      &format!("write access to \"{}\"", filename),
      "run again with the --allow-write flag",
    )
  }

  fn get_state_net(
    &self,
    host: &str,
    port: Option<u16>,
  ) -> PermissionAccessorState {
    if check_host_and_port_whitelist(host, port, &self.net_whitelist) {
      return PermissionAccessorState::Allow;
    }
    self.allow_net.get_state()
  }

  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.get_state_net(hostname, Some(port)),
      &format!("network access to \"{}:{}\"", hostname, port),
      "run again with the --allow-net flag",
    )
  }

  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.get_state_net(&format!("{}", url.host().unwrap()), url.port()),
      &format!("network access to \"{}\"", url),
      "run again with the --allow-net flag",
    )
  }

  pub fn check_env(&self) -> Result<(), ErrBox> {
    self.check_permission_state(
      self.allow_env.get_state(),
      "access to environment variables",
      "run again with the --allow-env flag",
    )
  }

  fn log_perm_access(&self, message: &str) {
    if log_enabled!(log::Level::Info) {
      eprintln!(
        "{}",
        Style::new()
          .bold()
          .paint(format!("{}️  Granted {}", PERMISSION_EMOJI, message))
      );
    }
  }

  pub fn get_permission_state(
    &self,
    name: &str,
    url: &Option<&str>,
    path: &Option<&str>,
  ) -> Result<PermissionAccessorState, ErrBox> {
    match name {
      "run" => Ok(self.allow_run.get_state()),
      "read" => Ok(self.get_state_read(path)),
      "write" => Ok(self.get_state_write(path)),
      "net" => {
        // If url is not given, then just check the entire net permission
        if url.is_none() {
          return Ok(self.allow_net.get_state());
        }
        let url: &str = url.unwrap();
        // If url is invalid, then throw a TypeError.
        let parsed = Url::parse(url)
          .map_err(|_| type_error(format!("Invalid url: {}", url)))?;
        let state = self
          .get_state_net(&format!("{}", parsed.host().unwrap()), parsed.port());
        Ok(state)
      }
      "env" => Ok(self.allow_env.get_state()),
      "hrtime" => Ok(self.allow_hrtime.get_state()),
      n => Err(type_error(format!("No such permission name: {}", n))),
    }
  }
}

fn check_path_white_list(
  filename: &Option<&str>,
  white_list: &Arc<HashSet<String>>,
) -> bool {
  if filename.is_none() {
    return false;
  }
  let mut path_buf = PathBuf::from(filename.unwrap());
  loop {
    if white_list.contains(path_buf.to_str().unwrap()) {
      return true;
    }
    if !path_buf.pop() {
      break;
    }
  }
  false
}

fn check_host_and_port_whitelist(
  host: &str,
  port: Option<u16>,
  whitelist: &Arc<HashSet<String>>,
) -> bool {
  whitelist.contains(host)
    || (port.is_some()
      && whitelist.contains(&format!("{}:{}", host, port.unwrap())))
}

#[cfg(test)]
mod tests {
  use super::*;

  // Creates vector of strings, Vec<String>
  macro_rules! svec {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
  }

  #[test]
  fn check_paths() {
    let whitelist = svec!["/a/specific/dir/name", "/a/specific", "/b/c"];

    let perms = DenoPermissions::from_flags(&DenoFlags {
      read_whitelist: whitelist.clone(),
      write_whitelist: whitelist.clone(),
      ..Default::default()
    });

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms.check_read("/a/specific/dir/name").is_ok());
    assert!(perms.check_write("/a/specific/dir/name").is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.check_read("/a/specific/dir").is_ok());
    assert!(perms.check_write("/a/specific/dir").is_ok());

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms.check_read("/a/specific/dir/name/inner").is_ok());
    assert!(perms.check_write("/a/specific/dir/name/inner").is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.check_read("/a/specific/other/dir").is_ok());
    assert!(perms.check_write("/a/specific/other/dir").is_ok());

    // Exact match with /b/c
    assert!(perms.check_read("/b/c").is_ok());
    assert!(perms.check_write("/b/c").is_ok());

    // Sub path within /b/c
    assert!(perms.check_read("/b/c/sub/path").is_ok());
    assert!(perms.check_write("/b/c/sub/path").is_ok());

    // Inside of /b but outside of /b/c
    assert!(perms.check_read("/b/e").is_err());
    assert!(perms.check_write("/b/e").is_err());

    // Inside of /a but outside of /a/specific
    assert!(perms.check_read("/a/b").is_err());
    assert!(perms.check_write("/a/b").is_err());
  }

  #[test]
  fn test_check_net() {
    let perms = DenoPermissions::from_flags(&DenoFlags {
      net_whitelist: svec![
        "localhost",
        "deno.land",
        "github.com:3000",
        "127.0.0.1",
        "172.16.0.2:8000"
      ],
      ..Default::default()
    });

    let domain_tests = vec![
      ("localhost", 1234, true),
      ("deno.land", 0, true),
      ("deno.land", 3000, true),
      ("deno.lands", 0, false),
      ("deno.lands", 3000, false),
      ("github.com", 3000, true),
      ("github.com", 0, false),
      ("github.com", 2000, false),
      ("github.net", 3000, false),
      ("127.0.0.1", 0, true),
      ("127.0.0.1", 3000, true),
      ("127.0.0.2", 0, false),
      ("127.0.0.2", 3000, false),
      ("172.16.0.2", 8000, true),
      ("172.16.0.2", 0, false),
      ("172.16.0.2", 6000, false),
      ("172.16.0.1", 8000, false),
      // Just some random hosts that should err
      ("somedomain", 0, false),
      ("192.168.0.1", 0, false),
    ];

    let url_tests = vec![
      // Any protocol + port for localhost should be ok, since we don't specify
      ("http://localhost", true),
      ("https://localhost", true),
      ("https://localhost:4443", true),
      ("tcp://localhost:5000", true),
      ("udp://localhost:6000", true),
      // Correct domain + any port and protocol should be ok incorrect shouldn't
      ("https://deno.land/std/example/welcome.ts", true),
      ("https://deno.land:3000/std/example/welcome.ts", true),
      ("https://deno.lands/std/example/welcome.ts", false),
      ("https://deno.lands:3000/std/example/welcome.ts", false),
      // Correct domain + port should be ok all other combinations should err
      ("https://github.com:3000/denoland/deno", true),
      ("https://github.com/denoland/deno", false),
      ("https://github.com:2000/denoland/deno", false),
      ("https://github.net:3000/denoland/deno", false),
      // Correct ipv4 address + any port should be ok others should err
      ("tcp://127.0.0.1", true),
      ("https://127.0.0.1", true),
      ("tcp://127.0.0.1:3000", true),
      ("https://127.0.0.1:3000", true),
      ("tcp://127.0.0.2", false),
      ("https://127.0.0.2", false),
      ("tcp://127.0.0.2:3000", false),
      ("https://127.0.0.2:3000", false),
      // Correct address + port should be ok all other combinations should err
      ("tcp://172.16.0.2:8000", true),
      ("https://172.16.0.2:8000", true),
      ("tcp://172.16.0.2", false),
      ("https://172.16.0.2", false),
      ("tcp://172.16.0.2:6000", false),
      ("https://172.16.0.2:6000", false),
      ("tcp://172.16.0.1:8000", false),
      ("https://172.16.0.1:8000", false),
    ];

    for (url_str, is_ok) in url_tests.iter() {
      let u = url::Url::parse(url_str).unwrap();
      assert_eq!(*is_ok, perms.check_net_url(&u).is_ok());
    }

    for (host, port, is_ok) in domain_tests.iter() {
      assert_eq!(*is_ok, perms.check_net(host, *port).is_ok());
    }
  }
}
