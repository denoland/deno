// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use atty;

use crate::flags::DenoFlags;

use ansi_term::Style;
use crate::errors::permission_denied;
use crate::errors::DenoResult;
use std::fmt;
use std::io;
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
    match val {
      true => PermissionAccessorState::Allow,
      false => PermissionAccessorState::Ask,
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

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
#[derive(Debug, Default)]
pub struct DenoPermissions {
  // Keep in sync with src/permissions.ts
  pub allow_read: PermissionAccessor,
  pub allow_write: PermissionAccessor,
  pub allow_net: PermissionAccessor,
  pub allow_env: PermissionAccessor,
  pub allow_run: PermissionAccessor,
  pub no_prompts: AtomicBool,
}

impl DenoPermissions {
  pub fn from_flags(flags: &DenoFlags) -> Self {
    Self {
      allow_read: PermissionAccessor::from(flags.allow_read),
      allow_write: PermissionAccessor::from(flags.allow_write),
      allow_env: PermissionAccessor::from(flags.allow_env),
      allow_net: PermissionAccessor::from(flags.allow_net),
      allow_run: PermissionAccessor::from(flags.allow_run),
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
      PermissionAccessorState::Ask => match self
        .try_permissions_prompt(&format!("read access to \"{}\"", filename))
      {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_read.update_with_prompt_result(&v);
          v.check()?;
          Ok(())
        }
      },
      PermissionAccessorState::Deny => Err(permission_denied()),
    }
  }

  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    match self.allow_write.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      PermissionAccessorState::Ask => match self
        .try_permissions_prompt(&format!("write access to \"{}\"", filename))
      {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_write.update_with_prompt_result(&v);
          v.check()?;
          Ok(())
        }
      },
      PermissionAccessorState::Deny => Err(permission_denied()),
    }
  }

  pub fn check_net(&self, domain_name: &str) -> DenoResult<()> {
    match self.allow_net.get_state() {
      PermissionAccessorState::Allow => Ok(()),
      PermissionAccessorState::Ask => match self.try_permissions_prompt(
        &format!("network access to \"{}\"", domain_name),
      ) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_net.update_with_prompt_result(&v);
          v.check()?;
          Ok(())
        }
      },
      PermissionAccessorState::Deny => Err(permission_denied()),
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
    return self.allow_run.is_allow();
  }

  pub fn allows_read(&self) -> bool {
    return self.allow_read.is_allow();
  }

  pub fn allows_write(&self) -> bool {
    return self.allow_write.is_allow();
  }

  pub fn allows_net(&self) -> bool {
    return self.allow_net.is_allow();
  }

  pub fn allows_env(&self) -> bool {
    return self.allow_env.is_allow();
  }

  pub fn revoke_run(&self) -> DenoResult<()> {
    self.allow_run.revoke();
    return Ok(());
  }

  pub fn revoke_read(&self) -> DenoResult<()> {
    self.allow_read.revoke();
    return Ok(());
  }

  pub fn revoke_write(&self) -> DenoResult<()> {
    self.allow_write.revoke();
    return Ok(());
  }

  pub fn revoke_net(&self) -> DenoResult<()> {
    self.allow_net.revoke();
    return Ok(());
  }

  pub fn revoke_env(&self) -> DenoResult<()> {
    self.allow_env.revoke();
    return Ok(());
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
