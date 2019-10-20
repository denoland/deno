// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::permission_denied;
use crate::flags::DenoFlags;
use ansi_term::Style;
use atty;
use deno::ErrBox;
use log;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

  pub fn deny(&self) {
    self.set_state(PermissionAccessorState::Deny)
  }

  /// Update this accessors state based on a PromptResult value
  pub fn update_with_prompt_result(&self, prompt_result: &PromptResult) {
    match prompt_result {
      PromptResult::Allow => self.allow(),
      PromptResult::Deny => self.deny(),
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

  pub fn request_run(&self) -> Result<(), ErrBox> {
    let msg = "access to run a subprocess";

    match self.allow_run.get_state() {
      PermissionAccessorState::Ask => match self.try_permissions_prompt(msg) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_run.update_with_prompt_result(&v);
          self.log_perm_access(msg);
          Ok(())
        }
      },
      _ => Ok(()),
    }
  }

  pub fn request_read(&self, filename: &Option<&str>) -> Result<(), ErrBox> {
    let msg = &match filename {
      None => "read access".to_string(),
      Some(filename) => format!("read access to \"{}\"", filename),
    };
    match self.allow_read.get_state() {
      PermissionAccessorState::Ask => match self.try_permissions_prompt(msg) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_read.update_with_prompt_result(&v);
          self.log_perm_access(msg);
          Ok(())
        }
      },
      _ => Ok(()),
    }
  }

  pub fn request_write(&self, filename: &Option<&str>) -> Result<(), ErrBox> {
    let msg = &match filename {
      None => "write access".to_string(),
      Some(filename) => format!("write access to \"{}\"", filename),
    };
    match self.allow_write.get_state() {
      PermissionAccessorState::Ask => match self.try_permissions_prompt(msg) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_write.update_with_prompt_result(&v);
          self.log_perm_access(msg);
          Ok(())
        }
      },
      _ => Ok(()),
    }
  }

  pub fn request_net(
    &self,
    host_and_port: &Option<&str>,
  ) -> Result<(), ErrBox> {
    let msg = &match host_and_port {
      None => "network access".to_string(),
      Some(host_and_port) => format!("network access to \"{}\"", host_and_port),
    };
    match self.allow_net.get_state() {
      PermissionAccessorState::Ask => self.request_net_inner(msg),
      _ => Ok(()),
    }
  }

  pub fn request_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    let msg = &format!("network access to \"{}\"", url);
    match self.allow_net.get_state() {
      PermissionAccessorState::Ask => self.request_net_inner(msg),
      _ => Ok(()),
    }
  }

  fn request_net_inner(&self, prompt_str: &str) -> Result<(), ErrBox> {
    match self.try_permissions_prompt(prompt_str) {
      Err(e) => Err(e),
      Ok(v) => {
        self.allow_net.update_with_prompt_result(&v);
        self.log_perm_access(prompt_str);
        Ok(())
      }
    }
  }

  pub fn request_env(&self) -> Result<(), ErrBox> {
    let msg = "access to environment variables";
    match self.allow_env.get_state() {
      PermissionAccessorState::Ask => match self.try_permissions_prompt(msg) {
        Err(e) => Err(e),
        Ok(v) => {
          self.allow_env.update_with_prompt_result(&v);
          self.log_perm_access(msg);
          Ok(())
        }
      },
      _ => Ok(()),
    }
  }

  pub fn request_hrtime(&self) -> Result<(), ErrBox> {
    let msg = "use high resolution time";

    if let PermissionAccessorState::Ask = self.allow_hrtime.get_state() {
      let v = self.try_permissions_prompt(msg)?;
      self.allow_hrtime.update_with_prompt_result(&v);
      self.log_perm_access(msg);
    }
    Ok(())
  }

  pub fn check_run(&self) -> Result<(), ErrBox> {
    if self.allows_run() {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_read(&self, filename: &str) -> Result<(), ErrBox> {
    if self.allows_read(filename) {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_write(&self, filename: &str) -> Result<(), ErrBox> {
    if self.allows_write(filename) {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_net(&self, host_and_port: &str) -> Result<(), ErrBox> {
    if self.allows_net(host_and_port) {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    if self.allows_net_url(url) {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_env(&self) -> Result<(), ErrBox> {
    if self.allows_env() {
      return Ok(());
    }
    Err(permission_denied())
  }

  pub fn check_hrtime(&self) -> Result<(), ErrBox> {
    if self.allows_hrtime() {
      return Ok(());
    }
    Err(permission_denied())
  }

  /// Try to present the user with a permission prompt
  fn try_permissions_prompt(
    &self,
    message: &str,
  ) -> Result<PromptResult, ErrBox> {
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
      // TODO: should be other kind of error
      // For example, "Permission request is not available in non tty environment
      // Use cli flags instead."
      return Err(permission_denied());
    };
    permission_prompt(message)
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

  fn read_state(&self, filename: &Option<&str>) -> PermissionAccessorState {
    let state = self.allow_read.get_state();
    if state == PermissionAccessorState::Ask
      && check_path_white_list(filename, &self.read_whitelist)
    {
      return PermissionAccessorState::Allow;
    }
    state
  }

  fn write_state(&self, filename: &Option<&str>) -> PermissionAccessorState {
    let state = self.allow_write.get_state();
    if state == PermissionAccessorState::Ask
      && check_path_white_list(filename, &self.write_whitelist)
    {
      return PermissionAccessorState::Allow;
    }
    state
  }

  fn net_state(&self, host_and_port: &Option<&str>) -> PermissionAccessorState {
    let state = self.allow_net.get_state();
    if state == PermissionAccessorState::Ask
      && check_host_and_port_whitelist(host_and_port, &self.net_whitelist)
    {
      return PermissionAccessorState::Allow;
    }
    state
  }

  fn net_state_url(&self, url: &url::Url) -> PermissionAccessorState {
    let state = self.allow_net.get_state();
    if state == PermissionAccessorState::Ask
      && check_url_whitelist(url, &self.net_whitelist)
    {
      return PermissionAccessorState::Allow;
    }
    state
  }

  pub fn allows_run(&self) -> bool {
    self.allow_run.is_allow()
  }

  pub fn allows_read(&self, filename: &str) -> bool {
    self.read_state(&Some(filename)) == PermissionAccessorState::Allow
  }

  pub fn allows_write(&self, filename: &str) -> bool {
    self.write_state(&Some(filename)) == PermissionAccessorState::Allow
  }

  pub fn allows_net(&self, host_and_port: &str) -> bool {
    self.net_state(&Some(host_and_port)) == PermissionAccessorState::Allow
  }

  pub fn allows_net_url(&self, url: &url::Url) -> bool {
    self.net_state_url(url) == PermissionAccessorState::Allow
  }

  pub fn allows_env(&self) -> bool {
    self.allow_env.is_allow()
  }

  pub fn allows_hrtime(&self) -> bool {
    self.allow_hrtime.is_allow()
  }

  pub fn revoke_run(&self) {
    self.allow_run.revoke();
  }

  pub fn revoke_read(&self) {
    self.allow_read.revoke();
  }

  pub fn revoke_write(&self) {
    self.allow_write.revoke();
  }

  pub fn revoke_net(&self) {
    self.allow_net.revoke();
  }

  pub fn revoke_env(&self) {
    self.allow_env.revoke();
  }
  pub fn revoke_hrtime(&self) {
    self.allow_hrtime.revoke();
  }

  pub fn get_permission_state(
    &self,
    name: &str,
    url: &Option<&str>,
    path: &Option<&str>,
  ) -> Result<PermissionAccessorState, ErrBox> {
    match name {
      "run" => Ok(self.allow_run.get_state()),
      "read" => Ok(self.read_state(path)),
      "write" => Ok(self.write_state(path)),
      "net" => Ok(self.net_state(url)),
      "env" => Ok(self.allow_env.get_state()),
      "hrtime" => Ok(self.allow_hrtime.get_state()),
      _ => Err(permission_denied()), // TODO: should be TypeError
    }
  }
}

/// 2-state value for representing user input on permission prompt
#[derive(Debug, Clone)]
pub enum PromptResult {
  Allow = 0,
  Deny = 1,
}

impl fmt::Display for PromptResult {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PromptResult::Allow => f.pad("Allow"),
      PromptResult::Deny => f.pad("Deny"),
    }
  }
}

fn permission_prompt(message: &str) -> Result<PromptResult, ErrBox> {
  let msg = format!(
    "️{}  Deno requests {}. Grant? [a/d (a = allow, d = deny)] ",
    PERMISSION_EMOJI, message
  );
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{}", Style::new().bold().paint(msg));
  loop {
    let mut input = String::new();
    let stdin = io::stdin();
    let _nread = stdin.read_line(&mut input)?;
    let ch = input.chars().next().unwrap();
    match ch.to_ascii_lowercase() {
      'a' => return Ok(PromptResult::Allow),
      'd' => return Ok(PromptResult::Deny),
      _ => {
        // If we don't get a recognized option try again.
        let msg_again =
          format!("Unrecognized option '{}' [a/d (a = allow, d = deny)] ", ch);
        eprint!("{}", Style::new().bold().paint(msg_again));
      }
    };
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
  host_and_port: &Option<&str>,
  whitelist: &Arc<HashSet<String>>,
) -> bool {
  if host_and_port.is_none() {
    return false;
  }
  let host_and_port = host_and_port.unwrap();
  let parts = host_and_port.split(':').collect::<Vec<&str>>();
  match parts.len() {
    2 => {
      whitelist.contains(parts[0])
        || whitelist.contains(&format!("{}:{}", parts[0], parts[1]))
    }
    1 => whitelist.contains(parts[0]),
    _ => panic!("Failed to parse origin string: {}", host_and_port),
  }
}

fn check_url_whitelist(
  url: &url::Url,
  whitelist: &Arc<HashSet<String>>,
) -> bool {
  let host = url.host().unwrap();
  whitelist.contains(&format!("{}", host))
    || match url.port() {
      Some(port) => whitelist.contains(&format!("{}:{}", host, port)),
      None => false,
    }
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
      ("localhost:1234", true),
      ("deno.land", true),
      ("deno.land:3000", true),
      ("deno.lands", false),
      ("deno.lands:3000", false),
      ("github.com:3000", true),
      ("github.com", false),
      ("github.com:2000", false),
      ("github.net:3000", false),
      ("127.0.0.1", true),
      ("127.0.0.1:3000", true),
      ("127.0.0.2", false),
      ("127.0.0.2:3000", false),
      ("172.16.0.2:8000", true),
      ("172.16.0.2", false),
      ("172.16.0.2:6000", false),
      ("172.16.0.1:8000", false),
      // Just some random hosts that should err
      ("somedomain", false),
      ("192.168.0.1", false),
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

    for (domain, is_ok) in domain_tests.iter() {
      assert_eq!(*is_ok, perms.check_net(domain).is_ok());
    }
  }
}
