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
use deno_core::OpState;
use log::debug;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
#[cfg(not(test))]
use std::io;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::atomic::Ordering;
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
  #[inline(always)]
  fn log_perm_access(name: &str, info: Option<&str>) {
    debug!(
      "{}",
      colors::bold(&format!(
        "{}️  Granted {}",
        PERMISSION_EMOJI,
        Self::fmt_access(name, info)
      ))
    );
  }

  fn fmt_access(name: &str, info: Option<&str>) -> String {
    format!(
      "{} access{}",
      name,
      info.map_or(String::new(), |info| { format!(" to {}", info) }),
    )
  }

  fn error(name: &str, info: Option<&str>) -> AnyError {
    custom_error(
      "PermissionDenied",
      format!(
        "Requires {}, run again with the --allow-{} flag",
        Self::fmt_access(name, info),
        name
      ),
    )
  }

  /// Check the permission state. bool is whether a prompt was issued.
  fn check(
    self,
    name: &str,
    info: Option<&str>,
    prompt: bool,
  ) -> (Result<(), AnyError>, bool) {
    match self {
      PermissionState::Granted => {
        Self::log_perm_access(name, info);
        (Ok(()), false)
      }
      PermissionState::Prompt if prompt => {
        let msg = Self::fmt_access(name, info);
        if permission_prompt(&msg) {
          Self::log_perm_access(name, info);
          (Ok(()), true)
        } else {
          (Err(Self::error(name, info)), true)
        }
      }
      _ => (Err(Self::error(name, info)), false),
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitPermission {
  pub name: &'static str,
  pub description: &'static str,
  pub state: PermissionState,
  pub prompt: bool,
}

impl UnitPermission {
  pub fn query(&self) -> PermissionState {
    self.state
  }

  pub fn request(&mut self) -> PermissionState {
    if self.state == PermissionState::Prompt {
      if permission_prompt(&format!("access to {}", self.description)) {
        self.state = PermissionState::Granted;
      } else {
        self.state = PermissionState::Denied;
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
    let (result, prompted) = self.state.check(self.name, None, self.prompt);
    if prompted {
      if result.is_ok() {
        self.state = PermissionState::Granted;
      } else {
        self.state = PermissionState::Denied;
      }
    }
    result
  }
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
  #[serde(skip)]
  pub prompt: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct ReadDescriptor(pub PathBuf);

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct WriteDescriptor(pub PathBuf);

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct NetDescriptor(pub String, pub Option<u16>);

impl NetDescriptor {
  fn new<T: AsRef<str>>(host: &&(T, Option<u16>)) -> Self {
    NetDescriptor(host.0.as_ref().to_string(), host.1)
  }

  pub fn from_string(host: String) -> Self {
    let url = url::Url::parse(&format!("http://{}", host)).unwrap();
    let hostname = url.host_str().unwrap().to_string();

    NetDescriptor(hostname, url.port())
  }
}

impl fmt::Display for NetDescriptor {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&match self.1 {
      None => self.0.clone(),
      Some(port) => format!("{}:{}", self.0, port),
    })
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct EnvDescriptor(pub String);

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
pub struct RunDescriptor(pub String);

impl UnaryPermission<ReadDescriptor> {
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
        if permission_prompt(&format!(
          "read access to \"{}\"",
          display_path.display()
        )) {
          self
            .granted_list
            .retain(|path| !path.0.starts_with(&resolved_path));
          self.granted_list.insert(ReadDescriptor(resolved_path));
          PermissionState::Granted
        } else {
          self
            .denied_list
            .retain(|path| !resolved_path.starts_with(&path.0));
          self.denied_list.insert(ReadDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("read access") {
          self.granted_list.clear();
          self.global_state = PermissionState::Granted;
          PermissionState::Granted
        } else {
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
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
    let (result, prompted) = self.query(Some(&resolved_path)).check(
      self.name,
      Some(&format!("\"{}\"", display_path.display())),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(ReadDescriptor(resolved_path));
      } else {
        self.denied_list.insert(ReadDescriptor(resolved_path));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }

  /// As `check()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  pub fn check_blind(
    &mut self,
    path: &Path,
    display: &str,
  ) -> Result<(), AnyError> {
    let resolved_path = resolve_from_cwd(path).unwrap();
    let (result, prompted) = self.query(Some(&resolved_path)).check(
      self.name,
      Some(&format!("<{}>", display)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(ReadDescriptor(resolved_path));
      } else {
        self.denied_list.insert(ReadDescriptor(resolved_path));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }
}

impl UnaryPermission<WriteDescriptor> {
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
        if permission_prompt(&format!(
          "write access to \"{}\"",
          display_path.display()
        )) {
          self
            .granted_list
            .retain(|path| !path.0.starts_with(&resolved_path));
          self.granted_list.insert(WriteDescriptor(resolved_path));
          PermissionState::Granted
        } else {
          self
            .denied_list
            .retain(|path| !resolved_path.starts_with(&path.0));
          self.denied_list.insert(WriteDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("write access") {
          self.granted_list.clear();
          self.global_state = PermissionState::Granted;
          PermissionState::Granted
        } else {
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
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
    let (result, prompted) = self.query(Some(&resolved_path)).check(
      self.name,
      Some(&format!("\"{}\"", display_path.display())),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(WriteDescriptor(resolved_path));
      } else {
        self.denied_list.insert(WriteDescriptor(resolved_path));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }
}

impl UnaryPermission<NetDescriptor> {
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
          Some(_) => self.denied_list.contains(&NetDescriptor::new(host)),
        },
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match host.as_ref() {
        None => false,
        Some(host) => {
          self.granted_list.contains(&NetDescriptor::new(&&(
            host.0.as_ref().to_string(),
            None,
          )))
            || self.granted_list.contains(&NetDescriptor::new(host))
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
        let host = NetDescriptor::new(&host);
        if permission_prompt(&format!("network access to \"{}\"", host)) {
          if host.1.is_none() {
            self.granted_list.retain(|h| h.0 != host.0);
          }
          self.granted_list.insert(host);
          PermissionState::Granted
        } else {
          if host.1.is_some() {
            self.denied_list.remove(&host);
          }
          self.denied_list.insert(host);
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    } else {
      let state = self.query::<&str>(None);
      if state == PermissionState::Prompt {
        if permission_prompt("network access") {
          self.granted_list.clear();
          self.global_state = PermissionState::Granted;
          PermissionState::Granted
        } else {
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
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
      self.granted_list.remove(&NetDescriptor::new(&host));
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
    let new_host = NetDescriptor::new(&host);
    let (result, prompted) = self.query(Some(host)).check(
      self.name,
      Some(&format!("\"{}\"", new_host)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(new_host);
      } else {
        self.denied_list.insert(new_host);
        self.global_state = PermissionState::Denied;
      }
    }
    result
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
    let host = &(&hostname, url.port_or_known_default());
    let (result, prompted) = self.query(Some(host)).check(
      self.name,
      Some(&format!("\"{}\"", display_host)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(NetDescriptor::new(&host));
      } else {
        self.denied_list.insert(NetDescriptor::new(&host));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }
}

impl UnaryPermission<EnvDescriptor> {
  pub fn query(&self, env: Option<&str>) -> PermissionState {
    #[cfg(windows)]
    let env = env.map(|env| env.to_uppercase());
    #[cfg(windows)]
    let env = env.as_deref();
    if self.global_state == PermissionState::Denied
      && match env {
        None => true,
        Some(env) => self.denied_list.iter().any(|env_| env_.0 == env),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match env {
        None => false,
        Some(env) => self.granted_list.iter().any(|env_| env_.0 == env),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, env: Option<&str>) -> PermissionState {
    if let Some(env) = env {
      #[cfg(windows)]
      let env = env.to_uppercase();
      let state = self.query(Some(&env));
      if state == PermissionState::Prompt {
        if permission_prompt(&format!("env access to \"{}\"", env)) {
          self.granted_list.retain(|env_| env_.0 != env);
          self.granted_list.insert(EnvDescriptor(env.to_string()));
          PermissionState::Granted
        } else {
          self.denied_list.retain(|env_| env_.0 != env);
          self.denied_list.insert(EnvDescriptor(env.to_string()));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("env access") {
          self.granted_list.clear();
          self.global_state = PermissionState::Granted;
          PermissionState::Granted
        } else {
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    }
  }

  pub fn revoke(&mut self, env: Option<&str>) -> PermissionState {
    if let Some(env) = env {
      #[cfg(windows)]
      let env = env.to_uppercase();
      self.granted_list.retain(|env_| env_.0 != env);
    } else {
      self.granted_list.clear();
      if self.global_state == PermissionState::Granted {
        self.global_state = PermissionState::Prompt;
      }
    }
    self.query(env)
  }

  pub fn check(&mut self, env: &str) -> Result<(), AnyError> {
    #[cfg(windows)]
    let env = &env.to_uppercase();
    let (result, prompted) = self.query(Some(env)).check(
      self.name,
      Some(&format!("\"{}\"", env)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(EnvDescriptor(env.to_string()));
      } else {
        self.denied_list.insert(EnvDescriptor(env.to_string()));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    let (result, prompted) =
      self.query(None).check(self.name, Some("all"), self.prompt);
    if prompted {
      if result.is_ok() {
        self.global_state = PermissionState::Granted;
      } else {
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }
}

impl UnaryPermission<RunDescriptor> {
  pub fn query(&self, cmd: Option<&str>) -> PermissionState {
    if self.global_state == PermissionState::Denied
      && match cmd {
        None => true,
        Some(cmd) => self.denied_list.iter().any(|cmd_| cmd_.0 == cmd),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match cmd {
        None => false,
        Some(cmd) => self.granted_list.iter().any(|cmd_| cmd_.0 == cmd),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, cmd: Option<&str>) -> PermissionState {
    if let Some(cmd) = cmd {
      let state = self.query(Some(&cmd));
      if state == PermissionState::Prompt {
        if permission_prompt(&format!("run access to \"{}\"", cmd)) {
          self.granted_list.retain(|cmd_| cmd_.0 != cmd);
          self.granted_list.insert(RunDescriptor(cmd.to_string()));
          PermissionState::Granted
        } else {
          self.denied_list.retain(|cmd_| cmd_.0 != cmd);
          self.denied_list.insert(RunDescriptor(cmd.to_string()));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("run access") {
          self.granted_list.clear();
          self.global_state = PermissionState::Granted;
          PermissionState::Granted
        } else {
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else {
        state
      }
    }
  }

  pub fn revoke(&mut self, cmd: Option<&str>) -> PermissionState {
    if let Some(cmd) = cmd {
      self.granted_list.retain(|cmd_| cmd_.0 != cmd);
    } else {
      self.granted_list.clear();
      if self.global_state == PermissionState::Granted {
        self.global_state = PermissionState::Prompt;
      }
    }
    self.query(cmd)
  }

  pub fn check(&mut self, cmd: &str) -> Result<(), AnyError> {
    let (result, prompted) = self.query(Some(cmd)).check(
      self.name,
      Some(&format!("\"{}\"", cmd)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(RunDescriptor(cmd.to_string()));
      } else {
        self.denied_list.insert(RunDescriptor(cmd.to_string()));
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    let (result, prompted) =
      self.query(None).check(self.name, Some("all"), self.prompt);
    if prompted {
      if result.is_ok() {
        self.global_state = PermissionState::Granted;
      } else {
        self.global_state = PermissionState::Denied;
      }
    }
    result
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Permissions {
  pub read: UnaryPermission<ReadDescriptor>,
  pub write: UnaryPermission<WriteDescriptor>,
  pub net: UnaryPermission<NetDescriptor>,
  pub env: UnaryPermission<EnvDescriptor>,
  pub run: UnaryPermission<RunDescriptor>,
  pub plugin: UnitPermission,
  pub hrtime: UnitPermission,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionsOptions {
  pub allow_env: Option<Vec<String>>,
  pub allow_hrtime: bool,
  pub allow_net: Option<Vec<String>>,
  pub allow_plugin: bool,
  pub allow_read: Option<Vec<PathBuf>>,
  pub allow_run: Option<Vec<String>>,
  pub allow_write: Option<Vec<PathBuf>>,
  pub prompt: bool,
}

impl Permissions {
  pub fn new_read(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> UnaryPermission<ReadDescriptor> {
    UnaryPermission::<ReadDescriptor> {
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
  ) -> UnaryPermission<WriteDescriptor> {
    UnaryPermission::<WriteDescriptor> {
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
  ) -> UnaryPermission<NetDescriptor> {
    UnaryPermission::<NetDescriptor> {
      name: "net",
      description: "network",
      global_state: global_state_from_option(state),
      granted_list: state
        .as_ref()
        .map(|v| {
          v.iter()
            .map(|x| NetDescriptor::from_string(x.clone()))
            .collect()
        })
        .unwrap_or_else(HashSet::new),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_env(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> UnaryPermission<EnvDescriptor> {
    UnaryPermission::<EnvDescriptor> {
      name: "env",
      description: "environment variables",
      global_state: global_state_from_option(state),
      granted_list: state
        .as_ref()
        .map(|v| {
          v.iter()
            .map(|x| {
              EnvDescriptor(if cfg!(windows) {
                x.to_uppercase()
              } else {
                x.clone()
              })
            })
            .collect()
        })
        .unwrap_or_else(HashSet::new),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_run(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> UnaryPermission<RunDescriptor> {
    UnaryPermission::<RunDescriptor> {
      name: "run",
      description: "run a subprocess",
      global_state: global_state_from_option(state),
      granted_list: state
        .as_ref()
        .map(|v| v.iter().map(|x| RunDescriptor(x.clone())).collect())
        .unwrap_or_else(HashSet::new),
      denied_list: Default::default(),
      prompt,
    }
  }

  pub fn new_plugin(state: bool, prompt: bool) -> UnitPermission {
    unit_permission_from_flag_bool(state, "plugin", "open a plugin", prompt)
  }

  pub fn new_hrtime(state: bool, prompt: bool) -> UnitPermission {
    unit_permission_from_flag_bool(
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
      env: Permissions::new_env(&opts.allow_env, opts.prompt),
      run: Permissions::new_run(&opts.allow_run, opts.prompt),
      plugin: Permissions::new_plugin(opts.allow_plugin, opts.prompt),
      hrtime: Permissions::new_hrtime(opts.allow_hrtime, opts.prompt),
    }
  }

  pub fn allow_all() -> Self {
    Self {
      read: Permissions::new_read(&Some(vec![]), false),
      write: Permissions::new_write(&Some(vec![]), false),
      net: Permissions::new_net(&Some(vec![]), false),
      env: Permissions::new_env(&Some(vec![]), false),
      run: Permissions::new_run(&Some(vec![]), false),
      plugin: Permissions::new_plugin(true, false),
      hrtime: Permissions::new_hrtime(true, false),
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
      "blob" => Ok(()),
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

impl deno_timers::TimersPermission for Permissions {
  fn allow_hrtime(&mut self) -> bool {
    self.hrtime.check().is_ok()
  }

  fn check_unstable(&self, state: &OpState, api_name: &'static str) {
    crate::ops::check_unstable(state, api_name);
  }
}

impl deno_websocket::WebSocketPermissions for Permissions {
  fn check_net_url(&mut self, url: &url::Url) -> Result<(), AnyError> {
    self.net.check_url(url)
  }
}

fn unit_permission_from_flag_bool(
  flag: bool,
  name: &'static str,
  description: &'static str,
  prompt: bool,
) -> UnitPermission {
  UnitPermission {
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
) -> HashSet<ReadDescriptor> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        ReadDescriptor(resolve_from_cwd(Path::new(&raw_path)).unwrap())
      })
      .collect()
  } else {
    HashSet::new()
  }
}

pub fn resolve_write_allowlist(
  allow: &Option<Vec<PathBuf>>,
) -> HashSet<WriteDescriptor> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        WriteDescriptor(resolve_from_cwd(Path::new(&raw_path)).unwrap())
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
fn permission_prompt(message: &str) -> bool {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    return false;
  };
  let opts = "[g/d (g = grant, d = deny)] ";
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
      return false;
    };
    let ch = match input.chars().next() {
      None => return false,
      Some(v) => v,
    };
    match ch.to_ascii_lowercase() {
      'g' => return true,
      'd' => return false,
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
fn permission_prompt(_message: &str) -> bool {
  STUB_PROMPT_VALUE.load(Ordering::SeqCst)
}

#[cfg(test)]
lazy_static::lazy_static! {
  /// Lock this when you use `set_prompt_result` in a test case.
  static ref PERMISSION_PROMPT_GUARD: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
static STUB_PROMPT_VALUE: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
fn set_prompt_result(value: bool) {
  STUB_PROMPT_VALUE.store(value, Ordering::SeqCst);
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
      env: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_env(&Some(svec!["HOME"]), false)
      },
      run: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_run(&Some(svec!["deno"]), false)
      },
      plugin: UnitPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      hrtime: UnitPermission {
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
      assert_eq!(perms1.env.query(None), PermissionState::Granted);
      assert_eq!(perms1.env.query(Some(&"HOME".to_string())), PermissionState::Granted);
      assert_eq!(perms2.env.query(None), PermissionState::Prompt);
      assert_eq!(perms2.env.query(Some(&"HOME".to_string())), PermissionState::Granted);
      assert_eq!(perms1.run.query(None), PermissionState::Granted);
      assert_eq!(perms1.run.query(Some(&"deno".to_string())), PermissionState::Granted);
      assert_eq!(perms2.run.query(None), PermissionState::Prompt);
      assert_eq!(perms2.run.query(Some(&"deno".to_string())), PermissionState::Granted);
      assert_eq!(perms1.plugin.query(), PermissionState::Granted);
      assert_eq!(perms2.plugin.query(), PermissionState::Prompt);
      assert_eq!(perms1.hrtime.query(), PermissionState::Granted);
      assert_eq!(perms2.hrtime.query(), PermissionState::Prompt);
    };
  }

  #[test]
  fn test_request() {
    let mut perms: Permissions = Default::default();
    #[rustfmt::skip]
    {
      let _guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
      set_prompt_result(true);
      assert_eq!(perms.read.request(Some(&Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms.read.query(None), PermissionState::Prompt);
      set_prompt_result(false);
      assert_eq!(perms.read.request(Some(&Path::new("/foo/bar"))), PermissionState::Granted);
      set_prompt_result(false);
      assert_eq!(perms.write.request(Some(&Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms.write.query(Some(&Path::new("/foo/bar"))), PermissionState::Prompt);
      set_prompt_result(true);
      assert_eq!(perms.write.request(None), PermissionState::Denied);
      set_prompt_result(true);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", None))), PermissionState::Granted);
      set_prompt_result(false);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      set_prompt_result(true);
      assert_eq!(perms.env.request(Some(&"HOME".to_string())), PermissionState::Granted);
      assert_eq!(perms.env.query(None), PermissionState::Prompt);
      set_prompt_result(false);
      assert_eq!(perms.env.request(Some(&"HOME".to_string())), PermissionState::Granted);
      set_prompt_result(true);
      assert_eq!(perms.run.request(Some(&"deno".to_string())), PermissionState::Granted);
      assert_eq!(perms.run.query(None), PermissionState::Prompt);
      set_prompt_result(false);
      assert_eq!(perms.run.request(Some(&"deno".to_string())), PermissionState::Granted);
      set_prompt_result(true);
      assert_eq!(perms.plugin.request(), PermissionState::Granted);
      set_prompt_result(false);
      assert_eq!(perms.plugin.request(), PermissionState::Granted);
      set_prompt_result(false);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
      set_prompt_result(true);
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
      env: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_env(&Some(svec!["HOME"]), false)
      },
      run: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_run(&Some(svec!["deno"]), false)
      },
      plugin: UnitPermission {
        state: PermissionState::Prompt,
        ..Default::default()
      },
      hrtime: UnitPermission {
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
      assert_eq!(perms.env.revoke(Some(&"HOME".to_string())), PermissionState::Prompt);
      assert_eq!(perms.run.revoke(Some(&"deno".to_string())), PermissionState::Prompt);
      assert_eq!(perms.plugin.revoke(), PermissionState::Prompt);
      assert_eq!(perms.hrtime.revoke(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_check() {
    let mut perms = Permissions {
      read: Permissions::new_read(&None, true),
      write: Permissions::new_write(&None, true),
      net: Permissions::new_net(&None, true),
      env: Permissions::new_env(&None, true),
      run: Permissions::new_run(&None, true),
      plugin: Permissions::new_plugin(false, true),
      hrtime: Permissions::new_hrtime(false, true),
    };

    let _guard = PERMISSION_PROMPT_GUARD.lock().unwrap();

    set_prompt_result(true);
    assert!(perms.read.check(&Path::new("/foo")).is_ok());
    set_prompt_result(false);
    assert!(perms.read.check(&Path::new("/foo")).is_ok());
    assert!(perms.read.check(&Path::new("/bar")).is_err());

    set_prompt_result(true);
    assert!(perms.write.check(&Path::new("/foo")).is_ok());
    set_prompt_result(false);
    assert!(perms.write.check(&Path::new("/foo")).is_ok());
    assert!(perms.write.check(&Path::new("/bar")).is_err());

    set_prompt_result(true);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_ok());
    set_prompt_result(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_ok());
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_err());
    assert!(perms.net.check(&("127.0.0.1", None)).is_err());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_err());
    assert!(perms.net.check(&("deno.land", None)).is_err());

    set_prompt_result(true);
    assert!(perms.run.check("cat").is_ok());
    set_prompt_result(false);
    assert!(perms.run.check("cat").is_ok());
    assert!(perms.run.check("ls").is_err());

    set_prompt_result(true);
    assert!(perms.env.check("HOME").is_ok());
    set_prompt_result(false);
    assert!(perms.env.check("HOME").is_ok());
    assert!(perms.env.check("PATH").is_err());

    set_prompt_result(true);
    assert!(perms.hrtime.check().is_ok());
    set_prompt_result(false);
    assert!(perms.hrtime.check().is_ok());
  }

  #[test]
  fn test_check_fail() {
    let mut perms = Permissions {
      read: Permissions::new_read(&None, true),
      write: Permissions::new_write(&None, true),
      net: Permissions::new_net(&None, true),
      env: Permissions::new_env(&None, true),
      run: Permissions::new_run(&None, true),
      plugin: Permissions::new_plugin(false, true),
      hrtime: Permissions::new_hrtime(false, true),
    };

    let _guard = PERMISSION_PROMPT_GUARD.lock().unwrap();

    set_prompt_result(false);
    assert!(perms.read.check(&Path::new("/foo")).is_err());
    set_prompt_result(true);
    assert!(perms.read.check(&Path::new("/foo")).is_err());
    assert!(perms.read.check(&Path::new("/bar")).is_ok());
    set_prompt_result(false);
    assert!(perms.read.check(&Path::new("/bar")).is_ok());

    set_prompt_result(false);
    assert!(perms.write.check(&Path::new("/foo")).is_err());
    set_prompt_result(true);
    assert!(perms.write.check(&Path::new("/foo")).is_err());
    assert!(perms.write.check(&Path::new("/bar")).is_ok());
    set_prompt_result(false);
    assert!(perms.write.check(&Path::new("/bar")).is_ok());

    set_prompt_result(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_err());
    set_prompt_result(true);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_err());
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_ok());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_ok());
    set_prompt_result(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_ok());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_ok());

    set_prompt_result(false);
    assert!(perms.run.check("cat").is_err());
    set_prompt_result(true);
    assert!(perms.run.check("cat").is_err());
    assert!(perms.run.check("ls").is_ok());
    set_prompt_result(false);
    assert!(perms.run.check("ls").is_ok());

    set_prompt_result(false);
    assert!(perms.env.check("HOME").is_err());
    set_prompt_result(true);
    assert!(perms.env.check("HOME").is_err());
    assert!(perms.env.check("PATH").is_ok());
    set_prompt_result(false);
    assert!(perms.env.check("PATH").is_ok());

    set_prompt_result(false);
    assert!(perms.hrtime.check().is_err());
    set_prompt_result(true);
    assert!(perms.hrtime.check().is_err());
  }

  #[test]
  #[cfg(windows)]
  fn test_env_windows() {
    let mut perms = Permissions::allow_all();
    perms.env = UnaryPermission {
      global_state: PermissionState::Prompt,
      ..Permissions::new_env(&Some(svec!["HOME"]), false)
    };

    set_prompt_result(true);
    assert!(perms.env.check("HOME").is_ok());
    set_prompt_result(false);
    assert!(perms.env.check("HOME").is_ok());
    assert!(perms.env.check("hOmE").is_ok());

    assert_eq!(
      perms.env.revoke(Some(&"HomE".to_string())),
      PermissionState::Prompt
    );
  }
}
