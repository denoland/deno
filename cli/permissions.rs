// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use atty;

use crate::flags::DenoFlags;

use ansi_term::Style;
use crate::deno_error::permission_denied;
use crate::deno_error::DenoResult;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

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

  pub fn deny(&self) {
    self.set_state(PermissionAccessorState::Deny)
  }

  /// Update this accessors state based on a PromptResult value
  /// This will only update the state if the PromptResult value
  /// is one of the "Always" values
  pub fn update_with_prompt_result(&self, prompt_result: &PromptResult) {
    match prompt_result {
      PromptResult::AllowAlways => self.allow(),
      PromptResult::DenyAlways => self.deny(),
      _ => {}
    }
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
  pub no_prompts: AtomicBool,
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
      no_prompts: AtomicBool::new(flags.no_prompts),
    }
  }

  pub fn check_run(&self) -> DenoResult<()> {
    match self.allow_run.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      PermissionAccessorState::Ask => {
        match self.try_permissions_prompt("access to run a subprocess") {
          Err(e) => Err(e),
          Ok(v) => {
            self.allow_run.update_with_prompt_result(&v);
            v.check()?;
            Ok(())
          }
        }
      }
      PermissionAccessorState::Deny => Err(permission_denied()),
    }
  }

  pub fn check_read(&self, filename: &str) -> DenoResult<()> {
    match self.allow_read.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      state => {
        if check_path_white_list(filename, &self.read_whitelist) {
          Ok(())
        } else {
          match state {
            PermissionAccessorState::Ask => match self.try_permissions_prompt(
              &format!("read access to \"{}\"", filename),
            ) {
              Err(e) => Err(e),
              Ok(v) => {
                self.allow_read.update_with_prompt_result(&v);
                v.check()?;
                Ok(())
              }
            },
            PermissionAccessorState::Deny => Err(permission_denied()),
            _ => unreachable!(),
          }
        }
      }
    }
  }

  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    match self.allow_write.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      state => {
        if check_path_white_list(filename, &self.write_whitelist) {
          Ok(())
        } else {
          match state {
            PermissionAccessorState::Ask => match self.try_permissions_prompt(
              &format!("write access to \"{}\"", filename),
            ) {
              Err(e) => Err(e),
              Ok(v) => {
                self.allow_write.update_with_prompt_result(&v);
                v.check()?;
                Ok(())
              }
            },
            PermissionAccessorState::Deny => Err(permission_denied()),
            _ => unreachable!(),
          }
        }
      }
    }
  }

  pub fn check_net(&self, host_and_port: &str) -> DenoResult<()> {
    match self.allow_net.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      state => {
        let parts = host_and_port.split(':').collect::<Vec<&str>>();
        if match parts.len() {
          2 => {
            if self.net_whitelist.contains(parts[0]) {
              true
            } else {
              self
                .net_whitelist
                .contains(&format!("{}:{}", parts[0], parts[1]))
            }
          }
          1 => self.net_whitelist.contains(parts[0]),
          _ => panic!("Failed to parse origin string: {}", host_and_port),
        } {
          Ok(())
        } else {
          self.check_net_inner(state, host_and_port)
        }
      }
    }
  }

  pub fn check_net_url(&self, url: url::Url) -> DenoResult<()> {
    match self.allow_net.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      state => {
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
          Ok(())
        } else {
          self.check_net_inner(state, &url.to_string())
        }
      }
    }
  }

  fn check_net_inner(
    &self,
    state: PermissionAccessorState,
    prompt_str: &str,
  ) -> DenoResult<()> {
    match state {
      PermissionAccessorState::Ask => match self.try_permissions_prompt(
        &format!("network access to \"{}\"", prompt_str),
      ) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_net.update_with_prompt_result(&v);
          v.check()?;
          Ok(())
        }
      },
      PermissionAccessorState::Deny => Err(permission_denied()),
      _ => unreachable!(),
    }
  }

  pub fn check_env(&self) -> DenoResult<()> {
    match self.allow_env.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      PermissionAccessorState::Ask => {
        match self.try_permissions_prompt("access to environment variables") {
          Err(e) => Err(e),
          Ok(v) => {
            self.allow_env.update_with_prompt_result(&v);
            v.check()?;
            Ok(())
          }
        }
      }
      PermissionAccessorState::Deny => Err(permission_denied()),
    }
  }

  /// Try to present the user with a permission prompt
  /// will error with permission_denied if no_prompts is enabled
  fn try_permissions_prompt(&self, message: &str) -> DenoResult<PromptResult> {
    if self.no_prompts.load(Ordering::SeqCst) {
      return Err(permission_denied());
    }
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
      return Err(permission_denied());
    };
    permission_prompt(message)
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

  pub fn revoke_run(&self) -> DenoResult<()> {
    self.allow_run.revoke();
    Ok(())
  }

  pub fn revoke_read(&self) -> DenoResult<()> {
    self.allow_read.revoke();
    Ok(())
  }

  pub fn revoke_write(&self) -> DenoResult<()> {
    self.allow_write.revoke();
    Ok(())
  }

  pub fn revoke_net(&self) -> DenoResult<()> {
    self.allow_net.revoke();
    Ok(())
  }

  pub fn revoke_env(&self) -> DenoResult<()> {
    self.allow_env.revoke();
    Ok(())
  }
  pub fn revoke_hrtime(&self) -> DenoResult<()> {
    self.allow_hrtime.revoke();
    Ok(())
  }
}

/// Quad-state value for representing user input on permission prompt
#[derive(Debug, Clone)]
pub enum PromptResult {
  AllowAlways = 0,
  AllowOnce = 1,
  DenyOnce = 2,
  DenyAlways = 3,
}

