// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fs_util::resolve_from_cwd;
use deno_core::error::custom_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::url;
use deno_core::ModuleSpecifier;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
#[cfg(not(test))]
use std::io;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Mutex;

const PERMISSION_EMOJI: &str = "⚠️";

/// Tri-state value for storing permission state
#[derive(PartialEq, Debug, Clone, Copy, Deserialize, PartialOrd)]
pub enum PermissionState {
  Granted = 0,
  Prompt = 1,
  Denied = 2,
}

impl PermissionState {
  /// Check the permission state. Errors if denied.
  /// Ok value is whether PromptResult is AllowAlways.
  /// first Err value is whether PromptResult is DenyAlways.
  fn check(
    self,
    name: &str,
    info: Option<&str>,
    prompt: bool,
  ) -> Result<bool, (bool, AnyError)> {
    let access = format!(
      "{} access{}",
      name,
      info.map_or(Default::default(), |info| { format!(" to {}", info) }),
    );
    let mut e_result = false;
    if self == PermissionState::Granted {
      log_perm_access(&access);
      Ok(true)
    } else {
      if prompt && self == PermissionState::Prompt {
        match permission_prompt(&access) {
          PromptResult::AllowAlways => {
            log_perm_access(&access);
            return Ok(true);
          }
          PromptResult::AllowOnce => {
            log_perm_access(&access);
            return Ok(false);
          }
          PromptResult::DenyOnce => e_result = false,
          PromptResult::DenyAlways => e_result = true,
        }
      }

      let message = format!(
        "Requires {}, run again with the --allow-{} flag",
        access, name
      );
      Err((e_result, custom_error("PermissionDenied", message)))
    }
  }
}

impl fmt::Display for PermissionState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PermissionState::Granted => f.pad("granted"),
      PermissionState::Prompt => f.pad("prompt"),
      PermissionState::Denied => f.pad("denied"),
    }
  }
}

impl Default for PermissionState {
  fn default() -> Self {
    PermissionState::Prompt
  }
}

