// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::permission_denied_msg;
use crate::flags::DenoFlags;
use ansi_term::Style;
use deno::ErrBox;
use log;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const PERMISSION_EMOJI: &str = "⚠️";

/// Tri-state value for storing permission state
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
      PermissionAccessorState::Allow => f.pad("Allow"),
      PermissionAccessorState::Ask => f.pad("Ask"),
      PermissionAccessorState::Deny => f.pad("Deny"),
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
  // Keep in sync with src/permissions.ts
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

  pub fn check_run(&self) -> Result<(), ErrBox> {
    let msg = "access to run a subprocess";

    match self.allow_run.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      PermissionAccessorState::Ask => Err(permission_denied_msg(
        "run again with the --allow-run flag".to_string(),
      )),
      PermissionAccessorState::Deny => Err(permission_denied_msg(
        "run again with the --allow-run flag".to_string(),
      )),
    }
  }

  pub fn check_read(&self, filename: &str) -> Result<(), ErrBox> {
    let msg = &format!("read access to \"{}\"", filename);
    match self.allow_read.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      state => {
        if check_path_white_list(filename, &self.read_whitelist) {
          self.log_perm_access(msg);
          Ok(())
        } else {
          match state {
            PermissionAccessorState::Ask => Err(permission_denied_msg(
              "run again with the --allow-read flag".to_string(),
            )),
            PermissionAccessorState::Deny => Err(permission_denied_msg(
              "run again with the --allow-read flag".to_string(),
            )),
            _ => unreachable!(),
          }
        }
      }
    }
  }

  pub fn check_write(&self, filename: &str) -> Result<(), ErrBox> {
    let msg = &format!("write access to \"{}\"", filename);
    match self.allow_write.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      state => {
        if check_path_white_list(filename, &self.write_whitelist) {
          self.log_perm_access(msg);
          Ok(())
        } else {
          match state {
            PermissionAccessorState::Ask => Err(permission_denied_msg(
              "run again with the --allow-write flag".to_string(),
            )),
            PermissionAccessorState::Deny => Err(permission_denied_msg(
              "run again with the --allow-write flag".to_string(),
            )),
            _ => unreachable!(),
          }
        }
      }
    }
  }

  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    let msg = &format!("network access to \"{}:{}\"", hostname, port);
    match self.allow_net.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      _state => {
        if self.net_whitelist.contains(hostname)
          || self
            .net_whitelist
            .contains(&format!("{}:{}", hostname, port))
        {
          self.log_perm_access(msg);
          Ok(())
        } else {
          Err(permission_denied_msg(
            "run again with the --allow-net flag".to_string(),
          ))
        }
      }
    }
  }

  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    let msg = &format!("network access to \"{}\"", url);
    match self.allow_net.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      _state => {
        let host = url.host().unwrap();
        let whitelist_result = {
          if self.net_whitelist.contains(&format!("{}", host)) {
            true
          } else {
            match url.port() {
              Some(port) => {
                self.net_whitelist.contains(&format!("{}:{}", host, port))
              }
              None => false,
            }
          }
        };
        if whitelist_result {
          self.log_perm_access(msg);
          Ok(())
        } else {
          Err(permission_denied_msg(
            "run again with the --allow-net flag".to_string(),
          ))
        }
      }
    }
  }

  pub fn check_env(&self) -> Result<(), ErrBox> {
    let msg = "access to environment variables";
    match self.allow_env.get_state() {
      PermissionAccessorState::Allow => {
        self.log_perm_access(msg);
        Ok(())
      }
      PermissionAccessorState::Ask => Err(permission_denied_msg(
        "run again with the --allow-env flag".to_string(),
      )),
      PermissionAccessorState::Deny => Err(permission_denied_msg(
        "run again with the --allow-env flag".to_string(),
      )),
    }
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

  pub fn allows_run(&self) -> bool {
    self.allow_run.is_allow()
  }

  pub fn allows_read(&self) -> bool {
    self.allow_read.is_allow()
  }

  pub fn allows_write(&self) -> bool {
    self.allow_write.is_allow()
  }

  pub fn allows_net(&self) -> bool {
    self.allow_net.is_allow()
  }

  pub fn allows_env(&self) -> bool {
    self.allow_env.is_allow()
  }

  pub fn allows_hrtime(&self) -> bool {
    self.allow_hrtime.is_allow()
  }

  pub fn revoke_run(&self) -> Result<(), ErrBox> {
    self.allow_run.revoke();
    Ok(())
  }

  pub fn revoke_read(&self) -> Result<(), ErrBox> {
    self.allow_read.revoke();
    Ok(())
  }

  pub fn revoke_write(&self) -> Result<(), ErrBox> {
    self.allow_write.revoke();
    Ok(())
  }

  pub fn revoke_net(&self) -> Result<(), ErrBox> {
    self.allow_net.revoke();
    Ok(())
  }

  pub fn revoke_env(&self) -> Result<(), ErrBox> {
    self.allow_env.revoke();
    Ok(())
  }
  pub fn revoke_hrtime(&self) -> Result<(), ErrBox> {
    self.allow_hrtime.revoke();
    Ok(())
  }
}

fn check_path_white_list(
  filename: &str,
  white_list: &Arc<HashSet<String>>,
) -> bool {
  let mut path_buf = PathBuf::from(filename);

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