impl PromptResult {
  /// If value is any form of deny this will error with permission_denied
  pub fn check(&self) -> DenoResult<()> {
    match self {
      PromptResult::DenyOnce => Err(permission_denied()),
      PromptResult::DenyAlways => Err(permission_denied()),
      _ => Ok(()),
    }
  }
}

impl fmt::Display for PromptResult {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PromptResult::AllowAlways => f.pad("AllowAlways"),
      PromptResult::AllowOnce => f.pad("AllowOnce"),
      PromptResult::DenyOnce => f.pad("DenyOnce"),
      PromptResult::DenyAlways => f.pad("DenyAlways"),
    }
  }
}

fn permission_prompt(message: &str) -> DenoResult<PromptResult> {
  let msg = format!("⚠️  Deno requests {}. Grant? [a/y/n/d (a = allow always, y = allow once, n = deny once, d = deny always)] ", message);
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{}", Style::new().bold().paint(msg));
  loop {
    let mut input = String::new();
    let stdin = io::stdin();
    let _nread = stdin.read_line(&mut input)?;
    let ch = input.chars().next().unwrap();
    match ch.to_ascii_lowercase() {
      'a' => return Ok(PromptResult::AllowAlways),
      'y' => return Ok(PromptResult::AllowOnce),
      'n' => return Ok(PromptResult::DenyOnce),
      'd' => return Ok(PromptResult::DenyAlways),
      _ => {
        // If we don't get a recognized option try again.
        let msg_again = format!("Unrecognized option '{}' [a/y/n/d (a = allow always, y = allow once, n = deny once, d = deny always)] ", ch);
        eprint!("{}", Style::new().bold().paint(msg_again));
      }
    };
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
  #![allow(clippy::cyclomatic_complexity)]
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
      no_prompts: true,
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
  fn check_net() {
    let perms = DenoPermissions::from_flags(&DenoFlags {
      net_whitelist: svec![
        "localhost",
        "deno.land",
        "github.com:3000",
        "127.0.0.1",
        "172.16.0.2:8000"
      ],
      no_prompts: true,
      ..Default::default()
    });

    // Any protocol + port for localhost should be ok, since we don't specify
    assert!(
      perms
        .check_net_url(url::Url::parse("http://localhost").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("http://localhost:8080").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://localhost").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://localhost:4443").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://localhost:5000").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("udp://localhost:6000").unwrap())
        .is_ok()
    );
    assert!(perms.check_net("localhost:1234").is_ok());

    // Correct domain + any port and protocol should be ok incorrect shouldn't
    assert!(perms.check_net("deno.land").is_ok());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://deno.land/std/example/welcome.ts").unwrap()
        ).is_ok()
    );
    assert!(perms.check_net("deno.land:3000").is_ok());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://deno.land:3000/std/example/welcome.ts")
            .unwrap()
        ).is_ok()
    );
    assert!(perms.check_net("deno.lands").is_err());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://deno.lands/std/example/welcome.ts").unwrap()
        ).is_err()
    );
    assert!(perms.check_net("deno.lands:3000").is_err());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://deno.lands:3000/std/example/welcome.ts")
            .unwrap()
        ).is_err()
    );

    // Correct domain + port should be ok all other combinations should err
    assert!(perms.check_net("github.com:3000").is_ok());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://github.com:3000/denoland/deno").unwrap()
        ).is_ok()
    );
    assert!(perms.check_net("github.com").is_err());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://github.com/denoland/deno").unwrap()
        ).is_err()
    );
    assert!(perms.check_net("github.com:2000").is_err());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://github.com:2000/denoland/deno").unwrap()
        ).is_err()
    );
    assert!(perms.check_net("github.net:3000").is_err());
    assert!(
      perms
        .check_net_url(
          url::Url::parse("https://github.net:3000/denoland/deno").unwrap()
        ).is_err()
    );

    // Correct ipv4 address + any port should be ok others should err
    assert!(perms.check_net("127.0.0.1").is_ok());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://127.0.0.1").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://127.0.0.1").unwrap())
        .is_ok()
    );
    assert!(perms.check_net("127.0.0.1:3000").is_ok());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://127.0.0.1:3000").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://127.0.0.1:3000").unwrap())
        .is_ok()
    );
    assert!(perms.check_net("127.0.0.2").is_err());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://127.0.0.2").unwrap())
        .is_err()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://127.0.0.2").unwrap())
        .is_err()
    );
    assert!(perms.check_net("127.0.0.2:3000").is_err());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://127.0.0.2:3000").unwrap())
        .is_err()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://127.0.0.2:3000").unwrap())
        .is_err()
    );

    // Correct address + port should be ok all other combinations should err
    assert!(perms.check_net("172.16.0.2:8000").is_ok());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://172.16.0.2:8000").unwrap())
        .is_ok()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://172.16.0.2:8000").unwrap())
        .is_ok()
    );
    assert!(perms.check_net("172.16.0.2").is_err());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://172.16.0.2").unwrap())
        .is_err()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://172.16.0.2").unwrap())
        .is_err()
    );
    assert!(perms.check_net("172.16.0.2:6000").is_err());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://172.16.0.2:6000").unwrap())
        .is_err()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://172.16.0.2:6000").unwrap())
        .is_err()
    );
    assert!(perms.check_net("172.16.0.1:8000").is_err());
    assert!(
      perms
        .check_net_url(url::Url::parse("tcp://172.16.0.1:8000").unwrap())
        .is_err()
    );
    assert!(
      perms
        .check_net_url(url::Url::parse("https://172.16.0.1:8000").unwrap())
        .is_err()
    );

    // Just some random hosts that should err
    assert!(perms.check_net("somedomain").is_err());
    assert!(perms.check_net("192.168.0.1").is_err());
  }
}