#[derive(Debug, Clone)]
pub enum PromptResult {
  AllowAlways,
  AllowOnce,
  DenyOnce,
  DenyAlways,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct UnaryPermission<T: Eq + Hash> {
  #[serde(skip)]
  pub name: &'static str,
  #[serde(skip)]
  pub description: &'static str,
  pub global_state: PermissionState,
  pub granted_list: HashSet<T>,
  pub denied_list: HashSet<T>,
  pub prompt: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct ReadPermission(pub PathBuf);

impl UnaryPermission<ReadPermission> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    let path = path.map(|p| resolve_from_cwd(p).unwrap());
    if self.global_state == PermissionState::Denied
      && match path.as_ref() {
        None => true,
        Some(path) => self
          .denied_list
          .iter()
          .any(|path_| path_.0.starts_with(path)),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match path.as_ref() {
        None => false,
        Some(path) => self
          .granted_list
          .iter()
          .any(|path_| path.starts_with(&path_.0)),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, path: Option<&Path>) -> PermissionState {
    if let Some(path) = path {
      let (resolved_path, display_path) = resolved_and_display_path(path);
      let state = self.query(Some(&resolved_path));
      if state == PermissionState::Prompt {
        match permission_prompt(&format!(
          "read access to \"{}\"",
          display_path.display()
        )) {
          PromptResult::AllowAlways => {
            self
              .granted_list
              .retain(|path| !path.0.starts_with(&resolved_path));
            self.granted_list.insert(ReadPermission(resolved_path));
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            self
              .denied_list
              .retain(|path| !resolved_path.starts_with(&path.0));
            self.denied_list.insert(ReadPermission(resolved_path));
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        match permission_prompt("read access") {
          PromptResult::AllowAlways => {
            self.granted_list.clear();
            self.global_state = PermissionState::Granted;
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    }
  }

  pub fn revoke(&mut self, path: Option<&Path>) -> PermissionState {
    if let Some(path) = path {
      let path = resolve_from_cwd(path).unwrap();
      self
        .granted_list
        .retain(|path_| !path_.0.starts_with(&path));
    } else {
      self.granted_list.clear();
      if self.global_state == PermissionState::Granted {
        self.global_state = PermissionState::Prompt;
      }
    }
    self.query(path)
  }

  fn base_check(&mut self, path: PathBuf, info: &str) -> Result<(), AnyError> {
    match self
      .query(Some(&path))
      .check(self.name, Some(info), self.prompt)
    {
      Ok(always) => {
        if always {
          self.granted_list.insert(ReadPermission(path));
        }
        Ok(())
      }
      Err((always, e)) => {
        if always {
          self.denied_list.insert(ReadPermission(path));
          self.global_state = PermissionState::Denied;
        }
        Err(e)
      }
    }
  }

  pub fn check(&mut self, path: &Path) -> Result<(), AnyError> {
    let (resolved_path, display_path) = resolved_and_display_path(path);
    self.base_check(resolved_path, &format!("\"{}\"", display_path.display()))
  }

  /// As `check()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  pub fn check_blind(
    &mut self,
    path: &Path,
    display: &str,
  ) -> Result<(), AnyError> {
    let resolved_path = resolve_from_cwd(path).unwrap();
    self.base_check(resolved_path, &format!("<{}>", display))
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct WritePermission(pub PathBuf);

impl UnaryPermission<WritePermission> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    let path = path.map(|p| resolve_from_cwd(p).unwrap());
    if self.global_state == PermissionState::Denied
      && match path.as_ref() {
        None => true,
        Some(path) => self
          .denied_list
          .iter()
          .any(|path_| path_.0.starts_with(path)),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match path.as_ref() {
        None => false,
        Some(path) => self
          .granted_list
          .iter()
          .any(|path_| path.starts_with(&path_.0)),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, path: Option<&Path>) -> PermissionState {
    if let Some(path) = path {
      let (resolved_path, display_path) = resolved_and_display_path(path);
      let state = self.query(Some(&resolved_path));
      if state == PermissionState::Prompt {
        match permission_prompt(&format!(
          "write access to \"{}\"",
          display_path.display()
        )) {
          PromptResult::AllowAlways => {
            self
              .granted_list
              .retain(|path| !path.0.starts_with(&resolved_path));
            self.granted_list.insert(WritePermission(resolved_path));
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            self
              .denied_list
              .retain(|path| !resolved_path.starts_with(path.0.clone()));
            self.denied_list.insert(WritePermission(resolved_path));
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        match permission_prompt("write access") {
          PromptResult::AllowAlways => {
            self.granted_list.clear();
            self.global_state = PermissionState::Granted;
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    }
  }

  pub fn revoke(&mut self, path: Option<&Path>) -> PermissionState {
    if let Some(path) = path {
      let path = resolve_from_cwd(path).unwrap();
      self
        .granted_list
        .retain(|path_| !path_.0.starts_with(&path));
    } else {
      self.granted_list.clear();
      if self.global_state == PermissionState::Granted {
        self.global_state = PermissionState::Prompt;
      }
    }
    self.query(path)
  }

  pub fn check(&mut self, path: &Path) -> Result<(), AnyError> {
    let (resolved_path, display_path) = resolved_and_display_path(path);
    match self.query(Some(&resolved_path)).check(
      self.name,
      Some(&format!("\"{}\"", display_path.display())),
      self.prompt,
    ) {
      Ok(always) => {
        if always {
          self.granted_list.insert(WritePermission(resolved_path));
        }
        Ok(())
      }
      Err((always, e)) => {
        if always {
          self.denied_list.insert(WritePermission(resolved_path));
          self.global_state = PermissionState::Denied;
        }
        Err(e)
      }
    }
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct NetPermission(pub String, pub Option<u16>);

impl NetPermission {
  fn new<T: AsRef<str>>(host: &&(T, Option<u16>)) -> Self {
    NetPermission(host.0.as_ref().to_string(), host.1)
  }

  pub fn from_string(host: String) -> Self {
    let url = url::Url::parse(&format!("http://{}", host)).unwrap();
    let hostname = url.host_str().unwrap().to_string();

    NetPermission(hostname, url.port())
  }
}

impl fmt::Display for NetPermission {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&match self.1 {
      None => self.0.clone(),
      Some(port) => format!("{}:{}", self.0, port),
    })
  }
}

impl UnaryPermission<NetPermission> {
  pub fn query<T: AsRef<str>>(
    &self,
    host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    if self.global_state == PermissionState::Denied
      && match host.as_ref() {
        None => true,
        Some(host) => match host.1 {
          None => self
            .denied_list
            .iter()
            .any(|host_| host.0.as_ref() == host_.0),
          Some(_) => self.denied_list.contains(&NetPermission::new(host)),
        },
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match host.as_ref() {
        None => false,
        Some(host) => {
          self.granted_list.contains(&NetPermission::new(&&(
            host.0.as_ref().to_string(),
            None,
          )))
            || self.granted_list.contains(&NetPermission::new(host))
        }
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request<T: AsRef<str>>(
    &mut self,
    host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    if let Some(host) = host {
      let state = self.query(Some(host));
      if state == PermissionState::Prompt {
        let host = NetPermission::new(&host);
        match permission_prompt(&format!("network access to \"{}\"", host)) {
          PromptResult::AllowAlways => {
            if host.1.is_none() {
              self.granted_list.retain(|h| h.0 != host.0);
            }
            self.granted_list.insert(host);
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            if host.1.is_some() {
              self.denied_list.remove(&host);
            }
            self.denied_list.insert(host);
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    } else {
      let state = self.query::<&str>(None);
      if state == PermissionState::Prompt {
        match permission_prompt("network access") {
          PromptResult::AllowAlways => {
            self.granted_list.clear();
            self.global_state = PermissionState::Granted;
            PermissionState::Granted
          }
          PromptResult::AllowOnce => PermissionState::Granted,
          PromptResult::DenyOnce => PermissionState::Denied,
          PromptResult::DenyAlways => {
            self.global_state = PermissionState::Denied;
            PermissionState::Denied
          }
        }
      } else {
        state
      }
    }
  }

  pub fn revoke<T: AsRef<str>>(
    &mut self,
    host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    if let Some(host) = host {
      self.granted_list.remove(&NetPermission::new(&host));
      if host.1.is_none() {
        self.granted_list.retain(|h| h.0 != host.0.as_ref());
      }
    } else {
      self.granted_list.clear();
      if self.global_state == PermissionState::Granted {
        self.global_state = PermissionState::Prompt;
      }
    }
    self.query(host)
  }

  pub fn check<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
  ) -> Result<(), AnyError> {
    let new_host = NetPermission::new(&host);
    match self.query(Some(host)).check(
      self.name,
      Some(&format!("\"{}\"", new_host)),
      self.prompt,
    ) {
      Ok(always) => {
        if always {
          self.granted_list.insert(new_host);
        }
        Ok(())
      }
      Err((always, e)) => {
        if always {
          self.denied_list.insert(new_host);
          self.global_state = PermissionState::Denied;
        }
        Err(e)
      }
    }
  }

  pub fn check_url(&mut self, url: &url::Url) -> Result<(), AnyError> {
    let hostname = url
      .host_str()
      .ok_or_else(|| uri_error("Missing host"))?
      .to_string();
    let display_host = match url.port() {
      None => hostname.clone(),
      Some(port) => format!("{}:{}", hostname, port),
    };

    match self
      .query(Some(&(&hostname, url.port_or_known_default())))
      .check(
        self.name,
        Some(&format!("\"{}\"", display_host)),
        self.prompt,
      ) {
      Ok(always) => {
        if always {
          self.granted_list.insert(NetPermission::new(&&(
            hostname,
            url.port_or_known_default(),
          )));
        }
        Ok(())
      }
      Err((always, e)) => {
        if always {
          self.denied_list.insert(NetPermission::new(&&(
            hostname,
            url.port_or_known_default(),
          )));
          self.global_state = PermissionState::Denied;
        }
        Err(e)
      }
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BooleanPermission {
  pub name: &'static str,
  pub description: &'static str,
  pub state: PermissionState,
  pub prompt: bool,
}

impl BooleanPermission {
  pub fn query(&self) -> PermissionState {
    self.state
  }

  pub fn request(&mut self) -> PermissionState {
    if self.state == PermissionState::Prompt {
      match permission_prompt(&format!("access to {}", self.description)) {
        PromptResult::AllowAlways => self.state = PermissionState::Granted,
        PromptResult::AllowOnce => return PermissionState::Granted,
        PromptResult::DenyOnce => return PermissionState::Denied,
        PromptResult::DenyAlways => self.state = PermissionState::Denied,
      }
    }
    self.state
  }

  pub fn revoke(&mut self) -> PermissionState {
    if self.state == PermissionState::Granted {
      self.state = PermissionState::Prompt;
    }
    self.state
  }

  pub fn check(&mut self) -> Result<(), AnyError> {
    match self.state.check(self.name, None, self.prompt) {
      Ok(always) => {
        if always {
          self.state = PermissionState::Granted;
        }
        Ok(())
      }
      Err((always, e)) => {
        if always {
          self.state = PermissionState::Denied;
        }
        Err(e)
      }
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Permissions {
  pub read: UnaryPermission<ReadPermission>,
  pub write: UnaryPermission<WritePermission>,
  pub net: UnaryPermission<NetPermission>,
  pub env: BooleanPermission,
  pub run: BooleanPermission,
  pub plugin: BooleanPermission,
  pub hrtime: BooleanPermission,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionsOptions {
  pub allow_env: bool,
  pub allow_hrtime: bool,
  pub allow_net: Option<Vec<String>>,
  pub allow_plugin: bool,
  pub allow_read: Option<Vec<PathBuf>>,
  pub allow_run: bool,
  pub allow_write: Option<Vec<PathBuf>>,
  pub prompt: bool,
}

impl Permissions {
  pub fn new_read(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> UnaryPermission<ReadPermission> {
    UnaryPermission::<ReadPermission> {
      name: "read",
      description: "read the file system",
      global_state: global_state_from_option(state),
      granted_list: resolve_read_allowlist(&state),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_write(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> UnaryPermission<WritePermission> {
    UnaryPermission::<WritePermission> {
      name: "write",
      description: "write to the file system",
      global_state: global_state_from_option(state),
      granted_list: resolve_write_allowlist(&state),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_net(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> UnaryPermission<NetPermission> {
    UnaryPermission::<NetPermission> {
      name: "net",
      description: "network",
      global_state: global_state_from_option(state),
      granted_list: state
        .as_ref()
        .map(|v| {
          v.iter()
            .map(|x| NetPermission::from_string(x.clone()))
            .collect()
        })
        .unwrap_or_else(HashSet::new),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_env(state: bool, prompt: bool) -> BooleanPermission {
    boolean_permission_from_flag_bool(
      state,
      "env",
      "environment variables",
      prompt,
    )
  }

  pub fn new_run(state: bool, prompt: bool) -> BooleanPermission {
    boolean_permission_from_flag_bool(state, "run", "run a subprocess", prompt)
  }

  pub fn new_plugin(state: bool, prompt: bool) -> BooleanPermission {
    boolean_permission_from_flag_bool(state, "plugin", "open a plugin", prompt)
  }

  pub fn new_hrtime(state: bool, prompt: bool) -> BooleanPermission {
    boolean_permission_from_flag_bool(
      state,
      "hrtime",
      "high precision time",
      prompt,
    )
  }

  pub fn from_options(opts: &PermissionsOptions) -> Self {
    Self {
      read: Permissions::new_read(&opts.allow_read, opts.prompt),
      write: Permissions::new_write(&opts.allow_write, opts.prompt),
      net: Permissions::new_net(&opts.allow_net, opts.prompt),
      env: Permissions::new_env(opts.allow_env, opts.prompt),
      run: Permissions::new_run(opts.allow_run, opts.prompt),
      plugin: Permissions::new_plugin(opts.allow_plugin, opts.prompt),
      hrtime: Permissions::new_hrtime(opts.allow_hrtime, opts.prompt),
    }
  }

  pub fn allow_all() -> Self {
    Self {
      read: Permissions::new_read(&Some(vec![]), false),
      write: Permissions::new_write(&Some(vec![]), false),
      net: Permissions::new_net(&Some(vec![]), false),
      env: Permissions::new_env(true, false),
      run: Permissions::new_run(true, false),
      plugin: Permissions::new_plugin(true, false),
      hrtime: Permissions::new_hrtime(true, false),
    }
  }

  pub fn prompt() -> Self {
    Self {
      read: Permissions::new_read(&None, true),
      write: Permissions::new_write(&None, true),
      net: Permissions::new_net(&None, true),
      env: Permissions::new_env(false, true),
      run: Permissions::new_run(false, true),
      plugin: Permissions::new_plugin(false, true),
      hrtime: Permissions::new_hrtime(false, true),
    }
  }

  /// A helper function that determines if the module specifier is a local or
  /// remote, and performs a read or net check for the specifier.
  pub fn check_specifier(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    match specifier.scheme() {
      "file" => match specifier.to_file_path() {
        Ok(path) => self.read.check(&path),
        Err(_) => Err(uri_error(format!(
          "Invalid file path.\n  Specifier: {}",
          specifier
        ))),
      },
      "data" => Ok(()),
      _ => self.net.check_url(specifier),
    }
  }
}

impl deno_fetch::FetchPermissions for Permissions {
  fn check_net_url(&mut self, url: &url::Url) -> Result<(), AnyError> {
    self.net.check_url(url)
  }

  fn check_read(&mut self, path: &Path) -> Result<(), AnyError> {
    self.read.check(path)
  }
}

impl deno_websocket::WebSocketPermissions for Permissions {
  fn check_net_url(&mut self, url: &url::Url) -> Result<(), AnyError> {
    self.net.check_url(url)
  }
}

fn log_perm_access(message: &str) {
  debug!(
    "{}",
    colors::bold(&format!("{}️  Granted {}", PERMISSION_EMOJI, message))
  );
}

fn boolean_permission_from_flag_bool(
  flag: bool,
  name: &'static str,
  description: &'static str,
  prompt: bool,
) -> BooleanPermission {
  BooleanPermission {
    name,
    description,
    state: if flag {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    },
    prompt,
  }
}

fn global_state_from_option<T>(flag: &Option<Vec<T>>) -> PermissionState {
  if matches!(flag, Some(v) if v.is_empty()) {
    PermissionState::Granted
  } else {
    PermissionState::Prompt
  }
}

pub fn resolve_read_allowlist(
  allow: &Option<Vec<PathBuf>>,
) -> HashSet<ReadPermission> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        ReadPermission(resolve_from_cwd(Path::new(&raw_path)).unwrap())
      })
      .collect()
  } else {
    HashSet::new()
  }
}

pub fn resolve_write_allowlist(
  allow: &Option<Vec<PathBuf>>,
) -> HashSet<WritePermission> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        WritePermission(resolve_from_cwd(Path::new(&raw_path)).unwrap())
      })
      .collect()
  } else {
    HashSet::new()
  }
}

/// Arbitrary helper. Resolves the path from CWD, and also gets a path that
/// can be displayed without leaking the CWD when not allowed.
fn resolved_and_display_path(path: &Path) -> (PathBuf, PathBuf) {
  let resolved_path = resolve_from_cwd(path).unwrap();
  let display_path = path.to_path_buf();
  (resolved_path, display_path)
}

/// Shows the permission prompt and returns the answer according to the user input.
/// This loops until the user gives the proper input.
#[cfg(not(test))]
fn permission_prompt(message: &str) -> PromptResult {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    return PromptResult::DenyAlways;
  };
  let opts = "[a/y/n/d (a = allow always, y = allow once, n = deny once, d = deny always)] ";
  let msg = format!(
    "{}  ️Deno requests {}. Grant? {}",
    PERMISSION_EMOJI, message, opts
  );
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{}", colors::bold(&msg));
  loop {
    let mut input = String::new();
    let stdin = io::stdin();
    let result = stdin.read_line(&mut input);
    if result.is_err() {
      return PromptResult::DenyOnce;
    };
    let ch = input.chars().next().unwrap();
    match ch.to_ascii_lowercase() {
      'a' => return PromptResult::AllowAlways,
      'y' => return PromptResult::AllowOnce,
      'n' => return PromptResult::DenyOnce,
      'd' => return PromptResult::DenyAlways,
      _ => {
        // If we don't get a recognized option try again.
        let msg_again = format!("Unrecognized option '{}' {}", ch, opts);
        eprint!("{}", colors::bold(&msg_again));
      }
    };
  }
}

// When testing, permission prompt returns the value of STUB_PROMPT_VALUE
// which we set from the test functions.
#[cfg(test)]
fn permission_prompt(_message: &str) -> PromptResult {
  (*STUB_PROMPT_VALUE).lock().unwrap().clone()
}

#[cfg(test)]
lazy_static! {
  static ref STUB_PROMPT_VALUE: Mutex<PromptResult> =
    Mutex::new(PromptResult::AllowAlways);
}

#[cfg(test)]
fn set_prompt_result(value: PromptResult) {
  *(*STUB_PROMPT_VALUE).lock().unwrap() = value;
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url_or_path;

  // Creates vector of strings, Vec<String>
  macro_rules! svec {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
  }

  #[test]
  fn check_paths() {
    let allowlist = vec![
      PathBuf::from("/a/specific/dir/name"),
      PathBuf::from("/a/specific"),
      PathBuf::from("/b/c"),
    ];

    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_read: Some(allowlist.clone()),
      allow_write: Some(allowlist),
      ..Default::default()
    });

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms.read.check(Path::new("/a/specific/dir/name")).is_ok());
    assert!(perms.write.check(Path::new("/a/specific/dir/name")).is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.read.check(Path::new("/a/specific/dir")).is_ok());
    assert!(perms.write.check(Path::new("/a/specific/dir")).is_ok());

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms
      .read
      .check(Path::new("/a/specific/dir/name/inner"))
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/dir/name/inner"))
      .is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.read.check(Path::new("/a/specific/other/dir")).is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/other/dir"))
      .is_ok());

    // Exact match with /b/c
    assert!(perms.read.check(Path::new("/b/c")).is_ok());
    assert!(perms.write.check(Path::new("/b/c")).is_ok());

    // Sub path within /b/c
    assert!(perms.read.check(Path::new("/b/c/sub/path")).is_ok());
    assert!(perms.write.check(Path::new("/b/c/sub/path")).is_ok());

    // Sub path within /b/c, needs normalizing
    assert!(perms
      .read
      .check(Path::new("/b/c/sub/path/../path/."))
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/b/c/sub/path/../path/."))
      .is_ok());

    // Inside of /b but outside of /b/c
    assert!(perms.read.check(Path::new("/b/e")).is_err());
    assert!(perms.write.check(Path::new("/b/e")).is_err());

    // Inside of /a but outside of /a/specific
    assert!(perms.read.check(Path::new("/a/b")).is_err());
    assert!(perms.write.check(Path::new("/a/b")).is_err());
  }

  #[test]
  fn test_check_net_with_values() {
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_net: Some(svec![
        "localhost",
        "deno.land",
        "github.com:3000",
        "127.0.0.1",
        "172.16.0.2:8000",
        "www.github.com:443"
      ]),
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

    for (host, port, is_ok) in domain_tests {
      assert_eq!(is_ok, perms.net.check(&(host, Some(port))).is_ok());
    }
  }

  #[test]
  fn test_check_net_only_flag() {
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_net: Some(svec![]), // this means `--allow-net` is present without values following `=` sign
      ..Default::default()
    });

    let domain_tests = vec![
      ("localhost", 1234),
      ("deno.land", 0),
      ("deno.land", 3000),
      ("deno.lands", 0),
      ("deno.lands", 3000),
      ("github.com", 3000),
      ("github.com", 0),
      ("github.com", 2000),
      ("github.net", 3000),
      ("127.0.0.1", 0),
      ("127.0.0.1", 3000),
      ("127.0.0.2", 0),
      ("127.0.0.2", 3000),
      ("172.16.0.2", 8000),
      ("172.16.0.2", 0),
      ("172.16.0.2", 6000),
      ("172.16.0.1", 8000),
      ("somedomain", 0),
      ("192.168.0.1", 0),
    ];

    for (host, port) in domain_tests {
      assert!(perms.net.check(&(host, Some(port))).is_ok());
    }
  }

  #[test]
  fn test_check_net_no_flag() {
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_net: None,
      ..Default::default()
    });

    let domain_tests = vec![
      ("localhost", 1234),
      ("deno.land", 0),
      ("deno.land", 3000),
      ("deno.lands", 0),
      ("deno.lands", 3000),
      ("github.com", 3000),
      ("github.com", 0),
      ("github.com", 2000),
      ("github.net", 3000),
      ("127.0.0.1", 0),
      ("127.0.0.1", 3000),
      ("127.0.0.2", 0),
      ("127.0.0.2", 3000),
      ("172.16.0.2", 8000),
      ("172.16.0.2", 0),
      ("172.16.0.2", 6000),
      ("172.16.0.1", 8000),
      ("somedomain", 0),
      ("192.168.0.1", 0),
    ];

    for (host, port) in domain_tests {
      assert!(!perms.net.check(&(host, Some(port))).is_ok());
    }
  }

  #[test]
  fn test_check_net_url() {
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_net: Some(svec![
        "localhost",
        "deno.land",
        "github.com:3000",
        "127.0.0.1",
        "172.16.0.2:8000",
        "www.github.com:443"
      ]),
      ..Default::default()
    });

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
      // Testing issue #6531 (Network permissions check doesn't account for well-known default ports) so we dont regress
      ("https://www.github.com:443/robots.txt", true),
    ];

    for (url_str, is_ok) in url_tests {
      let u = url::Url::parse(url_str).unwrap();
      assert_eq!(is_ok, perms.net.check_url(&u).is_ok());
    }
  }

  #[test]
  fn check_specifiers() {
    let read_allowlist = if cfg!(target_os = "windows") {
      vec![PathBuf::from("C:\\a")]
    } else {
      vec![PathBuf::from("/a")]
    };
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_read: Some(read_allowlist),
      allow_net: Some(svec!["localhost"]),
      ..Default::default()
    });

    let mut fixtures = vec![
      (
        resolve_url_or_path("http://localhost:4545/mod.ts").unwrap(),
        true,
      ),
      (
        resolve_url_or_path("http://deno.land/x/mod.ts").unwrap(),
        false,
      ),
      (
        resolve_url_or_path("data:text/plain,Hello%2C%20Deno!").unwrap(),
        true,
      ),
    ];

    if cfg!(target_os = "windows") {
      fixtures
        .push((resolve_url_or_path("file:///C:/a/mod.ts").unwrap(), true));
      fixtures
        .push((resolve_url_or_path("file:///C:/b/mod.ts").unwrap(), false));
    } else {
      fixtures.push((resolve_url_or_path("file:///a/mod.ts").unwrap(), true));
      fixtures.push((resolve_url_or_path("file:///b/mod.ts").unwrap(), false));
    }

    for (specifier, expected) in fixtures {
      assert_eq!(perms.check_specifier(&specifier).is_ok(), expected);
    }
  }

  #[test]
  fn check_invalid_specifiers() {
    let mut perms = Permissions::allow_all();

    let mut test_cases = vec![];

    if cfg!(target_os = "windows") {
      test_cases.push("file://");
      test_cases.push("file:///");
    } else {
      test_cases.push("file://remotehost/");
    }

    for url in test_cases {
      assert!(perms
        .check_specifier(&resolve_url_or_path(url).unwrap())
        .is_err());
    }
  }

  #[test]
  fn test_query() {
    let perms1 = Permissions::allow_all();
    let perms2 = Permissions {
      read: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_read(&Some(vec![PathBuf::from("/foo")]), false)
      },
      write: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_write(&Some(vec![PathBuf::from("/foo")]), false)
      },
      net: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_net(&Some(svec!["127.0.0.1:8000"]), false)
      },
      env: BooleanPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      run: BooleanPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      plugin: BooleanPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      hrtime: BooleanPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms1.read.query(None), PermissionState::Granted);
      assert_eq!(perms1.read.query(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(None), PermissionState::Prompt);
      assert_eq!(perms2.read.query(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms1.write.query(None), PermissionState::Granted);
      assert_eq!(perms1.write.query(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(None), PermissionState::Prompt);
      assert_eq!(perms2.write.query(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms1.net.query::<&str>(None), PermissionState::Granted);
      assert_eq!(perms1.net.query(Some(&("127.0.0.1", None))), PermissionState::Granted);
      assert_eq!(perms2.net.query::<&str>(None), PermissionState::Prompt);
      assert_eq!(perms2.net.query(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      assert_eq!(perms1.env.query(), PermissionState::Granted);
      assert_eq!(perms2.env.query(), PermissionState::Prompt);
      assert_eq!(perms1.run.query(), PermissionState::Granted);
      assert_eq!(perms2.run.query(), PermissionState::Prompt);
      assert_eq!(perms1.plugin.query(), PermissionState::Granted);
      assert_eq!(perms2.plugin.query(), PermissionState::Prompt);
      assert_eq!(perms1.hrtime.query(), PermissionState::Granted);
      assert_eq!(perms2.hrtime.query(), PermissionState::Prompt);
    };
  }

  #[test]
  fn test_prompt_fallback() {
    let mut perms = Permissions::prompt();

    {
      set_prompt_result(PromptResult::AllowAlways);
      assert!(perms.read.check(&Path::new("/foo")).is_ok());
      set_prompt_result(PromptResult::DenyOnce); // doesnt prompt
      assert!(perms.read.check(&Path::new("/foo")).is_ok());
    }
    {
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.read.check(&Path::new("/bar")).is_ok());
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.read.check(&Path::new("/bar")).is_err());
    }
    {
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.read.check(&Path::new("/foo1")).is_err());
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.read.check(&Path::new("/foo1")).is_ok());
    }
    {
      set_prompt_result(PromptResult::DenyAlways);
      assert!(perms.read.check(&Path::new("/bar1")).is_err());
      set_prompt_result(PromptResult::AllowOnce); // doesnt prompt
      assert!(perms.read.check(&Path::new("/bar1")).is_err());
    }

    {
      set_prompt_result(PromptResult::AllowAlways);
      assert!(perms.write.check(&Path::new("/foo")).is_ok());
      set_prompt_result(PromptResult::DenyOnce); // doesnt prompt
      assert!(perms.write.check(&Path::new("/foo")).is_ok());
    }
    {
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.write.check(&Path::new("/bar")).is_ok());
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.write.check(&Path::new("/bar")).is_err());
    }
    {
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.write.check(&Path::new("/foo1")).is_err());
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.write.check(&Path::new("/foo1")).is_ok());
    }
    {
      set_prompt_result(PromptResult::DenyAlways);
      assert!(perms.write.check(&Path::new("/bar1")).is_err());
      set_prompt_result(PromptResult::AllowOnce); // doesnt prompt
      assert!(perms.write.check(&Path::new("/bar1")).is_err());
    }

    {
      set_prompt_result(PromptResult::AllowAlways);
      assert!(perms.net.check(&("localhost", Some(1234))).is_ok());
      set_prompt_result(PromptResult::DenyOnce); // doesnt prompt
      assert!(perms.net.check(&("localhost", Some(1234))).is_ok());
    }
    {
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.net.check(&("deno.land", Some(1234))).is_ok());
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.net.check(&("deno.land", Some(1234))).is_err());
    }
    {
      set_prompt_result(PromptResult::DenyOnce);
      assert!(perms.net.check(&("foo", Some(1234))).is_err());
      set_prompt_result(PromptResult::AllowOnce);
      assert!(perms.net.check(&("foo", Some(1234))).is_ok());
    }
    {
      set_prompt_result(PromptResult::DenyAlways);
      assert!(perms.net.check(&("bar", Some(1234))).is_err());
      set_prompt_result(PromptResult::AllowOnce); // doesnt prompt
      assert!(perms.net.check(&("bar", Some(1234))).is_err());
    }
  }

  #[test]
  fn test_prompt_fallback_unit_allow_always() {
    let mut perms = Permissions::prompt();
    set_prompt_result(PromptResult::AllowAlways);
    assert!(perms.env.check().is_ok());
    set_prompt_result(PromptResult::DenyOnce); // doesnt prompt
    assert!(perms.env.check().is_ok());
  }

  #[test]
  fn test_prompt_fallback_unit_allow_once() {
    let mut perms = Permissions::prompt();
    set_prompt_result(PromptResult::AllowOnce);
    assert!(perms.env.check().is_ok());
    set_prompt_result(PromptResult::DenyOnce);
    assert!(perms.env.check().is_err());
  }

  #[test]
  fn test_prompt_fallback_unit_deny_once() {
    let mut perms = Permissions::prompt();
    set_prompt_result(PromptResult::DenyOnce);
    assert!(perms.env.check().is_err());
    set_prompt_result(PromptResult::AllowOnce);
    assert!(perms.env.check().is_ok());
  }

  #[test]
  fn test_prompt_fallback_unit_deny_always() {
    let mut perms = Permissions::prompt();
    set_prompt_result(PromptResult::DenyAlways);
    assert!(perms.env.check().is_err());
    set_prompt_result(PromptResult::AllowOnce); // doesnt prompt
    assert!(perms.env.check().is_err());
  }

  #[test]
  fn test_request() {
    let mut perms: Permissions = Default::default();
    #[rustfmt::skip]
    {
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.read.request(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms.read.query(None), PermissionState::Prompt);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.read.request(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.write.request(Some(&Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms.write.query(Some(&Path::new("/foo/bar"))), PermissionState::Prompt);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.write.request(None), PermissionState::Denied);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", None))), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.env.request(), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.env.request(), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.run.request(), PermissionState::Denied);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.run.request(), PermissionState::Denied);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.plugin.request(), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.plugin.request(), PermissionState::Granted);
      set_prompt_result(PromptResult::DenyAlways);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
      set_prompt_result(PromptResult::AllowAlways);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_revoke() {
    let mut perms = Permissions {
      read: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_read(&Some(vec![PathBuf::from("/foo")]), false)
      },
      write: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_write(&Some(vec![PathBuf::from("/foo")]), false)
      },
      net: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_net(&Some(svec!["127.0.0.1"]), false)
      },
      env: BooleanPermission {
        state: PermissionState::Granted,
        ..Default::default()
      },
      run: BooleanPermission {
        state: PermissionState::Granted,
        ..Default::default()
      },
      plugin: BooleanPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      hrtime: BooleanPermission {
        state: PermissionState::Denied,
        ..Default::default()
      },
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms.read.revoke(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms.read.revoke(Some(&Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.read.query(Some(&Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.write.revoke(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms.write.revoke(None), PermissionState::Prompt);
      assert_eq!(perms.write.query(Some(&Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.net.revoke(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      assert_eq!(perms.net.revoke(Some(&("127.0.0.1", None))), PermissionState::Prompt);
      assert_eq!(perms.env.revoke(), PermissionState::Prompt);
      assert_eq!(perms.run.revoke(), PermissionState::Prompt);
      assert_eq!(perms.plugin.revoke(), PermissionState::Prompt);
      assert_eq!(perms.hrtime.revoke(), PermissionState::Denied);
    };
  }
}
