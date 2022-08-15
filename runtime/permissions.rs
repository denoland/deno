// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fs_util::resolve_from_cwd;
use deno_core::error::custom_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
#[cfg(test)]
use deno_core::parking_lot::Mutex;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::url;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use log;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::atomic::Ordering;

const PERMISSION_EMOJI: &str = "⚠️";

static DEBUG_LOG_ENABLED: Lazy<bool> =
  Lazy::new(|| log::log_enabled!(log::Level::Debug));

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
    // Eliminates log overhead (when logging is disabled),
    // log_enabled!(Debug) check in a hot path still has overhead
    // TODO(AaronO): generalize or upstream this optimization
    if *DEBUG_LOG_ENABLED {
      log::debug!(
        "{}",
        colors::bold(&format!(
          "{}️  Granted {}",
          PERMISSION_EMOJI,
          Self::fmt_access(name, info)
        ))
      );
    }
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
        if permission_prompt(&msg, name) {
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

#[derive(Clone, Debug, PartialEq)]
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
      if permission_prompt(
        &format!("access to {}", self.description),
        self.name,
      ) {
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

/// A normalized environment variable name. On Windows this will
/// be uppercase and on other platforms it will stay as-is.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct EnvVarName {
  inner: String,
}

impl EnvVarName {
  pub fn new(env: impl AsRef<str>) -> Self {
    Self {
      inner: if cfg!(windows) {
        env.as_ref().to_uppercase()
      } else {
        env.as_ref().to_string()
      },
    }
  }
}

impl AsRef<str> for EnvVarName {
  fn as_ref(&self) -> &str {
    self.inner.as_str()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnaryPermission<T: Eq + Hash> {
  pub name: &'static str,
  pub description: &'static str,
  pub global_state: PermissionState,
  pub granted_list: HashSet<T>,
  pub denied_list: HashSet<T>,
  pub prompt: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ReadDescriptor(pub PathBuf);

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WriteDescriptor(pub PathBuf);

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NetDescriptor(pub String, pub Option<u16>);

impl NetDescriptor {
  fn new<T: AsRef<str>>(host: &&(T, Option<u16>)) -> Self {
    NetDescriptor(host.0.as_ref().to_string(), host.1)
  }
}

impl FromStr for NetDescriptor {
  type Err = AnyError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let url = url::Url::parse(&format!("http://{s}"))?;
    let hostname = url.host_str().unwrap().to_string();

    Ok(NetDescriptor(hostname, url.port()))
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

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct EnvDescriptor(EnvVarName);

impl EnvDescriptor {
  pub fn new(env: impl AsRef<str>) -> Self {
    Self(EnvVarName::new(env))
  }
}

impl AsRef<str> for EnvDescriptor {
  fn as_ref(&self) -> &str {
    self.0.as_ref()
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum RunDescriptor {
  Name(String),
  Path(PathBuf),
}

impl FromStr for RunDescriptor {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let is_path = s.contains('/');
    #[cfg(windows)]
    let is_path = is_path || s.contains('\\') || Path::new(s).is_absolute();
    if is_path {
      Ok(Self::Path(resolve_from_cwd(Path::new(s)).unwrap()))
    } else {
      Ok(Self::Name(s.to_string()))
    }
  }
}

impl ToString for RunDescriptor {
  fn to_string(&self) -> String {
    match self {
      RunDescriptor::Name(s) => s.clone(),
      RunDescriptor::Path(p) => p.to_string_lossy().to_string(),
    }
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FfiDescriptor(pub PathBuf);

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
        if permission_prompt(
          &format!("read access to \"{}\"", display_path.display()),
          self.name,
        ) {
          self.granted_list.insert(ReadDescriptor(resolved_path));
          PermissionState::Granted
        } else {
          self.denied_list.insert(ReadDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self.granted_list.insert(ReadDescriptor(resolved_path));
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("read access", self.name) {
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
        .retain(|path_| !path.starts_with(&path_.0));
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
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

impl Default for UnaryPermission<ReadDescriptor> {
  fn default() -> Self {
    UnaryPermission::<ReadDescriptor> {
      name: "read",
      description: "read the file system",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
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
        if permission_prompt(
          &format!("write access to \"{}\"", display_path.display()),
          self.name,
        ) {
          self.granted_list.insert(WriteDescriptor(resolved_path));
          PermissionState::Granted
        } else {
          self.denied_list.insert(WriteDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self.granted_list.insert(WriteDescriptor(resolved_path));
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("write access", self.name) {
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
        .retain(|path_| !path.starts_with(&path_.0));
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
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

impl Default for UnaryPermission<WriteDescriptor> {
  fn default() -> Self {
    UnaryPermission::<WriteDescriptor> {
      name: "write",
      description: "write to the file system",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
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
      let host = NetDescriptor::new(&host);
      if state == PermissionState::Prompt {
        if permission_prompt(
          &format!("network access to \"{}\"", host),
          self.name,
        ) {
          self.granted_list.insert(host);
          PermissionState::Granted
        } else {
          self.denied_list.insert(host);
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self.granted_list.insert(host);
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query::<&str>(None);
      if state == PermissionState::Prompt {
        if permission_prompt("network access", self.name) {
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
      if host.1.is_some() {
        self
          .granted_list
          .remove(&NetDescriptor(host.0.as_ref().to_string(), host.1));
      }
      self
        .granted_list
        .remove(&NetDescriptor(host.0.as_ref().to_string(), None));
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
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

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    let (result, prompted) =
      self
        .query::<&str>(None)
        .check(self.name, Some("all"), self.prompt);
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

impl Default for UnaryPermission<NetDescriptor> {
  fn default() -> Self {
    UnaryPermission::<NetDescriptor> {
      name: "net",
      description: "network",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
  }
}

impl UnaryPermission<EnvDescriptor> {
  pub fn query(&self, env: Option<&str>) -> PermissionState {
    let env = env.map(EnvVarName::new);
    if self.global_state == PermissionState::Denied
      && match env.as_ref() {
        None => true,
        Some(env) => self.denied_list.contains(&EnvDescriptor::new(env)),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match env.as_ref() {
        None => false,
        Some(env) => self.granted_list.contains(&EnvDescriptor::new(env)),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, env: Option<&str>) -> PermissionState {
    if let Some(env) = env {
      let state = self.query(Some(env));
      if state == PermissionState::Prompt {
        if permission_prompt(&format!("env access to \"{}\"", env), self.name) {
          self.granted_list.insert(EnvDescriptor::new(env));
          PermissionState::Granted
        } else {
          self.denied_list.insert(EnvDescriptor::new(env));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self.granted_list.insert(EnvDescriptor::new(env));
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("env access", self.name) {
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
      self.granted_list.remove(&EnvDescriptor::new(env));
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
    }
    self.query(env)
  }

  pub fn check(&mut self, env: &str) -> Result<(), AnyError> {
    let (result, prompted) = self.query(Some(env)).check(
      self.name,
      Some(&format!("\"{}\"", env)),
      self.prompt,
    );
    if prompted {
      if result.is_ok() {
        self.granted_list.insert(EnvDescriptor::new(env));
      } else {
        self.denied_list.insert(EnvDescriptor::new(env));
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

impl Default for UnaryPermission<EnvDescriptor> {
  fn default() -> Self {
    UnaryPermission::<EnvDescriptor> {
      name: "env",
      description: "environment variables",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
  }
}

impl UnaryPermission<RunDescriptor> {
  pub fn query(&self, cmd: Option<&str>) -> PermissionState {
    if self.global_state == PermissionState::Denied
      && match cmd {
        None => true,
        Some(cmd) => self
          .denied_list
          .contains(&RunDescriptor::from_str(cmd).unwrap()),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match cmd {
        None => false,
        Some(cmd) => self
          .granted_list
          .contains(&RunDescriptor::from_str(cmd).unwrap()),
      }
    {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    }
  }

  pub fn request(&mut self, cmd: Option<&str>) -> PermissionState {
    if let Some(cmd) = cmd {
      let state = self.query(Some(cmd));
      if state == PermissionState::Prompt {
        if permission_prompt(&format!("run access to \"{}\"", cmd), self.name) {
          self
            .granted_list
            .insert(RunDescriptor::from_str(cmd).unwrap());
          PermissionState::Granted
        } else {
          self
            .denied_list
            .insert(RunDescriptor::from_str(cmd).unwrap());
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self
          .granted_list
          .insert(RunDescriptor::from_str(cmd).unwrap());
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("run access", self.name) {
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
      self
        .granted_list
        .remove(&RunDescriptor::from_str(cmd).unwrap());
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
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
        self
          .granted_list
          .insert(RunDescriptor::from_str(cmd).unwrap());
      } else {
        self
          .denied_list
          .insert(RunDescriptor::from_str(cmd).unwrap());
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

impl Default for UnaryPermission<RunDescriptor> {
  fn default() -> Self {
    UnaryPermission::<RunDescriptor> {
      name: "run",
      description: "run a subprocess",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
  }
}

impl UnaryPermission<FfiDescriptor> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    let path = path.map(|p| resolve_from_cwd(p).unwrap());
    if self.global_state == PermissionState::Denied
      && match path.as_ref() {
        None => true,
        Some(path) => self.denied_list.contains(&FfiDescriptor(path.clone())),
      }
    {
      PermissionState::Denied
    } else if self.global_state == PermissionState::Granted
      || match path.as_ref() {
        None => false,
        Some(path) => self.granted_list.contains(&FfiDescriptor(path.clone())),
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
        if permission_prompt(
          &format!("ffi access to \"{}\"", display_path.display()),
          self.name,
        ) {
          self.granted_list.insert(FfiDescriptor(resolved_path));
          PermissionState::Granted
        } else {
          self.denied_list.insert(FfiDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
          PermissionState::Denied
        }
      } else if state == PermissionState::Granted {
        self.granted_list.insert(FfiDescriptor(resolved_path));
        PermissionState::Granted
      } else {
        state
      }
    } else {
      let state = self.query(None);
      if state == PermissionState::Prompt {
        if permission_prompt("ffi access", self.name) {
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
      self.granted_list.remove(&FfiDescriptor(path));
    } else {
      self.granted_list.clear();
    }
    if self.global_state == PermissionState::Granted {
      self.global_state = PermissionState::Prompt;
    }
    self.query(path)
  }

  pub fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    if let Some(path) = path {
      let (resolved_path, display_path) = resolved_and_display_path(path);
      let (result, prompted) = self.query(Some(&resolved_path)).check(
        self.name,
        Some(&format!("\"{}\"", display_path.display())),
        self.prompt,
      );

      if prompted {
        if result.is_ok() {
          self.granted_list.insert(FfiDescriptor(resolved_path));
        } else {
          self.denied_list.insert(FfiDescriptor(resolved_path));
          self.global_state = PermissionState::Denied;
        }
      }

      result
    } else {
      let (result, prompted) =
        self.query(None).check(self.name, None, self.prompt);

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

impl Default for UnaryPermission<FfiDescriptor> {
  fn default() -> Self {
    UnaryPermission::<FfiDescriptor> {
      name: "ffi",
      description: "load a dynamic library",
      global_state: Default::default(),
      granted_list: Default::default(),
      denied_list: Default::default(),
      prompt: false,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permissions {
  pub read: UnaryPermission<ReadDescriptor>,
  pub write: UnaryPermission<WriteDescriptor>,
  pub net: UnaryPermission<NetDescriptor>,
  pub env: UnaryPermission<EnvDescriptor>,
  pub run: UnaryPermission<RunDescriptor>,
  pub ffi: UnaryPermission<FfiDescriptor>,
  pub hrtime: UnitPermission,
}

impl Default for Permissions {
  fn default() -> Self {
    Self {
      read: Permissions::new_read(&None, false).unwrap(),
      write: Permissions::new_write(&None, false).unwrap(),
      net: Permissions::new_net(&None, false).unwrap(),
      env: Permissions::new_env(&None, false).unwrap(),
      run: Permissions::new_run(&None, false).unwrap(),
      ffi: Permissions::new_ffi(&None, false).unwrap(),
      hrtime: Permissions::new_hrtime(false),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionsOptions {
  pub allow_env: Option<Vec<String>>,
  pub allow_hrtime: bool,
  pub allow_net: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<PathBuf>>,
  pub allow_read: Option<Vec<PathBuf>>,
  pub allow_run: Option<Vec<String>>,
  pub allow_write: Option<Vec<PathBuf>>,
  pub prompt: bool,
}

impl Permissions {
  pub fn new_read(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> Result<UnaryPermission<ReadDescriptor>, AnyError> {
    Ok(UnaryPermission::<ReadDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: resolve_read_allowlist(state)?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_write(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> Result<UnaryPermission<WriteDescriptor>, AnyError> {
    Ok(UnaryPermission::<WriteDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: resolve_write_allowlist(state)?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_net(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> Result<UnaryPermission<NetDescriptor>, AnyError> {
    Ok(UnaryPermission::<NetDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: state.as_ref().map_or_else(
        || Ok(HashSet::new()),
        |v| {
          v.iter()
            .map(|x| NetDescriptor::from_str(x))
            .collect::<Result<HashSet<NetDescriptor>, AnyError>>()
        },
      )?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_env(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> Result<UnaryPermission<EnvDescriptor>, AnyError> {
    Ok(UnaryPermission::<EnvDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: state.as_ref().map_or_else(
        || Ok(HashSet::new()),
        |v| {
          v.iter()
            .map(|x| {
              if x.is_empty() {
                Err(AnyError::msg("Empty path is not allowed"))
              } else {
                Ok(EnvDescriptor::new(x))
              }
            })
            .collect()
        },
      )?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_run(
    state: &Option<Vec<String>>,
    prompt: bool,
  ) -> Result<UnaryPermission<RunDescriptor>, AnyError> {
    Ok(UnaryPermission::<RunDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: state.as_ref().map_or_else(
        || Ok(HashSet::new()),
        |v| {
          v.iter()
            .map(|x| {
              if x.is_empty() {
                Err(AnyError::msg("Empty path is not allowed"))
              } else {
                Ok(RunDescriptor::from_str(x).unwrap())
              }
            })
            .collect()
        },
      )?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_ffi(
    state: &Option<Vec<PathBuf>>,
    prompt: bool,
  ) -> Result<UnaryPermission<FfiDescriptor>, AnyError> {
    Ok(UnaryPermission::<FfiDescriptor> {
      global_state: global_state_from_option(state),
      granted_list: resolve_ffi_allowlist(state)?,
      prompt,
      ..Default::default()
    })
  }

  pub fn new_hrtime(state: bool) -> UnitPermission {
    unit_permission_from_flag_bool(
      state,
      "hrtime",
      "high precision time",
      false, // never prompt for hrtime
    )
  }

  pub fn from_options(opts: &PermissionsOptions) -> Result<Self, AnyError> {
    Ok(Self {
      read: Permissions::new_read(&opts.allow_read, opts.prompt)?,
      write: Permissions::new_write(&opts.allow_write, opts.prompt)?,
      net: Permissions::new_net(&opts.allow_net, opts.prompt)?,
      env: Permissions::new_env(&opts.allow_env, opts.prompt)?,
      run: Permissions::new_run(&opts.allow_run, opts.prompt)?,
      ffi: Permissions::new_ffi(&opts.allow_ffi, opts.prompt)?,
      hrtime: Permissions::new_hrtime(opts.allow_hrtime),
    })
  }

  pub fn allow_all() -> Self {
    Self {
      read: Permissions::new_read(&Some(vec![]), false).unwrap(),
      write: Permissions::new_write(&Some(vec![]), false).unwrap(),
      net: Permissions::new_net(&Some(vec![]), false).unwrap(),
      env: Permissions::new_env(&Some(vec![]), false).unwrap(),
      run: Permissions::new_run(&Some(vec![]), false).unwrap(),
      ffi: Permissions::new_ffi(&Some(vec![]), false).unwrap(),
      hrtime: Permissions::new_hrtime(true),
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

impl deno_net::NetPermissions for Permissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
  ) -> Result<(), AnyError> {
    self.net.check(host)
  }

  fn check_read(&mut self, path: &Path) -> Result<(), AnyError> {
    self.read.check(path)
  }

  fn check_write(&mut self, path: &Path) -> Result<(), AnyError> {
    self.write.check(path)
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

impl deno_web::TimersPermission for Permissions {
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

impl deno_ffi::FfiPermissions for Permissions {
  fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    self.ffi.check(path)
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
) -> Result<HashSet<ReadDescriptor>, AnyError> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        if raw_path.as_os_str().is_empty() {
          Err(AnyError::msg("Empty path is not allowed"))
        } else {
          resolve_from_cwd(Path::new(&raw_path)).map(ReadDescriptor)
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

pub fn resolve_write_allowlist(
  allow: &Option<Vec<PathBuf>>,
) -> Result<HashSet<WriteDescriptor>, AnyError> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        if raw_path.as_os_str().is_empty() {
          Err(AnyError::msg("Empty path is not allowed"))
        } else {
          resolve_from_cwd(Path::new(&raw_path)).map(WriteDescriptor)
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

pub fn resolve_ffi_allowlist(
  allow: &Option<Vec<PathBuf>>,
) -> Result<HashSet<FfiDescriptor>, AnyError> {
  if let Some(v) = allow {
    v.iter()
      .map(|raw_path| {
        if raw_path.as_os_str().is_empty() {
          Err(AnyError::msg("Empty path is not allowed"))
        } else {
          resolve_from_cwd(Path::new(&raw_path)).map(FfiDescriptor)
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

/// Arbitrary helper. Resolves the path from CWD, and also gets a path that
/// can be displayed without leaking the CWD when not allowed.
fn resolved_and_display_path(path: &Path) -> (PathBuf, PathBuf) {
  let resolved_path = resolve_from_cwd(path).unwrap();
  let display_path = path.to_path_buf();
  (resolved_path, display_path)
}

fn escalation_error() -> AnyError {
  custom_error(
    "PermissionDenied",
    "Can't escalate parent thread permissions",
  )
}

#[derive(Debug, PartialEq)]
pub enum ChildUnitPermissionArg {
  Inherit,
  Granted,
  NotGranted,
}

impl<'de> Deserialize<'de> for ChildUnitPermissionArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildUnitPermissionArgVisitor;
    impl<'de> de::Visitor<'de> for ChildUnitPermissionArgVisitor {
      type Value = ChildUnitPermissionArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or boolean")
      }

      fn visit_unit<E>(self) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        Ok(ChildUnitPermissionArg::NotGranted)
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildUnitPermissionArg::Inherit)
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_bool<E>(self, v: bool) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        match v {
          true => Ok(ChildUnitPermissionArg::Granted),
          false => Ok(ChildUnitPermissionArg::NotGranted),
        }
      }
    }
    deserializer.deserialize_any(ChildUnitPermissionArgVisitor)
  }
}

#[derive(Debug, PartialEq)]
pub enum ChildUnaryPermissionArg {
  Inherit,
  Granted,
  NotGranted,
  GrantedList(Vec<String>),
}

impl<'de> Deserialize<'de> for ChildUnaryPermissionArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildUnaryPermissionArgVisitor;
    impl<'de> de::Visitor<'de> for ChildUnaryPermissionArgVisitor {
      type Value = ChildUnaryPermissionArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or boolean or string[]")
      }

      fn visit_unit<E>(self) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        Ok(ChildUnaryPermissionArg::NotGranted)
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildUnaryPermissionArg::Inherit)
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_bool<E>(self, v: bool) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        match v {
          true => Ok(ChildUnaryPermissionArg::Granted),
          false => Ok(ChildUnaryPermissionArg::NotGranted),
        }
      }

      fn visit_seq<V>(
        self,
        mut v: V,
      ) -> Result<ChildUnaryPermissionArg, V::Error>
      where
        V: de::SeqAccess<'de>,
      {
        let mut granted_list = vec![];
        while let Some(value) = v.next_element::<String>()? {
          granted_list.push(value);
        }
        Ok(ChildUnaryPermissionArg::GrantedList(granted_list))
      }
    }
    deserializer.deserialize_any(ChildUnaryPermissionArgVisitor)
  }
}

/// Directly deserializable from JS worker and test permission options.
#[derive(Debug, PartialEq)]
pub struct ChildPermissionsArg {
  env: ChildUnaryPermissionArg,
  hrtime: ChildUnitPermissionArg,
  net: ChildUnaryPermissionArg,
  ffi: ChildUnaryPermissionArg,
  read: ChildUnaryPermissionArg,
  run: ChildUnaryPermissionArg,
  write: ChildUnaryPermissionArg,
}

impl ChildPermissionsArg {
  pub fn inherit() -> Self {
    ChildPermissionsArg {
      env: ChildUnaryPermissionArg::Inherit,
      hrtime: ChildUnitPermissionArg::Inherit,
      net: ChildUnaryPermissionArg::Inherit,
      ffi: ChildUnaryPermissionArg::Inherit,
      read: ChildUnaryPermissionArg::Inherit,
      run: ChildUnaryPermissionArg::Inherit,
      write: ChildUnaryPermissionArg::Inherit,
    }
  }

  pub fn none() -> Self {
    ChildPermissionsArg {
      env: ChildUnaryPermissionArg::NotGranted,
      hrtime: ChildUnitPermissionArg::NotGranted,
      net: ChildUnaryPermissionArg::NotGranted,
      ffi: ChildUnaryPermissionArg::NotGranted,
      read: ChildUnaryPermissionArg::NotGranted,
      run: ChildUnaryPermissionArg::NotGranted,
      write: ChildUnaryPermissionArg::NotGranted,
    }
  }
}

impl<'de> Deserialize<'de> for ChildPermissionsArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildPermissionsArgVisitor;
    impl<'de> de::Visitor<'de> for ChildPermissionsArgVisitor {
      type Value = ChildPermissionsArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or \"none\" or object")
      }

      fn visit_unit<E>(self) -> Result<ChildPermissionsArg, E>
      where
        E: de::Error,
      {
        Ok(ChildPermissionsArg::inherit())
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildPermissionsArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildPermissionsArg::inherit())
        } else if v == "none" {
          Ok(ChildPermissionsArg::none())
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_map<V>(self, mut v: V) -> Result<ChildPermissionsArg, V::Error>
      where
        V: de::MapAccess<'de>,
      {
        let mut child_permissions_arg = ChildPermissionsArg::none();
        while let Some((key, value)) =
          v.next_entry::<String, serde_json::Value>()?
        {
          if key == "env" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.env = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.env) {}", e))
            })?;
          } else if key == "hrtime" {
            let arg = serde_json::from_value::<ChildUnitPermissionArg>(value);
            child_permissions_arg.hrtime = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.hrtime) {}", e))
            })?;
          } else if key == "net" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.net = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.net) {}", e))
            })?;
          } else if key == "ffi" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.ffi = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.ffi) {}", e))
            })?;
          } else if key == "read" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.read = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.read) {}", e))
            })?;
          } else if key == "run" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.run = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.run) {}", e))
            })?;
          } else if key == "write" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.write = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.write) {}", e))
            })?;
          } else {
            return Err(de::Error::custom("unknown permission name"));
          }
        }
        Ok(child_permissions_arg)
      }
    }
    deserializer.deserialize_any(ChildPermissionsArgVisitor)
  }
}

pub fn create_child_permissions(
  main_perms: &mut Permissions,
  child_permissions_arg: ChildPermissionsArg,
) -> Result<Permissions, AnyError> {
  let mut worker_perms = Permissions::default();
  match child_permissions_arg.env {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.env = main_perms.env.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.env.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.env.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.env.granted_list =
        Permissions::new_env(&Some(granted_list), false)?.granted_list;
      if !worker_perms
        .env
        .granted_list
        .iter()
        .all(|desc| main_perms.env.check(desc.as_ref()).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.env.denied_list = main_perms.env.denied_list.clone();
  if main_perms.env.global_state == PermissionState::Denied {
    worker_perms.env.global_state = PermissionState::Denied;
  }
  worker_perms.env.prompt = main_perms.env.prompt;
  match child_permissions_arg.hrtime {
    ChildUnitPermissionArg::Inherit => {
      worker_perms.hrtime = main_perms.hrtime.clone();
    }
    ChildUnitPermissionArg::Granted => {
      if main_perms.hrtime.check().is_err() {
        return Err(escalation_error());
      }
      worker_perms.hrtime.state = PermissionState::Granted;
    }
    ChildUnitPermissionArg::NotGranted => {}
  }
  if main_perms.hrtime.state == PermissionState::Denied {
    worker_perms.hrtime.state = PermissionState::Denied;
  }
  worker_perms.hrtime.prompt = main_perms.hrtime.prompt;
  match child_permissions_arg.net {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.net = main_perms.net.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.net.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.net.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.net.granted_list =
        Permissions::new_net(&Some(granted_list), false)?.granted_list;
      if !worker_perms
        .net
        .granted_list
        .iter()
        .all(|desc| main_perms.net.check(&(&desc.0, desc.1)).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.net.denied_list = main_perms.net.denied_list.clone();
  if main_perms.net.global_state == PermissionState::Denied {
    worker_perms.net.global_state = PermissionState::Denied;
  }
  worker_perms.net.prompt = main_perms.net.prompt;
  match child_permissions_arg.ffi {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.ffi = main_perms.ffi.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.ffi.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.ffi.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.ffi.granted_list = Permissions::new_ffi(
        &Some(granted_list.iter().map(PathBuf::from).collect()),
        false,
      )?
      .granted_list;
      if !worker_perms
        .ffi
        .granted_list
        .iter()
        .all(|desc| main_perms.ffi.check(Some(&desc.0)).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.ffi.denied_list = main_perms.ffi.denied_list.clone();
  if main_perms.ffi.global_state == PermissionState::Denied {
    worker_perms.ffi.global_state = PermissionState::Denied;
  }
  worker_perms.ffi.prompt = main_perms.ffi.prompt;
  match child_permissions_arg.read {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.read = main_perms.read.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.read.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.read.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.read.granted_list = Permissions::new_read(
        &Some(granted_list.iter().map(PathBuf::from).collect()),
        false,
      )?
      .granted_list;
      if !worker_perms
        .read
        .granted_list
        .iter()
        .all(|desc| main_perms.read.check(&desc.0).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.read.denied_list = main_perms.read.denied_list.clone();
  if main_perms.read.global_state == PermissionState::Denied {
    worker_perms.read.global_state = PermissionState::Denied;
  }
  worker_perms.read.prompt = main_perms.read.prompt;
  match child_permissions_arg.run {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.run = main_perms.run.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.run.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.run.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.run.granted_list =
        Permissions::new_run(&Some(granted_list), false)?.granted_list;
      if !worker_perms
        .run
        .granted_list
        .iter()
        .all(|desc| main_perms.run.check(&desc.to_string()).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.run.denied_list = main_perms.run.denied_list.clone();
  if main_perms.run.global_state == PermissionState::Denied {
    worker_perms.run.global_state = PermissionState::Denied;
  }
  worker_perms.run.prompt = main_perms.run.prompt;
  match child_permissions_arg.write {
    ChildUnaryPermissionArg::Inherit => {
      worker_perms.write = main_perms.write.clone();
    }
    ChildUnaryPermissionArg::Granted => {
      if main_perms.write.check_all().is_err() {
        return Err(escalation_error());
      }
      worker_perms.write.global_state = PermissionState::Granted;
    }
    ChildUnaryPermissionArg::NotGranted => {}
    ChildUnaryPermissionArg::GrantedList(granted_list) => {
      worker_perms.write.granted_list = Permissions::new_write(
        &Some(granted_list.iter().map(PathBuf::from).collect()),
        false,
      )?
      .granted_list;
      if !worker_perms
        .write
        .granted_list
        .iter()
        .all(|desc| main_perms.write.check(&desc.0).is_ok())
      {
        return Err(escalation_error());
      }
    }
  }
  worker_perms.write.denied_list = main_perms.write.denied_list.clone();
  if main_perms.write.global_state == PermissionState::Denied {
    worker_perms.write.global_state = PermissionState::Denied;
  }
  worker_perms.write.prompt = main_perms.write.prompt;
  Ok(worker_perms)
}

/// Shows the permission prompt and returns the answer according to the user input.
/// This loops until the user gives the proper input.
#[cfg(not(test))]
fn permission_prompt(message: &str, name: &str) -> bool {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    return false;
  };

  #[cfg(unix)]
  fn clear_stdin() -> Result<(), AnyError> {
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let r = unsafe { libc::tcflush(0, libc::TCIFLUSH) };
    assert_eq!(r, 0);
    Ok(())
  }

  #[cfg(not(unix))]
  fn clear_stdin() -> Result<(), AnyError> {
    use deno_core::anyhow::bail;
    use winapi::shared::minwindef::TRUE;
    use winapi::shared::minwindef::UINT;
    use winapi::shared::minwindef::WORD;
    use winapi::shared::ntdef::WCHAR;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_INPUT_HANDLE;
    use winapi::um::wincon::FlushConsoleInputBuffer;
    use winapi::um::wincon::PeekConsoleInputW;
    use winapi::um::wincon::WriteConsoleInputW;
    use winapi::um::wincontypes::INPUT_RECORD;
    use winapi::um::wincontypes::KEY_EVENT;
    use winapi::um::winnt::HANDLE;
    use winapi::um::winuser::MapVirtualKeyW;
    use winapi::um::winuser::MAPVK_VK_TO_VSC;
    use winapi::um::winuser::VK_RETURN;

    // SAFETY: winapi calls
    unsafe {
      let stdin = GetStdHandle(STD_INPUT_HANDLE);
      // emulate an enter key press to clear any line buffered console characters
      emulate_enter_key_press(stdin)?;
      // read the buffered line or enter key press
      read_stdin_line()?;
      // check if our emulated key press was executed
      if is_input_buffer_empty(stdin)? {
        // if so, move the cursor up to prevent a blank line
        move_cursor_up()?;
      } else {
        // the emulated key press is still pending, so a buffered line was read
        // and we can flush the emulated key press
        flush_input_buffer(stdin)?;
      }
    }

    return Ok(());

    unsafe fn flush_input_buffer(stdin: HANDLE) -> Result<(), AnyError> {
      let success = FlushConsoleInputBuffer(stdin);
      if success != TRUE {
        bail!(
          "Could not flush the console input buffer: {}",
          std::io::Error::last_os_error()
        )
      }
      Ok(())
    }

    unsafe fn emulate_enter_key_press(stdin: HANDLE) -> Result<(), AnyError> {
      // https://github.com/libuv/libuv/blob/a39009a5a9252a566ca0704d02df8dabc4ce328f/src/win/tty.c#L1121-L1131
      let mut input_record: INPUT_RECORD = std::mem::zeroed();
      input_record.EventType = KEY_EVENT;
      input_record.Event.KeyEvent_mut().bKeyDown = TRUE;
      input_record.Event.KeyEvent_mut().wRepeatCount = 1;
      input_record.Event.KeyEvent_mut().wVirtualKeyCode = VK_RETURN as WORD;
      input_record.Event.KeyEvent_mut().wVirtualScanCode =
        MapVirtualKeyW(VK_RETURN as UINT, MAPVK_VK_TO_VSC) as WORD;
      *input_record.Event.KeyEvent_mut().uChar.UnicodeChar_mut() =
        '\r' as WCHAR;

      let mut record_written = 0;
      let success =
        WriteConsoleInputW(stdin, &input_record, 1, &mut record_written);
      if success != TRUE {
        bail!(
          "Could not emulate enter key press: {}",
          std::io::Error::last_os_error()
        );
      }
      Ok(())
    }

    unsafe fn is_input_buffer_empty(stdin: HANDLE) -> Result<bool, AnyError> {
      let mut buffer = Vec::with_capacity(1);
      let mut events_read = 0;
      let success =
        PeekConsoleInputW(stdin, buffer.as_mut_ptr(), 1, &mut events_read);
      if success != TRUE {
        bail!(
          "Could not peek the console input buffer: {}",
          std::io::Error::last_os_error()
        )
      }
      Ok(events_read == 0)
    }

    fn move_cursor_up() -> Result<(), AnyError> {
      use std::io::Write;
      write!(std::io::stderr(), "\x1B[1A")?;
      Ok(())
    }

    fn read_stdin_line() -> Result<(), AnyError> {
      let mut input = String::new();
      let stdin = std::io::stdin();
      stdin.read_line(&mut input)?;
      Ok(())
    }
  }

  // For security reasons we must consume everything in stdin so that previously
  // buffered data cannot effect the prompt.
  if let Err(err) = clear_stdin() {
    eprintln!("Error clearing stdin for permission prompt. {:#}", err);
    return false; // don't grant permission if this fails
  }

  let opts = "[y/n (y = yes allow, n = no deny)] ";
  let msg = format!(
    "{}  ️Deno requests {}. Run again with --allow-{} to bypass this prompt.\n   Allow? {} ",
    PERMISSION_EMOJI, message, name, opts
  );
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{}", colors::bold(&msg));
  loop {
    let mut input = String::new();
    let stdin = std::io::stdin();
    let result = stdin.read_line(&mut input);
    if result.is_err() {
      return false;
    };
    let ch = match input.chars().next() {
      None => return false,
      Some(v) => v,
    };
    match ch.to_ascii_lowercase() {
      'y' => return true,
      'n' => return false,
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
fn permission_prompt(_message: &str, _flag: &str) -> bool {
  STUB_PROMPT_VALUE.load(Ordering::SeqCst)
}

#[cfg(test)]
static STUB_PROMPT_VALUE: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
static PERMISSION_PROMPT_STUB_VALUE_SETTER: Lazy<
  Mutex<PermissionPromptStubValueSetter>,
> = Lazy::new(|| Mutex::new(PermissionPromptStubValueSetter));

#[cfg(test)]
struct PermissionPromptStubValueSetter;

#[cfg(test)]
impl PermissionPromptStubValueSetter {
  pub fn set(&self, value: bool) {
    STUB_PROMPT_VALUE.store(value, Ordering::SeqCst);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url_or_path;
  use deno_core::serde_json::json;

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
    })
    .unwrap();

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
    })
    .unwrap();

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
    })
    .unwrap();

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
    })
    .unwrap();

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
      assert!(perms.net.check(&(host, Some(port))).is_err());
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
    })
    .unwrap();

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
    })
    .unwrap();

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
          .unwrap()
      },
      write: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_write(&Some(vec![PathBuf::from("/foo")]), false)
          .unwrap()
      },
      net: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_net(&Some(svec!["127.0.0.1:8000"]), false).unwrap()
      },
      env: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_env(&Some(svec!["HOME"]), false).unwrap()
      },
      run: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_run(&Some(svec!["deno"]), false).unwrap()
      },
      ffi: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_ffi(&Some(vec![PathBuf::from("deno")]), false)
          .unwrap()
      },
      hrtime: UnitPermission {
        state: PermissionState::Prompt,
        ..Permissions::new_hrtime(false)
      },
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms1.read.query(None), PermissionState::Granted);
      assert_eq!(perms1.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(None), PermissionState::Prompt);
      assert_eq!(perms2.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms1.write.query(None), PermissionState::Granted);
      assert_eq!(perms1.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(None), PermissionState::Prompt);
      assert_eq!(perms2.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms1.net.query::<&str>(None), PermissionState::Granted);
      assert_eq!(perms1.net.query(Some(&("127.0.0.1", None))), PermissionState::Granted);
      assert_eq!(perms2.net.query::<&str>(None), PermissionState::Prompt);
      assert_eq!(perms2.net.query(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      assert_eq!(perms1.env.query(None), PermissionState::Granted);
      assert_eq!(perms1.env.query(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms2.env.query(None), PermissionState::Prompt);
      assert_eq!(perms2.env.query(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms1.run.query(None), PermissionState::Granted);
      assert_eq!(perms1.run.query(Some("deno")), PermissionState::Granted);
      assert_eq!(perms2.run.query(None), PermissionState::Prompt);
      assert_eq!(perms2.run.query(Some("deno")), PermissionState::Granted);
      assert_eq!(perms1.ffi.query(None), PermissionState::Granted);
      assert_eq!(perms1.ffi.query(Some(Path::new("deno"))), PermissionState::Granted);
      assert_eq!(perms2.ffi.query(None), PermissionState::Prompt);
      assert_eq!(perms2.ffi.query(Some(Path::new("deno"))), PermissionState::Granted);
      assert_eq!(perms1.hrtime.query(), PermissionState::Granted);
      assert_eq!(perms2.hrtime.query(), PermissionState::Prompt);
    };
  }

  #[test]
  fn test_request() {
    let mut perms: Permissions = Default::default();
    #[rustfmt::skip]
    {
      let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
      prompt_value.set(true);
      assert_eq!(perms.read.request(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms.read.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.read.request(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      prompt_value.set(false);
      assert_eq!(perms.write.request(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms.write.query(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      prompt_value.set(true);
      assert_eq!(perms.write.request(None), PermissionState::Denied);
      prompt_value.set(true);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", None))), PermissionState::Granted);
      prompt_value.set(false);
      assert_eq!(perms.net.request(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms.env.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
      assert_eq!(perms.run.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.ffi.request(Some(Path::new("deno"))), PermissionState::Granted);
      assert_eq!(perms.ffi.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.ffi.request(Some(Path::new("deno"))), PermissionState::Granted);
      prompt_value.set(false);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
      prompt_value.set(true);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_revoke() {
    let mut perms = Permissions {
      read: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_read(
          &Some(vec![PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
          false,
        )
        .unwrap()
      },
      write: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_write(
          &Some(vec![PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
          false,
        )
        .unwrap()
      },
      net: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_net(
          &Some(svec!["127.0.0.1", "127.0.0.1:8000"]),
          false,
        )
        .unwrap()
      },
      env: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_env(&Some(svec!["HOME"]), false).unwrap()
      },
      run: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_run(&Some(svec!["deno"]), false).unwrap()
      },
      ffi: UnaryPermission {
        global_state: PermissionState::Prompt,
        ..Permissions::new_ffi(&Some(vec![PathBuf::from("deno")]), false)
          .unwrap()
      },
      hrtime: UnitPermission {
        state: PermissionState::Denied,
        ..Permissions::new_hrtime(false)
      },
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms.read.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.read.query(Some(Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.read.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
      assert_eq!(perms.write.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.write.query(Some(Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.write.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
      assert_eq!(perms.net.revoke(Some(&("127.0.0.1", Some(9000)))), PermissionState::Prompt);
      assert_eq!(perms.net.query(Some(&("127.0.0.1", None))), PermissionState::Prompt);
      assert_eq!(perms.net.query(Some(&("127.0.0.1", Some(8000)))), PermissionState::Granted);
      assert_eq!(perms.env.revoke(Some("HOME")), PermissionState::Prompt);
      assert_eq!(perms.run.revoke(Some("deno")), PermissionState::Prompt);
      assert_eq!(perms.ffi.revoke(Some(Path::new("deno"))), PermissionState::Prompt);
      assert_eq!(perms.hrtime.revoke(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_check() {
    let mut perms = Permissions {
      read: Permissions::new_read(&None, true).unwrap(),
      write: Permissions::new_write(&None, true).unwrap(),
      net: Permissions::new_net(&None, true).unwrap(),
      env: Permissions::new_env(&None, true).unwrap(),
      run: Permissions::new_run(&None, true).unwrap(),
      ffi: Permissions::new_ffi(&None, true).unwrap(),
      hrtime: Permissions::new_hrtime(false),
    };

    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

    prompt_value.set(true);
    assert!(perms.read.check(Path::new("/foo")).is_ok());
    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/foo")).is_ok());
    assert!(perms.read.check(Path::new("/bar")).is_err());

    prompt_value.set(true);
    assert!(perms.write.check(Path::new("/foo")).is_ok());
    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/foo")).is_ok());
    assert!(perms.write.check(Path::new("/bar")).is_err());

    prompt_value.set(true);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_ok());
    prompt_value.set(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_ok());
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_err());
    assert!(perms.net.check(&("127.0.0.1", None)).is_err());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_err());
    assert!(perms.net.check(&("deno.land", None)).is_err());

    prompt_value.set(true);
    assert!(perms.run.check("cat").is_ok());
    prompt_value.set(false);
    assert!(perms.run.check("cat").is_ok());
    assert!(perms.run.check("ls").is_err());

    prompt_value.set(true);
    assert!(perms.env.check("HOME").is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("HOME").is_ok());
    assert!(perms.env.check("PATH").is_err());

    assert!(perms.hrtime.check().is_err());
  }

  #[test]
  fn test_check_fail() {
    let mut perms = Permissions {
      read: Permissions::new_read(&None, true).unwrap(),
      write: Permissions::new_write(&None, true).unwrap(),
      net: Permissions::new_net(&None, true).unwrap(),
      env: Permissions::new_env(&None, true).unwrap(),
      run: Permissions::new_run(&None, true).unwrap(),
      ffi: Permissions::new_ffi(&None, true).unwrap(),
      hrtime: Permissions::new_hrtime(false),
    };

    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/foo")).is_err());
    prompt_value.set(true);
    assert!(perms.read.check(Path::new("/foo")).is_err());
    assert!(perms.read.check(Path::new("/bar")).is_ok());
    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/bar")).is_ok());

    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/foo")).is_err());
    prompt_value.set(true);
    assert!(perms.write.check(Path::new("/foo")).is_err());
    assert!(perms.write.check(Path::new("/bar")).is_ok());
    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/bar")).is_ok());

    prompt_value.set(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_err());
    prompt_value.set(true);
    assert!(perms.net.check(&("127.0.0.1", Some(8000))).is_err());
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_ok());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_ok());
    prompt_value.set(false);
    assert!(perms.net.check(&("127.0.0.1", Some(8001))).is_ok());
    assert!(perms.net.check(&("deno.land", Some(8000))).is_ok());

    prompt_value.set(false);
    assert!(perms.run.check("cat").is_err());
    prompt_value.set(true);
    assert!(perms.run.check("cat").is_err());
    assert!(perms.run.check("ls").is_ok());
    prompt_value.set(false);
    assert!(perms.run.check("ls").is_ok());

    prompt_value.set(false);
    assert!(perms.env.check("HOME").is_err());
    prompt_value.set(true);
    assert!(perms.env.check("HOME").is_err());
    assert!(perms.env.check("PATH").is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("PATH").is_ok());

    prompt_value.set(false);
    assert!(perms.hrtime.check().is_err());
    prompt_value.set(true);
    assert!(perms.hrtime.check().is_err());
  }

  #[test]
  #[cfg(windows)]
  fn test_env_windows() {
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
    let mut perms = Permissions::allow_all();
    perms.env = UnaryPermission {
      global_state: PermissionState::Prompt,
      ..Permissions::new_env(&Some(svec!["HOME"]), false).unwrap()
    };

    prompt_value.set(true);
    assert!(perms.env.check("HOME").is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("HOME").is_ok());
    assert!(perms.env.check("hOmE").is_ok());

    assert_eq!(perms.env.revoke(Some("HomE")), PermissionState::Prompt);
  }

  #[test]
  fn test_deserialize_child_permissions_arg() {
    assert_eq!(
      ChildPermissionsArg::inherit(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::Inherit,
        hrtime: ChildUnitPermissionArg::Inherit,
        net: ChildUnaryPermissionArg::Inherit,
        ffi: ChildUnaryPermissionArg::Inherit,
        read: ChildUnaryPermissionArg::Inherit,
        run: ChildUnaryPermissionArg::Inherit,
        write: ChildUnaryPermissionArg::Inherit,
      }
    );
    assert_eq!(
      ChildPermissionsArg::none(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::NotGranted,
        hrtime: ChildUnitPermissionArg::NotGranted,
        net: ChildUnaryPermissionArg::NotGranted,
        ffi: ChildUnaryPermissionArg::NotGranted,
        read: ChildUnaryPermissionArg::NotGranted,
        run: ChildUnaryPermissionArg::NotGranted,
        write: ChildUnaryPermissionArg::NotGranted,
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!("inherit")).unwrap(),
      ChildPermissionsArg::inherit()
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!("none")).unwrap(),
      ChildPermissionsArg::none()
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({})).unwrap(),
      ChildPermissionsArg::none()
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "env": ["foo", "bar"],
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
        ..ChildPermissionsArg::none()
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "hrtime": true,
      }))
      .unwrap(),
      ChildPermissionsArg {
        hrtime: ChildUnitPermissionArg::Granted,
        ..ChildPermissionsArg::none()
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "hrtime": false,
      }))
      .unwrap(),
      ChildPermissionsArg {
        hrtime: ChildUnitPermissionArg::NotGranted,
        ..ChildPermissionsArg::none()
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "env": true,
        "net": true,
        "ffi": true,
        "read": true,
        "run": true,
        "write": true,
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::Granted,
        net: ChildUnaryPermissionArg::Granted,
        ffi: ChildUnaryPermissionArg::Granted,
        read: ChildUnaryPermissionArg::Granted,
        run: ChildUnaryPermissionArg::Granted,
        write: ChildUnaryPermissionArg::Granted,
        ..ChildPermissionsArg::none()
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "env": false,
        "net": false,
        "ffi": false,
        "read": false,
        "run": false,
        "write": false,
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::NotGranted,
        net: ChildUnaryPermissionArg::NotGranted,
        ffi: ChildUnaryPermissionArg::NotGranted,
        read: ChildUnaryPermissionArg::NotGranted,
        run: ChildUnaryPermissionArg::NotGranted,
        write: ChildUnaryPermissionArg::NotGranted,
        ..ChildPermissionsArg::none()
      }
    );
    assert_eq!(
      serde_json::from_value::<ChildPermissionsArg>(json!({
        "env": ["foo", "bar"],
        "net": ["foo", "bar:8000"],
        "ffi": ["foo", "file:///bar/baz"],
        "read": ["foo", "file:///bar/baz"],
        "run": ["foo", "file:///bar/baz", "./qux"],
        "write": ["foo", "file:///bar/baz"],
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
        net: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar:8000"]),
        ffi: ChildUnaryPermissionArg::GrantedList(svec![
          "foo",
          "file:///bar/baz"
        ]),
        read: ChildUnaryPermissionArg::GrantedList(svec![
          "foo",
          "file:///bar/baz"
        ]),
        run: ChildUnaryPermissionArg::GrantedList(svec![
          "foo",
          "file:///bar/baz",
          "./qux"
        ]),
        write: ChildUnaryPermissionArg::GrantedList(svec![
          "foo",
          "file:///bar/baz"
        ]),
        ..ChildPermissionsArg::none()
      }
    );
  }

  #[test]
  fn test_create_child_permissions() {
    let mut main_perms = Permissions {
      env: Permissions::new_env(&Some(vec![]), false).unwrap(),
      hrtime: Permissions::new_hrtime(true),
      net: Permissions::new_net(&Some(svec!["foo", "bar"]), false).unwrap(),
      ..Default::default()
    };
    assert_eq!(
      create_child_permissions(
        &mut main_perms.clone(),
        ChildPermissionsArg {
          env: ChildUnaryPermissionArg::Inherit,
          hrtime: ChildUnitPermissionArg::NotGranted,
          net: ChildUnaryPermissionArg::GrantedList(svec!["foo"]),
          ffi: ChildUnaryPermissionArg::NotGranted,
          ..ChildPermissionsArg::none()
        }
      )
      .unwrap(),
      Permissions {
        env: Permissions::new_env(&Some(vec![]), false).unwrap(),
        net: Permissions::new_net(&Some(svec!["foo"]), false).unwrap(),
        ..Default::default()
      }
    );
    assert!(create_child_permissions(
      &mut main_perms.clone(),
      ChildPermissionsArg {
        net: ChildUnaryPermissionArg::Granted,
        ..ChildPermissionsArg::none()
      }
    )
    .is_err());
    assert!(create_child_permissions(
      &mut main_perms.clone(),
      ChildPermissionsArg {
        net: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar", "baz"]),
        ..ChildPermissionsArg::none()
      }
    )
    .is_err());
    assert!(create_child_permissions(
      &mut main_perms,
      ChildPermissionsArg {
        ffi: ChildUnaryPermissionArg::GrantedList(svec!["foo"]),
        ..ChildPermissionsArg::none()
      }
    )
    .is_err());
  }

  #[test]
  fn test_create_child_permissions_with_prompt() {
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
    let mut main_perms = Permissions::from_options(&PermissionsOptions {
      prompt: true,
      ..Default::default()
    })
    .unwrap();
    prompt_value.set(true);
    let worker_perms = create_child_permissions(
      &mut main_perms,
      ChildPermissionsArg {
        read: ChildUnaryPermissionArg::Granted,
        run: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
        ..ChildPermissionsArg::none()
      },
    )
    .unwrap();
    assert_eq!(main_perms, worker_perms);
  }

  #[test]
  fn test_create_child_permissions_with_inherited_denied_list() {
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
    let mut main_perms = Permissions::from_options(&PermissionsOptions {
      prompt: true,
      ..Default::default()
    })
    .unwrap();
    prompt_value.set(false);
    assert!(main_perms.write.check(&PathBuf::from("foo")).is_err());
    let worker_perms = create_child_permissions(
      &mut main_perms.clone(),
      ChildPermissionsArg::none(),
    )
    .unwrap();
    assert_eq!(worker_perms.write.denied_list, main_perms.write.denied_list);
  }

  #[test]
  fn test_handle_empty_value() {
    assert!(Permissions::new_read(&Some(vec![PathBuf::new()]), false).is_err());
    assert!(Permissions::new_env(&Some(vec![String::new()]), false).is_err());
    assert!(Permissions::new_run(&Some(vec![String::new()]), false).is_err());
    assert!(Permissions::new_ffi(&Some(vec![PathBuf::new()]), false).is_err());
    assert!(Permissions::new_net(&Some(svec![String::new()]), false).is_err());
    assert!(Permissions::new_write(&Some(vec![PathBuf::new()]), false).is_err());
  }
}
