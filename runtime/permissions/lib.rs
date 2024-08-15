// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::url;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_terminal::colors;
use fqdn::FQDN;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;
use std::sync::Once;
use which::which;

pub mod prompter;
use prompter::permission_prompt;
use prompter::PromptResponse;
use prompter::PERMISSION_EMOJI;

pub use prompter::set_prompt_callbacks;
pub use prompter::PromptCallback;

/// Fast exit from permission check routines if this permission
/// is in the "fully-granted" state.
macro_rules! skip_check_if_is_permission_fully_granted {
  ($this:ident) => {
    if $this.is_allow_all() {
      return Ok(());
    }
  };
}

#[inline]
fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  if path.is_absolute() {
    Ok(normalize_path(path))
  } else {
    #[allow(clippy::disallowed_methods)]
    let cwd = std::env::current_dir()
      .context("Failed to get current working directory")?;
    Ok(normalize_path(cwd.join(path)))
  }
}

static DEBUG_LOG_ENABLED: Lazy<bool> =
  Lazy::new(|| log::log_enabled!(log::Level::Debug));

/// Quadri-state value for storing permission state
#[derive(
  Eq, PartialEq, Default, Debug, Clone, Copy, Deserialize, PartialOrd,
)]
pub enum PermissionState {
  Granted = 0,
  GrantedPartial = 1,
  #[default]
  Prompt = 2,
  Denied = 3,
}

/// `AllowPartial` prescribes how to treat a permission which is partially
/// denied due to a `--deny-*` flag affecting a subscope of the queried
/// permission.
///
/// `TreatAsGranted` is used in place of `TreatAsPartialGranted` when we don't
/// want to wastefully check for partial denials when, say, checking read
/// access for a file.
#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
enum AllowPartial {
  TreatAsGranted,
  TreatAsDenied,
  TreatAsPartialGranted,
}

impl From<bool> for AllowPartial {
  fn from(value: bool) -> Self {
    if value {
      Self::TreatAsGranted
    } else {
      Self::TreatAsDenied
    }
  }
}

impl PermissionState {
  #[inline(always)]
  fn log_perm_access(name: &str, info: impl FnOnce() -> Option<String>) {
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

  fn fmt_access(name: &str, info: impl FnOnce() -> Option<String>) -> String {
    format!(
      "{} access{}",
      name,
      info()
        .map(|info| { format!(" to {info}") })
        .unwrap_or_default(),
    )
  }

  fn error(name: &str, info: impl FnOnce() -> Option<String>) -> AnyError {
    let msg = if !IsStandaloneBinary::get_instance(false).is_standalone_binary()
    {
      format!(
        "Requires {}, run again with the --allow-{} flag",
        Self::fmt_access(name, info),
        name
      )
    } else {
      format!(
        "Requires {}, specify the required permissions during compilation using `deno compile --allow-{}`",
        Self::fmt_access(name, info),
        name
      )
    };
    custom_error("PermissionDenied", msg)
  }

  /// Check the permission state. bool is whether a prompt was issued.
  #[inline]
  fn check(
    self,
    name: &str,
    api_name: Option<&str>,
    info: Option<&str>,
    prompt: bool,
  ) -> (Result<(), AnyError>, bool, bool) {
    self.check2(name, api_name, || info.map(|s| s.to_string()), prompt)
  }

  #[inline]
  fn check2(
    self,
    name: &str,
    api_name: Option<&str>,
    info: impl Fn() -> Option<String>,
    prompt: bool,
  ) -> (Result<(), AnyError>, bool, bool) {
    match self {
      PermissionState::Granted => {
        Self::log_perm_access(name, info);
        (Ok(()), false, false)
      }
      PermissionState::Prompt if prompt => {
        let msg = format!(
          "{} access{}",
          name,
          info()
            .map(|info| { format!(" to {info}") })
            .unwrap_or_default(),
        );
        match permission_prompt(&msg, name, api_name, true) {
          PromptResponse::Allow => {
            Self::log_perm_access(name, info);
            (Ok(()), true, false)
          }
          PromptResponse::AllowAll => {
            Self::log_perm_access(name, info);
            (Ok(()), true, true)
          }
          PromptResponse::Deny => (Err(Self::error(name, info)), true, false),
        }
      }
      _ => (Err(Self::error(name, info)), false, false),
    }
  }
}

impl fmt::Display for PermissionState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PermissionState::Granted => f.pad("granted"),
      PermissionState::GrantedPartial => f.pad("granted-partial"),
      PermissionState::Prompt => f.pad("prompt"),
      PermissionState::Denied => f.pad("denied"),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
      if PromptResponse::Allow
        == permission_prompt(
          &format!("access to {}", self.description),
          self.name,
          Some("Deno.permissions.query()"),
          false,
        )
      {
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
    let (result, prompted, _is_allow_all) =
      self.state.check(self.name, None, None, self.prompt);
    if prompted {
      if result.is_ok() {
        self.state = PermissionState::Granted;
      } else {
        self.state = PermissionState::Denied;
      }
    }
    result
  }

  fn create_child_permissions(
    &mut self,
    flag: ChildUnitPermissionArg,
  ) -> Result<Self, AnyError> {
    let mut perm = self.clone();
    match flag {
      ChildUnitPermissionArg::Inherit => {
        // copy
      }
      ChildUnitPermissionArg::Granted => {
        if self.check().is_err() {
          return Err(escalation_error());
        }
        perm.state = PermissionState::Granted;
      }
      ChildUnitPermissionArg::NotGranted => {
        perm.state = PermissionState::Prompt;
      }
    }
    if self.state == PermissionState::Denied {
      perm.state = PermissionState::Denied;
    }
    Ok(perm)
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

pub trait Descriptor: Eq + Clone + Hash {
  type Arg: From<String>;

  /// Parse this descriptor from a list of Self::Arg, which may have been converted from
  /// command-line strings.
  fn parse(list: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError>;

  /// Generic check function to check this descriptor against a `UnaryPermission`.
  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError>;

  fn flag_name() -> &'static str;
  fn name(&self) -> Cow<str>;
  // By default, specifies no-stronger-than relationship.
  // As this is not strict, it's only true when descriptors are the same.
  fn stronger_than(&self, other: &Self) -> bool {
    self == other
  }
  fn aliases(&self) -> Vec<Self> {
    vec![]
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnaryPermission<T: Descriptor + Hash> {
  granted_global: bool,
  granted_list: HashSet<T>,
  flag_denied_global: bool,
  flag_denied_list: HashSet<T>,
  prompt_denied_global: bool,
  prompt_denied_list: HashSet<T>,
  prompt: bool,
}

impl<T: Descriptor + Hash> Default for UnaryPermission<T> {
  fn default() -> Self {
    UnaryPermission {
      granted_global: Default::default(),
      granted_list: Default::default(),
      flag_denied_global: Default::default(),
      flag_denied_list: Default::default(),
      prompt_denied_global: Default::default(),
      prompt_denied_list: Default::default(),
      prompt: Default::default(),
    }
  }
}

impl<T: Descriptor + Hash> UnaryPermission<T> {
  pub fn allow_all() -> Self {
    Self {
      granted_global: true,
      ..Default::default()
    }
  }

  pub fn is_allow_all(&self) -> bool {
    self.granted_global
      && self.flag_denied_list.is_empty()
      && self.prompt_denied_list.is_empty()
  }

  pub fn check_all_api(
    &mut self,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name, || None)
  }

  fn check_desc(
    &mut self,
    desc: Option<&T>,
    assert_non_partial: bool,
    api_name: Option<&str>,
    get_display_name: impl Fn() -> Option<String>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let (result, prompted, is_allow_all) = self
      .query_desc(desc, AllowPartial::from(!assert_non_partial))
      .check2(
        T::flag_name(),
        api_name,
        || match get_display_name() {
          Some(display_name) => Some(display_name),
          None => desc.map(|d| format!("\"{}\"", d.name())),
        },
        self.prompt,
      );
    if prompted {
      if result.is_ok() {
        if is_allow_all {
          self.insert_granted(None);
        } else {
          self.insert_granted(desc.cloned());
        }
      } else {
        self.insert_prompt_denied(desc.cloned());
      }
    }
    result
  }

  fn query_desc(
    &self,
    desc: Option<&T>,
    allow_partial: AllowPartial,
  ) -> PermissionState {
    let aliases = desc.map_or(vec![], T::aliases);
    for desc in [desc]
      .into_iter()
      .chain(aliases.iter().map(Some).collect::<Vec<_>>())
    {
      let state = if self.is_flag_denied(desc) || self.is_prompt_denied(desc) {
        PermissionState::Denied
      } else if self.is_granted(desc) {
        match allow_partial {
          AllowPartial::TreatAsGranted => PermissionState::Granted,
          AllowPartial::TreatAsDenied => {
            if self.is_partial_flag_denied(desc) {
              PermissionState::Denied
            } else {
              PermissionState::Granted
            }
          }
          AllowPartial::TreatAsPartialGranted => {
            if self.is_partial_flag_denied(desc) {
              PermissionState::GrantedPartial
            } else {
              PermissionState::Granted
            }
          }
        }
      } else if matches!(allow_partial, AllowPartial::TreatAsDenied)
        && self.is_partial_flag_denied(desc)
      {
        PermissionState::Denied
      } else {
        PermissionState::Prompt
      };
      if state != PermissionState::Prompt {
        return state;
      }
    }
    PermissionState::Prompt
  }

  fn request_desc(
    &mut self,
    desc: Option<&T>,
    get_display_name: impl Fn() -> Option<String>,
  ) -> PermissionState {
    let state = self.query_desc(desc, AllowPartial::TreatAsPartialGranted);
    if state == PermissionState::Granted {
      self.insert_granted(desc.cloned());
      return state;
    }
    if state != PermissionState::Prompt {
      return state;
    }
    let mut message = String::with_capacity(40);
    message.push_str(&format!("{} access", T::flag_name()));
    match get_display_name() {
      Some(display_name) => {
        message.push_str(&format!(" to \"{}\"", display_name))
      }
      None => {
        if let Some(desc) = desc {
          message.push_str(&format!(" to \"{}\"", desc.name()));
        }
      }
    }
    match permission_prompt(
      &message,
      T::flag_name(),
      Some("Deno.permissions.request()"),
      true,
    ) {
      PromptResponse::Allow => {
        self.insert_granted(desc.cloned());
        PermissionState::Granted
      }
      PromptResponse::Deny => {
        self.insert_prompt_denied(desc.cloned());
        PermissionState::Denied
      }
      PromptResponse::AllowAll => {
        self.insert_granted(None);
        PermissionState::Granted
      }
    }
  }

  fn revoke_desc(&mut self, desc: Option<&T>) -> PermissionState {
    match desc {
      Some(desc) => {
        self.granted_list.retain(|v| !v.stronger_than(desc));
        for alias in desc.aliases() {
          self.granted_list.retain(|v| !v.stronger_than(&alias));
        }
      }
      None => {
        self.granted_global = false;
        // Revoke global is a special case where the entire granted list is
        // cleared. It's inconsistent with the granular case where only
        // descriptors stronger than the revoked one are purged.
        self.granted_list.clear();
      }
    }
    self.query_desc(desc, AllowPartial::TreatAsPartialGranted)
  }

  fn is_granted(&self, desc: Option<&T>) -> bool {
    Self::list_contains(desc, self.granted_global, &self.granted_list)
  }

  fn is_flag_denied(&self, desc: Option<&T>) -> bool {
    Self::list_contains(desc, self.flag_denied_global, &self.flag_denied_list)
  }

  fn is_prompt_denied(&self, desc: Option<&T>) -> bool {
    match desc {
      Some(desc) => self
        .prompt_denied_list
        .iter()
        .any(|v| desc.stronger_than(v)),
      None => self.prompt_denied_global || !self.prompt_denied_list.is_empty(),
    }
  }

  fn is_partial_flag_denied(&self, desc: Option<&T>) -> bool {
    match desc {
      None => !self.flag_denied_list.is_empty(),
      Some(desc) => self.flag_denied_list.iter().any(|v| desc.stronger_than(v)),
    }
  }

  fn list_contains(
    desc: Option<&T>,
    list_global: bool,
    list: &HashSet<T>,
  ) -> bool {
    match desc {
      Some(desc) => list_global || list.iter().any(|v| v.stronger_than(desc)),
      None => list_global,
    }
  }

  fn insert_granted(&mut self, desc: Option<T>) {
    Self::list_insert(desc, &mut self.granted_global, &mut self.granted_list);
  }

  fn insert_prompt_denied(&mut self, desc: Option<T>) {
    Self::list_insert(
      desc,
      &mut self.prompt_denied_global,
      &mut self.prompt_denied_list,
    );
  }

  fn list_insert(
    desc: Option<T>,
    list_global: &mut bool,
    list: &mut HashSet<T>,
  ) {
    match desc {
      Some(desc) => {
        let aliases = desc.aliases();
        list.insert(desc);
        for alias in aliases {
          list.insert(alias);
        }
      }
      None => *list_global = true,
    }
  }

  fn create_child_permissions(
    &mut self,
    flag: ChildUnaryPermissionArg,
  ) -> Result<UnaryPermission<T>, AnyError> {
    let mut perms = Self::default();

    match flag {
      ChildUnaryPermissionArg::Inherit => {
        perms.clone_from(self);
      }
      ChildUnaryPermissionArg::Granted => {
        if self.check_all_api(None).is_err() {
          return Err(escalation_error());
        }
        perms.granted_global = true;
      }
      ChildUnaryPermissionArg::NotGranted => {}
      ChildUnaryPermissionArg::GrantedList(granted_list) => {
        let granted: Vec<T::Arg> =
          granted_list.into_iter().map(From::from).collect();
        perms.granted_list = T::parse(&Some(granted))?;
        if !perms
          .granted_list
          .iter()
          .all(|desc| desc.check_in_permission(self, None).is_ok())
        {
          return Err(escalation_error());
        }
      }
    }
    perms.flag_denied_global = self.flag_denied_global;
    perms.prompt_denied_global = self.prompt_denied_global;
    perms.prompt = self.prompt;
    perms.flag_denied_list.clone_from(&self.flag_denied_list);
    perms
      .prompt_denied_list
      .clone_from(&self.prompt_denied_list);

    Ok(perms)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ReadDescriptor(pub PathBuf);

impl Descriptor for ReadDescriptor {
  type Arg = PathBuf;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name, || None)
  }

  fn parse(args: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_path_list(args, ReadDescriptor)
  }

  fn flag_name() -> &'static str {
    "read"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.0.display().to_string())
  }

  fn stronger_than(&self, other: &Self) -> bool {
    other.0.starts_with(&self.0)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WriteDescriptor(pub PathBuf);

impl Descriptor for WriteDescriptor {
  type Arg = PathBuf;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name, || None)
  }

  fn parse(args: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_path_list(args, WriteDescriptor)
  }

  fn flag_name() -> &'static str {
    "write"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.0.display().to_string())
  }

  fn stronger_than(&self, other: &Self) -> bool {
    other.0.starts_with(&self.0)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Host {
  Fqdn(FQDN),
  Ip(IpAddr),
}

impl FromStr for Host {
  type Err = AnyError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.starts_with('[') && s.ends_with(']') {
      let ip = s[1..s.len() - 1]
        .parse::<Ipv6Addr>()
        .map_err(|_| uri_error(format!("invalid IPv6 address: '{s}'")))?;
      return Ok(Host::Ip(IpAddr::V6(ip)));
    }
    let (without_trailing_dot, has_trailing_dot) =
      s.strip_suffix('.').map_or((s, false), |s| (s, true));
    if let Ok(ip) = without_trailing_dot.parse::<IpAddr>() {
      if has_trailing_dot {
        return Err(uri_error(format!(
          "invalid host: '{without_trailing_dot}'"
        )));
      }
      Ok(Host::Ip(ip))
    } else {
      let lower = if s.chars().all(|c| c.is_ascii_lowercase()) {
        Cow::Borrowed(s)
      } else {
        Cow::Owned(s.to_ascii_lowercase())
      };
      let fqdn = FQDN::from_str(&lower)
        .with_context(|| format!("invalid host: '{s}'"))?;
      if fqdn.is_root() {
        return Err(uri_error(format!("invalid empty host: '{s}'")));
      }
      Ok(Host::Fqdn(fqdn))
    }
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NetDescriptor(pub Host, pub Option<u16>);

impl Descriptor for NetDescriptor {
  type Arg = String;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name, || None)
  }

  fn parse(args: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_net_list(args)
  }

  fn flag_name() -> &'static str {
    "net"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(format!("{}", self))
  }

  fn stronger_than(&self, other: &Self) -> bool {
    self.0 == other.0 && (self.1.is_none() || self.1 == other.1)
  }
}

impl FromStr for NetDescriptor {
  type Err = AnyError;

  fn from_str(hostname: &str) -> Result<Self, Self::Err> {
    // If this is a IPv6 address enclosed in square brackets, parse it as such.
    if hostname.starts_with('[') {
      if let Some((ip, after)) = hostname.split_once(']') {
        let ip = ip[1..].parse::<Ipv6Addr>().map_err(|_| {
          uri_error(format!("invalid IPv6 address in '{hostname}': '{ip}'"))
        })?;
        let port = if let Some(port) = after.strip_prefix(':') {
          let port = port.parse::<u16>().map_err(|_| {
            uri_error(format!("invalid port in '{hostname}': '{port}'"))
          })?;
          Some(port)
        } else if after.is_empty() {
          None
        } else {
          return Err(uri_error(format!("invalid host: '{hostname}'")));
        };
        return Ok(NetDescriptor(Host::Ip(IpAddr::V6(ip)), port));
      } else {
        return Err(uri_error(format!("invalid host: '{hostname}'")));
      }
    }

    // Otherwise it is an IPv4 address or a FQDN with an optional port.
    let (host, port) = match hostname.split_once(':') {
      Some((_, "")) => {
        return Err(uri_error(format!("invalid empty port in '{hostname}'")));
      }
      Some((host, port)) => (host, port),
      None => (hostname, ""),
    };
    let host = host.parse::<Host>()?;

    let port = if port.is_empty() {
      None
    } else {
      let port = port.parse::<u16>().map_err(|_| {
        // If the user forgot to enclose an IPv6 address in square brackets, we
        // should give them a hint. There are always at least two colons in an
        // IPv6 address, so this heuristic finds likely a bare IPv6 address.
        if port.contains(':') {
          uri_error(format!(
            "ipv6 addresses must be enclosed in square brackets: '{hostname}'"
          ))
        } else {
          uri_error(format!("invalid port in '{hostname}': '{port}'"))
        }
      })?;
      Some(port)
    };

    Ok(NetDescriptor(host, port))
  }
}

impl fmt::Display for NetDescriptor {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.0 {
      Host::Fqdn(fqdn) => write!(f, "{fqdn}"),
      Host::Ip(IpAddr::V4(ip)) => write!(f, "{ip}"),
      Host::Ip(IpAddr::V6(ip)) => write!(f, "[{ip}]"),
    }?;
    if let Some(port) = self.1 {
      write!(f, ":{}", port)?;
    }
    Ok(())
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct EnvDescriptor(EnvVarName);

impl EnvDescriptor {
  pub fn new(env: impl AsRef<str>) -> Self {
    Self(EnvVarName::new(env))
  }
}

impl Descriptor for EnvDescriptor {
  type Arg = String;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name, || None)
  }

  fn parse(list: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_env_list(list)
  }

  fn flag_name() -> &'static str {
    "env"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.0.as_ref())
  }
}

impl AsRef<str> for EnvDescriptor {
  fn as_ref(&self) -> &str {
    self.0.as_ref()
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum RunDescriptor {
  /// Warning: You may want to construct with `RunDescriptor::from()` for case
  /// handling.
  Name(String),
  /// Warning: You may want to construct with `RunDescriptor::from()` for case
  /// handling.
  Path(PathBuf),
}

impl Descriptor for RunDescriptor {
  type Arg = String;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name, || None)
  }

  fn parse(args: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_run_list(args)
  }

  fn flag_name() -> &'static str {
    "run"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.to_string())
  }

  fn aliases(&self) -> Vec<Self> {
    match self {
      RunDescriptor::Name(name) => match which(name) {
        Ok(path) => vec![RunDescriptor::Path(path)],
        Err(_) => vec![],
      },
      RunDescriptor::Path(_) => vec![],
    }
  }
}

impl From<String> for RunDescriptor {
  fn from(s: String) -> Self {
    #[cfg(windows)]
    let s = s.to_lowercase();
    let is_path = s.contains('/');
    #[cfg(windows)]
    let is_path = is_path || s.contains('\\') || Path::new(&s).is_absolute();
    if is_path {
      Self::Path(resolve_from_cwd(Path::new(&s)).unwrap())
    } else {
      Self::Name(s)
    }
  }
}

impl From<PathBuf> for RunDescriptor {
  fn from(p: PathBuf) -> Self {
    #[cfg(windows)]
    let p = PathBuf::from(p.to_string_lossy().to_string().to_lowercase());
    if p.is_absolute() {
      Self::Path(p)
    } else {
      Self::Path(resolve_from_cwd(&p).unwrap())
    }
  }
}

impl std::fmt::Display for RunDescriptor {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RunDescriptor::Name(s) => f.write_str(s),
      RunDescriptor::Path(p) => f.write_str(&p.display().to_string()),
    }
  }
}

impl AsRef<Path> for RunDescriptor {
  fn as_ref(&self) -> &Path {
    match self {
      RunDescriptor::Name(s) => s.as_ref(),
      RunDescriptor::Path(s) => s.as_ref(),
    }
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SysDescriptor(pub String);

impl Descriptor for SysDescriptor {
  type Arg = String;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name, || None)
  }

  fn parse(list: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_sys_list(list)
  }

  fn flag_name() -> &'static str {
    "sys"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.0.to_string())
  }
}

pub fn parse_sys_kind(kind: &str) -> Result<&str, AnyError> {
  match kind {
    "hostname" | "osRelease" | "osUptime" | "loadavg" | "networkInterfaces"
    | "systemMemoryInfo" | "uid" | "gid" | "cpus" | "homedir" | "getegid"
    | "username" | "statfs" | "getPriority" | "setPriority" => Ok(kind),
    _ => Err(type_error(format!("unknown system info kind \"{kind}\""))),
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FfiDescriptor(pub PathBuf);

impl Descriptor for FfiDescriptor {
  type Arg = PathBuf;

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name, || None)
  }

  fn parse(list: &Option<Vec<Self::Arg>>) -> Result<HashSet<Self>, AnyError> {
    parse_path_list(list, FfiDescriptor)
  }

  fn flag_name() -> &'static str {
    "ffi"
  }

  fn name(&self) -> Cow<str> {
    Cow::from(self.0.display().to_string())
  }

  fn stronger_than(&self, other: &Self) -> bool {
    other.0.starts_with(&self.0)
  }
}

impl UnaryPermission<ReadDescriptor> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    self.query_desc(
      path
        .map(|p| ReadDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, path: Option<&Path>) -> PermissionState {
    self.request_desc(
      path
        .map(|p| ReadDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      || Some(path?.display().to_string()),
    )
  }

  pub fn revoke(&mut self, path: Option<&Path>) -> PermissionState {
    self.revoke_desc(
      path
        .map(|p| ReadDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
    )
  }

  pub fn check(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&ReadDescriptor(resolve_from_cwd(path)?)),
      true,
      api_name,
      || Some(format!("\"{}\"", path.display())),
    )
  }

  #[inline]
  pub fn check_partial(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let desc = ReadDescriptor(resolve_from_cwd(path)?);
    self.check_desc(Some(&desc), false, api_name, || {
      Some(format!("\"{}\"", path.display()))
    })
  }

  /// As `check()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  pub fn check_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let desc = ReadDescriptor(resolve_from_cwd(path)?);
    self.check_desc(Some(&desc), false, Some(api_name), || {
      Some(format!("<{display}>"))
    })
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name, || None)
  }
}

impl UnaryPermission<WriteDescriptor> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    self.query_desc(
      path
        .map(|p| WriteDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, path: Option<&Path>) -> PermissionState {
    self.request_desc(
      path
        .map(|p| WriteDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      || Some(path?.display().to_string()),
    )
  }

  pub fn revoke(&mut self, path: Option<&Path>) -> PermissionState {
    self.revoke_desc(
      path
        .map(|p| WriteDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
    )
  }

  pub fn check(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&WriteDescriptor(resolve_from_cwd(path)?)),
      true,
      api_name,
      || Some(format!("\"{}\"", path.display())),
    )
  }

  #[inline]
  pub fn check_partial(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&WriteDescriptor(resolve_from_cwd(path)?)),
      false,
      api_name,
      || Some(format!("\"{}\"", path.display())),
    )
  }

  /// As `check()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  pub fn check_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let desc = WriteDescriptor(resolve_from_cwd(path)?);
    self.check_desc(Some(&desc), false, Some(api_name), || {
      Some(format!("<{display}>"))
    })
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name, || None)
  }
}

impl UnaryPermission<NetDescriptor> {
  pub fn query(&self, host: Option<&NetDescriptor>) -> PermissionState {
    self.query_desc(host, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(&mut self, host: Option<&NetDescriptor>) -> PermissionState {
    self.request_desc(host, || None)
  }

  pub fn revoke(&mut self, host: Option<&NetDescriptor>) -> PermissionState {
    self.revoke_desc(host)
  }

  pub fn check(
    &mut self,
    host: &NetDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(host), false, api_name, || None)
  }

  pub fn check_url(
    &mut self,
    url: &url::Url,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let host = url
      .host_str()
      .ok_or_else(|| type_error(format!("Missing host in url: '{}'", url)))?;
    let host = host.parse::<Host>()?;
    let port = url.port_or_known_default();
    let descriptor = NetDescriptor(host, port);
    self.check_desc(Some(&descriptor), false, api_name, || {
      Some(format!("\"{descriptor}\""))
    })
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None, || None)
  }
}

impl UnaryPermission<EnvDescriptor> {
  pub fn query(&self, env: Option<&str>) -> PermissionState {
    self.query_desc(
      env.map(EnvDescriptor::new).as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, env: Option<&str>) -> PermissionState {
    self.request_desc(env.map(EnvDescriptor::new).as_ref(), || None)
  }

  pub fn revoke(&mut self, env: Option<&str>) -> PermissionState {
    self.revoke_desc(env.map(EnvDescriptor::new).as_ref())
  }

  pub fn check(
    &mut self,
    env: &str,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(&EnvDescriptor::new(env)), false, api_name, || None)
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None, || None)
  }
}

impl UnaryPermission<SysDescriptor> {
  pub fn query(&self, kind: Option<&str>) -> PermissionState {
    self.query_desc(
      kind.map(|k| SysDescriptor(k.to_string())).as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, kind: Option<&str>) -> PermissionState {
    self
      .request_desc(kind.map(|k| SysDescriptor(k.to_string())).as_ref(), || {
        None
      })
  }

  pub fn revoke(&mut self, kind: Option<&str>) -> PermissionState {
    self.revoke_desc(kind.map(|k| SysDescriptor(k.to_string())).as_ref())
  }

  pub fn check(
    &mut self,
    kind: &str,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&SysDescriptor(kind.to_string())),
      false,
      api_name,
      || None,
    )
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None, || None)
  }
}

impl UnaryPermission<RunDescriptor> {
  pub fn query(&self, cmd: Option<&str>) -> PermissionState {
    self.query_desc(
      cmd.map(|c| RunDescriptor::from(c.to_string())).as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, cmd: Option<&str>) -> PermissionState {
    self.request_desc(
      cmd.map(|c| RunDescriptor::from(c.to_string())).as_ref(),
      || Some(cmd?.to_string()),
    )
  }

  pub fn revoke(&mut self, cmd: Option<&str>) -> PermissionState {
    self.revoke_desc(cmd.map(|c| RunDescriptor::from(c.to_string())).as_ref())
  }

  pub fn check(
    &mut self,
    cmd: &str,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&RunDescriptor::from(cmd.to_string())),
      false,
      api_name,
      || Some(format!("\"{}\"", cmd)),
    )
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name, || None)
  }
}

impl UnaryPermission<FfiDescriptor> {
  pub fn query(&self, path: Option<&Path>) -> PermissionState {
    self.query_desc(
      path
        .map(|p| FfiDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      AllowPartial::TreatAsPartialGranted,
    )
  }

  pub fn request(&mut self, path: Option<&Path>) -> PermissionState {
    self.request_desc(
      path
        .map(|p| FfiDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
      || Some(path?.display().to_string()),
    )
  }

  pub fn revoke(&mut self, path: Option<&Path>) -> PermissionState {
    self.revoke_desc(
      path
        .map(|p| FfiDescriptor(resolve_from_cwd(p).unwrap()))
        .as_ref(),
    )
  }

  pub fn check(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(
      Some(&FfiDescriptor(resolve_from_cwd(path)?)),
      true,
      api_name,
      || Some(format!("\"{}\"", path.display())),
    )
  }

  pub fn check_partial(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let desc = match path {
      Some(path) => Some(FfiDescriptor(resolve_from_cwd(path)?)),
      None => None,
    };
    self.check_desc(desc.as_ref(), false, None, || {
      Some(format!("\"{}\"", path?.display()))
    })
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, Some("all"), || None)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Permissions {
  pub read: UnaryPermission<ReadDescriptor>,
  pub write: UnaryPermission<WriteDescriptor>,
  pub net: UnaryPermission<NetDescriptor>,
  pub env: UnaryPermission<EnvDescriptor>,
  pub sys: UnaryPermission<SysDescriptor>,
  pub run: UnaryPermission<RunDescriptor>,
  pub ffi: UnaryPermission<FfiDescriptor>,
  pub all: UnitPermission,
  pub hrtime: UnitPermission,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionsOptions {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub allow_hrtime: bool,
  pub deny_hrtime: bool,
  pub allow_net: Option<Vec<String>>,
  pub deny_net: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<PathBuf>>,
  pub deny_ffi: Option<Vec<PathBuf>>,
  pub allow_read: Option<Vec<PathBuf>>,
  pub deny_read: Option<Vec<PathBuf>>,
  pub allow_run: Option<Vec<String>>,
  pub deny_run: Option<Vec<String>>,
  pub allow_sys: Option<Vec<String>>,
  pub deny_sys: Option<Vec<String>>,
  pub allow_write: Option<Vec<PathBuf>>,
  pub deny_write: Option<Vec<PathBuf>>,
  pub prompt: bool,
}

impl Permissions {
  pub fn new_unary<T>(
    allow_list: &Option<Vec<T::Arg>>,
    deny_list: &Option<Vec<T::Arg>>,
    prompt: bool,
  ) -> Result<UnaryPermission<T>, AnyError>
  where
    T: Descriptor + Hash,
  {
    Ok(UnaryPermission::<T> {
      granted_global: global_from_option(allow_list),
      granted_list: T::parse(allow_list)?,
      flag_denied_global: global_from_option(deny_list),
      flag_denied_list: T::parse(deny_list)?,
      prompt,
      ..Default::default()
    })
  }

  pub const fn new_hrtime(
    allow_state: bool,
    deny_state: bool,
  ) -> UnitPermission {
    unit_permission_from_flag_bools(
      allow_state,
      deny_state,
      "hrtime",
      "high precision time",
      false, // never prompt for hrtime
    )
  }

  pub const fn new_all(allow_state: bool) -> UnitPermission {
    unit_permission_from_flag_bools(
      allow_state,
      false,
      "all",
      "all",
      false, // never prompt for all
    )
  }

  pub fn from_options(opts: &PermissionsOptions) -> Result<Self, AnyError> {
    Ok(Self {
      read: Permissions::new_unary(
        &opts.allow_read,
        &opts.deny_read,
        opts.prompt,
      )?,
      write: Permissions::new_unary(
        &opts.allow_write,
        &opts.deny_write,
        opts.prompt,
      )?,
      net: Permissions::new_unary(
        &opts.allow_net,
        &opts.deny_net,
        opts.prompt,
      )?,
      env: Permissions::new_unary(
        &opts.allow_env,
        &opts.deny_env,
        opts.prompt,
      )?,
      sys: Permissions::new_unary(
        &opts.allow_sys,
        &opts.deny_sys,
        opts.prompt,
      )?,
      run: Permissions::new_unary(
        &opts.allow_run,
        &opts.deny_run,
        opts.prompt,
      )?,
      ffi: Permissions::new_unary(
        &opts.allow_ffi,
        &opts.deny_ffi,
        opts.prompt,
      )?,
      all: Permissions::new_all(opts.allow_all),
      hrtime: Permissions::new_hrtime(opts.allow_hrtime, opts.deny_hrtime),
    })
  }

  /// Create a set of permissions that explicitly allow everything.
  pub fn allow_all() -> Self {
    Self {
      read: UnaryPermission::allow_all(),
      write: UnaryPermission::allow_all(),
      net: UnaryPermission::allow_all(),
      env: UnaryPermission::allow_all(),
      sys: UnaryPermission::allow_all(),
      run: UnaryPermission::allow_all(),
      ffi: UnaryPermission::allow_all(),
      all: Permissions::new_all(true),
      hrtime: Permissions::new_hrtime(true, false),
    }
  }

  /// Create a set of permissions that enable nothing, but will allow prompting.
  pub fn none_with_prompt() -> Self {
    Self::none(true)
  }

  /// Create a set of permissions that enable nothing, and will not allow prompting.
  pub fn none_without_prompt() -> Self {
    Self::none(false)
  }

  fn none(prompt: bool) -> Self {
    Self {
      read: Permissions::new_unary(&None, &None, prompt).unwrap(),
      write: Permissions::new_unary(&None, &None, prompt).unwrap(),
      net: Permissions::new_unary(&None, &None, prompt).unwrap(),
      env: Permissions::new_unary(&None, &None, prompt).unwrap(),
      sys: Permissions::new_unary(&None, &None, prompt).unwrap(),
      run: Permissions::new_unary(&None, &None, prompt).unwrap(),
      ffi: Permissions::new_unary(&None, &None, prompt).unwrap(),
      all: Permissions::new_all(false),
      hrtime: Permissions::new_hrtime(false, false),
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
        Ok(path) => self.read.check(&path, Some("import()")),
        Err(_) => Err(uri_error(format!(
          "Invalid file path.\n  Specifier: {specifier}"
        ))),
      },
      "data" => Ok(()),
      "blob" => Ok(()),
      _ => self.net.check_url(specifier, Some("import()")),
    }
  }
}

/// Wrapper struct for `Permissions` that can be shared across threads.
///
/// We need a way to have internal mutability for permissions as they might get
/// passed to a future that will prompt the user for permission (and in such
/// case might need to be mutated). Also for the Web Worker API we need a way
/// to send permissions to a new thread.
#[derive(Clone, Debug)]
pub struct PermissionsContainer(pub Arc<Mutex<Permissions>>);

impl PermissionsContainer {
  pub fn new(perms: Permissions) -> Self {
    Self(Arc::new(Mutex::new(perms)))
  }

  #[inline(always)]
  pub fn allow_hrtime(&mut self) -> bool {
    self.0.lock().hrtime.check().is_ok()
  }

  pub fn allow_all() -> Self {
    Self::new(Permissions::allow_all())
  }

  #[inline(always)]
  pub fn check_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    self.0.lock().check_specifier(specifier)
  }

  #[inline(always)]
  pub fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().read.check(path, Some(api_name))
  }

  #[inline(always)]
  pub fn check_read_with_api_name(
    &self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    self.0.lock().read.check(path, api_name)
  }

  #[inline(always)]
  pub fn check_read_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().read.check_blind(path, display, api_name)
  }

  #[inline(always)]
  pub fn check_read_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.0.lock().read.check_all(Some(api_name))
  }

  #[inline(always)]
  pub fn check_write(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().write.check(path, Some(api_name))
  }

  #[inline(always)]
  pub fn check_write_with_api_name(
    &self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    self.0.lock().write.check(path, api_name)
  }

  #[inline(always)]
  pub fn check_write_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.0.lock().write.check_all(Some(api_name))
  }

  #[inline(always)]
  pub fn check_write_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().write.check_blind(path, display, api_name)
  }

  #[inline(always)]
  pub fn check_write_partial(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().write.check_partial(path, Some(api_name))
  }

  #[inline(always)]
  pub fn check_run(
    &mut self,
    cmd: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().run.check(cmd, Some(api_name))
  }

  #[inline(always)]
  pub fn check_run_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.0.lock().run.check_all(Some(api_name))
  }

  #[inline(always)]
  pub fn check_sys(&self, kind: &str, api_name: &str) -> Result<(), AnyError> {
    self.0.lock().sys.check(kind, Some(api_name))
  }

  #[inline(always)]
  pub fn check_env(&mut self, var: &str) -> Result<(), AnyError> {
    self.0.lock().env.check(var, None)
  }

  #[inline(always)]
  pub fn check_env_all(&mut self) -> Result<(), AnyError> {
    self.0.lock().env.check_all()
  }

  #[inline(always)]
  pub fn check_sys_all(&mut self) -> Result<(), AnyError> {
    self.0.lock().sys.check_all()
  }

  #[inline(always)]
  pub fn check_ffi_all(&mut self) -> Result<(), AnyError> {
    self.0.lock().ffi.check_all()
  }

  /// This checks to see if the allow-all flag was passed, not whether all
  /// permissions are enabled!
  #[inline(always)]
  pub fn check_was_allow_all_flag_passed(&mut self) -> Result<(), AnyError> {
    self.0.lock().all.check()
  }

  /// Checks special file access, returning the failed permission type if
  /// not successful.
  pub fn check_special_file(
    &mut self,
    path: &Path,
    _api_name: &str,
  ) -> Result<(), &'static str> {
    let error_all = |_| "all";

    // Safe files with no major additional side-effects. While there's a small risk of someone
    // draining system entropy by just reading one of these files constantly, that's not really
    // something we worry about as they already have --allow-read to /dev.
    if cfg!(unix)
      && (path == OsStr::new("/dev/random")
        || path == OsStr::new("/dev/urandom")
        || path == OsStr::new("/dev/zero")
        || path == OsStr::new("/dev/null"))
    {
      return Ok(());
    }

    /// We'll allow opening /proc/self/fd/{n} without additional permissions under the following conditions:
    ///
    /// 1. n > 2. This allows for opening bash-style redirections, but not stdio
    /// 2. the fd referred to by n is a pipe
    #[cfg(unix)]
    fn is_fd_file_is_pipe(path: &Path) -> bool {
      if let Some(fd) = path.file_name() {
        if let Ok(s) = std::str::from_utf8(fd.as_encoded_bytes()) {
          if let Ok(n) = s.parse::<i32>() {
            if n > 2 {
              // SAFETY: This is proper use of the stat syscall
              unsafe {
                let mut stat = std::mem::zeroed::<libc::stat>();
                if libc::fstat(n, &mut stat as _) == 0
                  && ((stat.st_mode & libc::S_IFMT) & libc::S_IFIFO) != 0
                {
                  return true;
                }
              };
            }
          }
        }
      }
      false
    }

    // On unixy systems, we allow opening /dev/fd/XXX for valid FDs that
    // are pipes.
    #[cfg(unix)]
    if path.starts_with("/dev/fd") && is_fd_file_is_pipe(path) {
      return Ok(());
    }

    if cfg!(target_os = "linux") {
      // On Linux, we also allow opening /proc/self/fd/XXX for valid FDs that
      // are pipes.
      #[cfg(unix)]
      if path.starts_with("/proc/self/fd") && is_fd_file_is_pipe(path) {
        return Ok(());
      }
      if path.starts_with("/dev")
        || path.starts_with("/proc")
        || path.starts_with("/sys")
      {
        if path.ends_with("/environ") {
          self.check_env_all().map_err(|_| "env")?;
        } else {
          self.check_was_allow_all_flag_passed().map_err(error_all)?;
        }
      }
    } else if cfg!(unix) {
      if path.starts_with("/dev") {
        self.check_was_allow_all_flag_passed().map_err(error_all)?;
      }
    } else if cfg!(target_os = "windows") {
      // \\.\nul is allowed
      let s = path.as_os_str().as_encoded_bytes();
      if s.eq_ignore_ascii_case(br#"\\.\nul"#) {
        return Ok(());
      }

      fn is_normalized_windows_drive_path(path: &Path) -> bool {
        let s = path.as_os_str().as_encoded_bytes();
        // \\?\X:\
        if s.len() < 7 {
          false
        } else if s.starts_with(br#"\\?\"#) {
          s[4].is_ascii_alphabetic() && s[5] == b':' && s[6] == b'\\'
        } else {
          false
        }
      }

      // If this is a normalized drive path, accept it
      if !is_normalized_windows_drive_path(path) {
        self.check_was_allow_all_flag_passed().map_err(error_all)?;
      }
    } else {
      unimplemented!()
    }
    Ok(())
  }

  #[inline(always)]
  pub fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.lock().net.check_url(url, Some(api_name))
  }

  #[inline(always)]
  pub fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
    api_name: &str,
  ) -> Result<(), AnyError> {
    let hostname = host.0.as_ref().parse::<Host>()?;
    let descriptor = NetDescriptor(hostname, host.1);
    self.0.lock().net.check(&descriptor, Some(api_name))
  }

  #[inline(always)]
  pub fn check_ffi(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    self.0.lock().ffi.check(path.unwrap(), None)
  }

  #[inline(always)]
  pub fn check_ffi_partial(
    &mut self,
    path: Option<&Path>,
  ) -> Result<(), AnyError> {
    self.0.lock().ffi.check_partial(path)
  }
}

const fn unit_permission_from_flag_bools(
  allow_flag: bool,
  deny_flag: bool,
  name: &'static str,
  description: &'static str,
  prompt: bool,
) -> UnitPermission {
  UnitPermission {
    name,
    description,
    state: if deny_flag {
      PermissionState::Denied
    } else if allow_flag {
      PermissionState::Granted
    } else {
      PermissionState::Prompt
    },
    prompt,
  }
}

fn global_from_option<T>(flag: &Option<Vec<T>>) -> bool {
  matches!(flag, Some(v) if v.is_empty())
}

fn parse_net_list(
  list: &Option<Vec<String>>,
) -> Result<HashSet<NetDescriptor>, AnyError> {
  if let Some(v) = list {
    v.iter()
      .map(|x| NetDescriptor::from_str(x))
      .collect::<Result<HashSet<NetDescriptor>, AnyError>>()
  } else {
    Ok(HashSet::new())
  }
}

fn parse_env_list(
  list: &Option<Vec<String>>,
) -> Result<HashSet<EnvDescriptor>, AnyError> {
  if let Some(v) = list {
    v.iter()
      .map(|x| {
        if x.is_empty() {
          Err(AnyError::msg("Empty path is not allowed"))
        } else {
          Ok(EnvDescriptor::new(x))
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

fn parse_path_list<T: Descriptor + Hash>(
  list: &Option<Vec<PathBuf>>,
  f: fn(PathBuf) -> T,
) -> Result<HashSet<T>, AnyError> {
  if let Some(v) = list {
    v.iter()
      .map(|raw_path| {
        if raw_path.as_os_str().is_empty() {
          Err(AnyError::msg("Empty path is not allowed"))
        } else {
          resolve_from_cwd(Path::new(&raw_path)).map(f)
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

fn parse_sys_list(
  list: &Option<Vec<String>>,
) -> Result<HashSet<SysDescriptor>, AnyError> {
  if let Some(v) = list {
    v.iter()
      .map(|x| {
        if x.is_empty() {
          Err(AnyError::msg("empty"))
        } else {
          Ok(SysDescriptor(x.to_string()))
        }
      })
      .collect()
  } else {
    Ok(HashSet::new())
  }
}

fn parse_run_list(
  list: &Option<Vec<String>>,
) -> Result<HashSet<RunDescriptor>, AnyError> {
  let mut result = HashSet::new();
  if let Some(v) = list {
    for s in v {
      if s.is_empty() {
        return Err(AnyError::msg("Empty path is not allowed"));
      } else {
        let desc = RunDescriptor::from(s.to_string());
        let aliases = desc.aliases();
        result.insert(desc);
        result.extend(aliases);
      }
    }
  }
  Ok(result)
}

fn escalation_error() -> AnyError {
  custom_error(
    "PermissionDenied",
    "Can't escalate parent thread permissions",
  )
}

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Eq, PartialEq)]
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
#[derive(Debug, Eq, PartialEq)]
pub struct ChildPermissionsArg {
  env: ChildUnaryPermissionArg,
  hrtime: ChildUnitPermissionArg,
  net: ChildUnaryPermissionArg,
  ffi: ChildUnaryPermissionArg,
  read: ChildUnaryPermissionArg,
  run: ChildUnaryPermissionArg,
  sys: ChildUnaryPermissionArg,
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
      sys: ChildUnaryPermissionArg::Inherit,
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
      sys: ChildUnaryPermissionArg::NotGranted,
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
              de::Error::custom(format!("(deno.permissions.env) {e}"))
            })?;
          } else if key == "hrtime" {
            let arg = serde_json::from_value::<ChildUnitPermissionArg>(value);
            child_permissions_arg.hrtime = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.hrtime) {e}"))
            })?;
          } else if key == "net" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.net = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.net) {e}"))
            })?;
          } else if key == "ffi" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.ffi = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.ffi) {e}"))
            })?;
          } else if key == "read" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.read = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.read) {e}"))
            })?;
          } else if key == "run" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.run = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.run) {e}"))
            })?;
          } else if key == "sys" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.sys = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.sys) {e}"))
            })?;
          } else if key == "write" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.write = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.write) {e}"))
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
  let mut worker_perms = Permissions::none_without_prompt();
  worker_perms.read = main_perms
    .read
    .create_child_permissions(child_permissions_arg.read)?;
  worker_perms.write = main_perms
    .write
    .create_child_permissions(child_permissions_arg.write)?;
  worker_perms.net = main_perms
    .net
    .create_child_permissions(child_permissions_arg.net)?;
  worker_perms.env = main_perms
    .env
    .create_child_permissions(child_permissions_arg.env)?;
  worker_perms.sys = main_perms
    .sys
    .create_child_permissions(child_permissions_arg.sys)?;
  worker_perms.run = main_perms
    .run
    .create_child_permissions(child_permissions_arg.run)?;
  worker_perms.ffi = main_perms
    .ffi
    .create_child_permissions(child_permissions_arg.ffi)?;
  worker_perms.hrtime = main_perms
    .hrtime
    .create_child_permissions(child_permissions_arg.hrtime)?;
  worker_perms.all = main_perms
    .all
    .create_child_permissions(ChildUnitPermissionArg::Inherit)?;

  Ok(worker_perms)
}

#[derive(Clone, Debug)]
pub struct IsStandaloneBinary(bool);

static mut SINGLETON: Option<IsStandaloneBinary> = None;
static INIT: Once = Once::new();

impl IsStandaloneBinary {
  pub fn new() -> Self {
    Self(false)
  }

  pub fn new_for_standalone_binary() -> Self {
    Self(true)
  }

  pub fn is_standalone_binary(&self) -> bool {
    self.0
  }

  pub fn get_instance(standalone: bool) -> &'static IsStandaloneBinary {
    // SAFETY: runtime calls
    unsafe {
      INIT.call_once(|| {
        if standalone {
          SINGLETON = Some(IsStandaloneBinary::new_for_standalone_binary());
        } else {
          SINGLETON = Some(IsStandaloneBinary::new());
        }
      });
      SINGLETON.as_ref().unwrap()
    }
  }
}

impl Default for IsStandaloneBinary {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json::json;
  use fqdn::fqdn;
  use prompter::tests::*;
  use std::net::Ipv4Addr;

  // Creates vector of strings, Vec<String>
  macro_rules! svec {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
  }

  #[test]
  fn check_paths() {
    set_prompter(Box::new(TestPrompter));
    let allowlist = vec![
      PathBuf::from("/a/specific/dir/name"),
      PathBuf::from("/a/specific"),
      PathBuf::from("/b/c"),
    ];

    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_read: Some(allowlist.clone()),
      allow_write: Some(allowlist.clone()),
      allow_ffi: Some(allowlist),
      ..Default::default()
    })
    .unwrap();

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms
      .read
      .check(Path::new("/a/specific/dir/name"), None)
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/dir/name"), None)
      .is_ok());
    assert!(perms
      .ffi
      .check(Path::new("/a/specific/dir/name"), None)
      .is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.read.check(Path::new("/a/specific/dir"), None).is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/dir"), None)
      .is_ok());
    assert!(perms.ffi.check(Path::new("/a/specific/dir"), None).is_ok());

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms
      .read
      .check(Path::new("/a/specific/dir/name/inner"), None)
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/dir/name/inner"), None)
      .is_ok());
    assert!(perms
      .ffi
      .check(Path::new("/a/specific/dir/name/inner"), None)
      .is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms
      .read
      .check(Path::new("/a/specific/other/dir"), None)
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/a/specific/other/dir"), None)
      .is_ok());
    assert!(perms
      .ffi
      .check(Path::new("/a/specific/other/dir"), None)
      .is_ok());

    // Exact match with /b/c
    assert!(perms.read.check(Path::new("/b/c"), None).is_ok());
    assert!(perms.write.check(Path::new("/b/c"), None).is_ok());
    assert!(perms.ffi.check(Path::new("/b/c"), None).is_ok());

    // Sub path within /b/c
    assert!(perms.read.check(Path::new("/b/c/sub/path"), None).is_ok());
    assert!(perms.write.check(Path::new("/b/c/sub/path"), None).is_ok());
    assert!(perms.ffi.check(Path::new("/b/c/sub/path"), None).is_ok());

    // Sub path within /b/c, needs normalizing
    assert!(perms
      .read
      .check(Path::new("/b/c/sub/path/../path/."), None)
      .is_ok());
    assert!(perms
      .write
      .check(Path::new("/b/c/sub/path/../path/."), None)
      .is_ok());
    assert!(perms
      .ffi
      .check(Path::new("/b/c/sub/path/../path/."), None)
      .is_ok());

    // Inside of /b but outside of /b/c
    assert!(perms.read.check(Path::new("/b/e"), None).is_err());
    assert!(perms.write.check(Path::new("/b/e"), None).is_err());
    assert!(perms.ffi.check(Path::new("/b/e"), None).is_err());

    // Inside of /a but outside of /a/specific
    assert!(perms.read.check(Path::new("/a/b"), None).is_err());
    assert!(perms.write.check(Path::new("/a/b"), None).is_err());
    assert!(perms.ffi.check(Path::new("/a/b"), None).is_err());
  }

  #[test]
  fn test_check_net_with_values() {
    set_prompter(Box::new(TestPrompter));
    let mut perms = Permissions::from_options(&PermissionsOptions {
      allow_net: Some(svec![
        "localhost",
        "deno.land",
        "github.com:3000",
        "127.0.0.1",
        "172.16.0.2:8000",
        "www.github.com:443",
        "80.example.com:80",
        "443.example.com:443"
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
      ("443.example.com", 444, false),
      ("80.example.com", 81, false),
      ("80.example.com", 80, true),
      // Just some random hosts that should err
      ("somedomain", 0, false),
      ("192.168.0.1", 0, false),
    ];

    for (host, port, is_ok) in domain_tests {
      let host = host.parse().unwrap();
      let descriptor = NetDescriptor(host, Some(port));
      assert_eq!(
        is_ok,
        perms.net.check(&descriptor, None).is_ok(),
        "{descriptor}",
      );
    }
  }

  #[test]
  fn test_check_net_only_flag() {
    set_prompter(Box::new(TestPrompter));
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

    for (host_str, port) in domain_tests {
      let host = host_str.parse().unwrap();
      let descriptor = NetDescriptor(host, Some(port));
      assert!(
        perms.net.check(&descriptor, None).is_ok(),
        "expected {host_str}:{port} to pass"
      );
    }
  }

  #[test]
  fn test_check_net_no_flag() {
    set_prompter(Box::new(TestPrompter));
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

    for (host_str, port) in domain_tests {
      let host = host_str.parse().unwrap();
      let descriptor = NetDescriptor(host, Some(port));
      assert!(
        perms.net.check(&descriptor, None).is_err(),
        "expected {host_str}:{port} to fail"
      );
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
      assert_eq!(is_ok, perms.net.check_url(&u, None).is_ok(), "{}", u);
    }
  }

  #[test]
  fn check_specifiers() {
    set_prompter(Box::new(TestPrompter));
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
        ModuleSpecifier::parse("http://localhost:4545/mod.ts").unwrap(),
        true,
      ),
      (
        ModuleSpecifier::parse("http://deno.land/x/mod.ts").unwrap(),
        false,
      ),
      (
        ModuleSpecifier::parse("data:text/plain,Hello%2C%20Deno!").unwrap(),
        true,
      ),
    ];

    if cfg!(target_os = "windows") {
      fixtures
        .push((ModuleSpecifier::parse("file:///C:/a/mod.ts").unwrap(), true));
      fixtures.push((
        ModuleSpecifier::parse("file:///C:/b/mod.ts").unwrap(),
        false,
      ));
    } else {
      fixtures
        .push((ModuleSpecifier::parse("file:///a/mod.ts").unwrap(), true));
      fixtures
        .push((ModuleSpecifier::parse("file:///b/mod.ts").unwrap(), false));
    }

    for (specifier, expected) in fixtures {
      assert_eq!(
        perms.check_specifier(&specifier).is_ok(),
        expected,
        "{}",
        specifier,
      );
    }
  }

  #[test]
  fn check_invalid_specifiers() {
    set_prompter(Box::new(TestPrompter));
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
        .check_specifier(&ModuleSpecifier::parse(url).unwrap())
        .is_err());
    }
  }

  #[test]
  fn test_query() {
    set_prompter(Box::new(TestPrompter));
    let perms1 = Permissions::allow_all();
    let perms2 = Permissions {
      read: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo")]),
        &None,
        false,
      )
      .unwrap(),
      write: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo")]),
        &None,
        false,
      )
      .unwrap(),
      ffi: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo")]),
        &None,
        false,
      )
      .unwrap(),
      net: Permissions::new_unary(&Some(svec!["127.0.0.1:8000"]), &None, false)
        .unwrap(),
      env: Permissions::new_unary(&Some(svec!["HOME"]), &None, false).unwrap(),
      sys: Permissions::new_unary(&Some(svec!["hostname"]), &None, false)
        .unwrap(),
      run: Permissions::new_unary(&Some(svec!["deno"]), &None, false).unwrap(),
      all: Permissions::new_all(false),
      hrtime: Permissions::new_hrtime(false, false),
    };
    let perms3 = Permissions {
      read: Permissions::new_unary(
        &None,
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      write: Permissions::new_unary(
        &None,
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      ffi: Permissions::new_unary(
        &None,
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      net: Permissions::new_unary(&None, &Some(svec!["127.0.0.1:8000"]), false)
        .unwrap(),
      env: Permissions::new_unary(&None, &Some(svec!["HOME"]), false).unwrap(),
      sys: Permissions::new_unary(&None, &Some(svec!["hostname"]), false)
        .unwrap(),
      run: Permissions::new_unary(&None, &Some(svec!["deno"]), false).unwrap(),
      all: Permissions::new_all(false),
      hrtime: Permissions::new_hrtime(false, true),
    };
    let perms4 = Permissions {
      read: Permissions::new_unary(
        &Some(vec![]),
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      write: Permissions::new_unary(
        &Some(vec![]),
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      ffi: Permissions::new_unary(
        &Some(vec![]),
        &Some(vec![PathBuf::from("/foo")]),
        false,
      )
      .unwrap(),
      net: Permissions::new_unary(
        &Some(vec![]),
        &Some(svec!["127.0.0.1:8000"]),
        false,
      )
      .unwrap(),
      env: Permissions::new_unary(&Some(vec![]), &Some(svec!["HOME"]), false)
        .unwrap(),
      sys: Permissions::new_unary(
        &Some(vec![]),
        &Some(svec!["hostname"]),
        false,
      )
      .unwrap(),
      run: Permissions::new_unary(&Some(vec![]), &Some(svec!["deno"]), false)
        .unwrap(),
      all: Permissions::new_all(false),
      hrtime: Permissions::new_hrtime(true, true),
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms1.read.query(None), PermissionState::Granted);
      assert_eq!(perms1.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(None), PermissionState::Prompt);
      assert_eq!(perms2.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.read.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms3.read.query(None), PermissionState::Prompt);
      assert_eq!(perms3.read.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms3.read.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.read.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.read.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms4.read.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.read.query(Some(Path::new("/bar"))), PermissionState::Granted);
      assert_eq!(perms1.write.query(None), PermissionState::Granted);
      assert_eq!(perms1.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(None), PermissionState::Prompt);
      assert_eq!(perms2.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.write.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms3.write.query(None), PermissionState::Prompt);
      assert_eq!(perms3.write.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms3.write.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.write.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.write.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms4.write.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.write.query(Some(Path::new("/bar"))), PermissionState::Granted);
      assert_eq!(perms1.ffi.query(None), PermissionState::Granted);
      assert_eq!(perms1.ffi.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.ffi.query(None), PermissionState::Prompt);
      assert_eq!(perms2.ffi.query(Some(Path::new("/foo"))), PermissionState::Granted);
      assert_eq!(perms2.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
      assert_eq!(perms3.ffi.query(None), PermissionState::Prompt);
      assert_eq!(perms3.ffi.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms3.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.ffi.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.ffi.query(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms4.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
      assert_eq!(perms4.ffi.query(Some(Path::new("/bar"))), PermissionState::Granted);
      assert_eq!(perms1.net.query(None), PermissionState::Granted);
      assert_eq!(perms1.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), None))), PermissionState::Granted);
      assert_eq!(perms2.net.query(None), PermissionState::Prompt);
      assert_eq!(perms2.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)))), PermissionState::Granted);
      assert_eq!(perms3.net.query(None), PermissionState::Prompt);
      assert_eq!(perms3.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)))), PermissionState::Denied);
      assert_eq!(perms4.net.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)))), PermissionState::Denied);
      assert_eq!(perms4.net.query(Some(&NetDescriptor("192.168.0.1".parse().unwrap(), Some(8000)))), PermissionState::Granted);
      assert_eq!(perms1.env.query(None), PermissionState::Granted);
      assert_eq!(perms1.env.query(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms2.env.query(None), PermissionState::Prompt);
      assert_eq!(perms2.env.query(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms3.env.query(None), PermissionState::Prompt);
      assert_eq!(perms3.env.query(Some("HOME")), PermissionState::Denied);
      assert_eq!(perms4.env.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.env.query(Some("HOME")), PermissionState::Denied);
      assert_eq!(perms4.env.query(Some("AWAY")), PermissionState::Granted);
      assert_eq!(perms1.sys.query(None), PermissionState::Granted);
      assert_eq!(perms1.sys.query(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms2.sys.query(None), PermissionState::Prompt);
      assert_eq!(perms2.sys.query(Some("hostname")), PermissionState::Granted);
      assert_eq!(perms3.sys.query(None), PermissionState::Prompt);
      assert_eq!(perms3.sys.query(Some("hostname")), PermissionState::Denied);
      assert_eq!(perms4.sys.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.sys.query(Some("hostname")), PermissionState::Denied);
      assert_eq!(perms4.sys.query(Some("uid")), PermissionState::Granted);
      assert_eq!(perms1.run.query(None), PermissionState::Granted);
      assert_eq!(perms1.run.query(Some("deno")), PermissionState::Granted);
      assert_eq!(perms2.run.query(None), PermissionState::Prompt);
      assert_eq!(perms2.run.query(Some("deno")), PermissionState::Granted);
      assert_eq!(perms3.run.query(None), PermissionState::Prompt);
      assert_eq!(perms3.run.query(Some("deno")), PermissionState::Denied);
      assert_eq!(perms4.run.query(None), PermissionState::GrantedPartial);
      assert_eq!(perms4.run.query(Some("deno")), PermissionState::Denied);
      assert_eq!(perms4.run.query(Some("node")), PermissionState::Granted);
      assert_eq!(perms1.hrtime.query(), PermissionState::Granted);
      assert_eq!(perms2.hrtime.query(), PermissionState::Prompt);
      assert_eq!(perms3.hrtime.query(), PermissionState::Denied);
      assert_eq!(perms4.hrtime.query(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_request() {
    set_prompter(Box::new(TestPrompter));
    let mut perms: Permissions = Permissions::none_without_prompt();
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
      prompt_value.set(false);
      assert_eq!(perms.ffi.request(Some(Path::new("/foo"))), PermissionState::Denied);
      assert_eq!(perms.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      prompt_value.set(true);
      assert_eq!(perms.ffi.request(None), PermissionState::Denied);
      prompt_value.set(true);
      assert_eq!(perms.net.request(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), None))), PermissionState::Granted);
      prompt_value.set(false);
      assert_eq!(perms.net.request(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)))), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
      assert_eq!(perms.env.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.sys.request(Some("hostname")), PermissionState::Granted);
      assert_eq!(perms.sys.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.sys.request(Some("hostname")), PermissionState::Granted);
      prompt_value.set(true);
      assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
      assert_eq!(perms.run.query(None), PermissionState::Prompt);
      prompt_value.set(false);
      assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
      prompt_value.set(false);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
      prompt_value.set(true);
      assert_eq!(perms.hrtime.request(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_revoke() {
    set_prompter(Box::new(TestPrompter));
    let mut perms = Permissions {
      read: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
        &None,
        false,
      )
      .unwrap(),
      write: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
        &None,
        false,
      )
      .unwrap(),
      ffi: Permissions::new_unary(
        &Some(vec![PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
        &None,
        false,
      )
      .unwrap(),
      net: Permissions::new_unary(
        &Some(svec!["127.0.0.1", "127.0.0.1:8000"]),
        &None,
        false,
      )
      .unwrap(),
      env: Permissions::new_unary(&Some(svec!["HOME"]), &None, false).unwrap(),
      sys: Permissions::new_unary(&Some(svec!["hostname"]), &None, false)
        .unwrap(),
      run: Permissions::new_unary(&Some(svec!["deno"]), &None, false).unwrap(),
      all: Permissions::new_all(false),
      hrtime: Permissions::new_hrtime(false, true),
    };
    #[rustfmt::skip]
    {
      assert_eq!(perms.read.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.read.query(Some(Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.read.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
      assert_eq!(perms.write.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.write.query(Some(Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.write.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
      assert_eq!(perms.ffi.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
      assert_eq!(perms.ffi.query(Some(Path::new("/foo"))), PermissionState::Prompt);
      assert_eq!(perms.ffi.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
      assert_eq!(perms.net.revoke(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(9000)))), PermissionState::Prompt);
      assert_eq!(perms.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), None))), PermissionState::Prompt);
      assert_eq!(perms.net.query(Some(&NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)))), PermissionState::Granted);
      assert_eq!(perms.env.revoke(Some("HOME")), PermissionState::Prompt);
      assert_eq!(perms.env.revoke(Some("hostname")), PermissionState::Prompt);
      assert_eq!(perms.run.revoke(Some("deno")), PermissionState::Prompt);
      assert_eq!(perms.hrtime.revoke(), PermissionState::Denied);
    };
  }

  #[test]
  fn test_check() {
    set_prompter(Box::new(TestPrompter));
    let mut perms = Permissions::none_with_prompt();
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

    prompt_value.set(true);
    assert!(perms.read.check(Path::new("/foo"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/foo"), None).is_ok());
    assert!(perms.read.check(Path::new("/bar"), None).is_err());

    prompt_value.set(true);
    assert!(perms.write.check(Path::new("/foo"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/foo"), None).is_ok());
    assert!(perms.write.check(Path::new("/bar"), None).is_err());

    prompt_value.set(true);
    assert!(perms.ffi.check(Path::new("/foo"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.ffi.check(Path::new("/foo"), None).is_ok());
    assert!(perms.ffi.check(Path::new("/bar"), None).is_err());

    prompt_value.set(true);
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)),
        None
      )
      .is_ok());
    prompt_value.set(false);
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)),
        None
      )
      .is_ok());
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8001)),
        None
      )
      .is_err());
    assert!(perms
      .net
      .check(&NetDescriptor("127.0.0.1".parse().unwrap(), None), None)
      .is_err());
    assert!(perms
      .net
      .check(
        &NetDescriptor("deno.land".parse().unwrap(), Some(8000)),
        None
      )
      .is_err());
    assert!(perms
      .net
      .check(&NetDescriptor("deno.land".parse().unwrap(), None), None)
      .is_err());

    prompt_value.set(true);
    assert!(perms.run.check("cat", None).is_ok());
    prompt_value.set(false);
    assert!(perms.run.check("cat", None).is_ok());
    assert!(perms.run.check("ls", None).is_err());

    prompt_value.set(true);
    assert!(perms.env.check("HOME", None).is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("HOME", None).is_ok());
    assert!(perms.env.check("PATH", None).is_err());

    prompt_value.set(true);
    assert!(perms.env.check("hostname", None).is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("hostname", None).is_ok());
    assert!(perms.env.check("osRelease", None).is_err());

    assert!(perms.hrtime.check().is_err());
  }

  #[test]
  fn test_check_fail() {
    set_prompter(Box::new(TestPrompter));
    let mut perms = Permissions::none_with_prompt();
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/foo"), None).is_err());
    prompt_value.set(true);
    assert!(perms.read.check(Path::new("/foo"), None).is_err());
    assert!(perms.read.check(Path::new("/bar"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.read.check(Path::new("/bar"), None).is_ok());

    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/foo"), None).is_err());
    prompt_value.set(true);
    assert!(perms.write.check(Path::new("/foo"), None).is_err());
    assert!(perms.write.check(Path::new("/bar"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.write.check(Path::new("/bar"), None).is_ok());

    prompt_value.set(false);
    assert!(perms.ffi.check(Path::new("/foo"), None).is_err());
    prompt_value.set(true);
    assert!(perms.ffi.check(Path::new("/foo"), None).is_err());
    assert!(perms.ffi.check(Path::new("/bar"), None).is_ok());
    prompt_value.set(false);
    assert!(perms.ffi.check(Path::new("/bar"), None).is_ok());

    prompt_value.set(false);
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)),
        None
      )
      .is_err());
    prompt_value.set(true);
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8000)),
        None
      )
      .is_err());
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8001)),
        None
      )
      .is_ok());
    assert!(perms
      .net
      .check(
        &NetDescriptor("deno.land".parse().unwrap(), Some(8000)),
        None
      )
      .is_ok());
    prompt_value.set(false);
    assert!(perms
      .net
      .check(
        &NetDescriptor("127.0.0.1".parse().unwrap(), Some(8001)),
        None
      )
      .is_ok());
    assert!(perms
      .net
      .check(
        &NetDescriptor("deno.land".parse().unwrap(), Some(8000)),
        None
      )
      .is_ok());

    prompt_value.set(false);
    assert!(perms.run.check("cat", None).is_err());
    prompt_value.set(true);
    assert!(perms.run.check("cat", None).is_err());
    assert!(perms.run.check("ls", None).is_ok());
    prompt_value.set(false);
    assert!(perms.run.check("ls", None).is_ok());

    prompt_value.set(false);
    assert!(perms.env.check("HOME", None).is_err());
    prompt_value.set(true);
    assert!(perms.env.check("HOME", None).is_err());
    assert!(perms.env.check("PATH", None).is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("PATH", None).is_ok());

    prompt_value.set(false);
    assert!(perms.sys.check("hostname", None).is_err());
    prompt_value.set(true);
    assert!(perms.sys.check("hostname", None).is_err());
    assert!(perms.sys.check("osRelease", None).is_ok());
    prompt_value.set(false);
    assert!(perms.sys.check("osRelease", None).is_ok());

    prompt_value.set(false);
    assert!(perms.hrtime.check().is_err());
    prompt_value.set(true);
    assert!(perms.hrtime.check().is_err());
  }

  #[test]
  #[cfg(windows)]
  fn test_env_windows() {
    set_prompter(Box::new(TestPrompter));
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
    let mut perms = Permissions::allow_all();
    perms.env = UnaryPermission {
      granted_global: false,
      ..Permissions::new_unary(&Some(svec!["HOME"]), &None, false).unwrap()
    };

    prompt_value.set(true);
    assert!(perms.env.check("HOME", None).is_ok());
    prompt_value.set(false);
    assert!(perms.env.check("HOME", None).is_ok());
    assert!(perms.env.check("hOmE", None).is_ok());

    assert_eq!(perms.env.revoke(Some("HomE")), PermissionState::Prompt);
  }

  #[test]
  fn test_check_partial_denied() {
    let mut perms = Permissions {
      read: Permissions::new_unary(
        &Some(vec![]),
        &Some(vec![PathBuf::from("/foo/bar")]),
        false,
      )
      .unwrap(),
      write: Permissions::new_unary(
        &Some(vec![]),
        &Some(vec![PathBuf::from("/foo/bar")]),
        false,
      )
      .unwrap(),
      ..Permissions::none_without_prompt()
    };

    perms.read.check_partial(Path::new("/foo"), None).unwrap();
    assert!(perms.read.check(Path::new("/foo"), None).is_err());

    perms.write.check_partial(Path::new("/foo"), None).unwrap();
    assert!(perms.write.check(Path::new("/foo"), None).is_err());
  }

  #[test]
  fn test_net_fully_qualified_domain_name() {
    let mut perms = Permissions {
      net: Permissions::new_unary(
        &Some(vec!["allowed.domain".to_string(), "1.1.1.1".to_string()]),
        &Some(vec!["denied.domain".to_string(), "2.2.2.2".to_string()]),
        false,
      )
      .unwrap(),
      ..Permissions::none_without_prompt()
    };

    perms
      .net
      .check(
        &NetDescriptor("allowed.domain.".parse().unwrap(), None),
        None,
      )
      .unwrap();
    perms
      .net
      .check(&NetDescriptor("1.1.1.1".parse().unwrap(), None), None)
      .unwrap();
    assert!(perms
      .net
      .check(
        &NetDescriptor("denied.domain.".parse().unwrap(), None),
        None
      )
      .is_err());
    assert!(perms
      .net
      .check(&NetDescriptor("2.2.2.2".parse().unwrap(), None), None)
      .is_err());
  }

  #[test]
  fn test_deserialize_child_permissions_arg() {
    set_prompter(Box::new(TestPrompter));
    assert_eq!(
      ChildPermissionsArg::inherit(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::Inherit,
        hrtime: ChildUnitPermissionArg::Inherit,
        net: ChildUnaryPermissionArg::Inherit,
        ffi: ChildUnaryPermissionArg::Inherit,
        read: ChildUnaryPermissionArg::Inherit,
        run: ChildUnaryPermissionArg::Inherit,
        sys: ChildUnaryPermissionArg::Inherit,
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
        sys: ChildUnaryPermissionArg::NotGranted,
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
        "sys": true,
        "write": true,
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::Granted,
        net: ChildUnaryPermissionArg::Granted,
        ffi: ChildUnaryPermissionArg::Granted,
        read: ChildUnaryPermissionArg::Granted,
        run: ChildUnaryPermissionArg::Granted,
        sys: ChildUnaryPermissionArg::Granted,
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
        "sys": false,
        "write": false,
      }))
      .unwrap(),
      ChildPermissionsArg {
        env: ChildUnaryPermissionArg::NotGranted,
        net: ChildUnaryPermissionArg::NotGranted,
        ffi: ChildUnaryPermissionArg::NotGranted,
        read: ChildUnaryPermissionArg::NotGranted,
        run: ChildUnaryPermissionArg::NotGranted,
        sys: ChildUnaryPermissionArg::NotGranted,
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
        "sys": ["hostname", "osRelease"],
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
        sys: ChildUnaryPermissionArg::GrantedList(svec![
          "hostname",
          "osRelease"
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
    set_prompter(Box::new(TestPrompter));
    let mut main_perms = Permissions {
      env: Permissions::new_unary(&Some(vec![]), &None, false).unwrap(),
      hrtime: Permissions::new_hrtime(true, false),
      net: Permissions::new_unary(&Some(svec!["foo", "bar"]), &None, false)
        .unwrap(),
      ..Permissions::none_without_prompt()
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
        env: Permissions::new_unary(&Some(vec![]), &None, false).unwrap(),
        net: Permissions::new_unary(&Some(svec!["foo"]), &None, false).unwrap(),
        ..Permissions::none_without_prompt()
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
    set_prompter(Box::new(TestPrompter));
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
    assert_eq!(
      main_perms.run.granted_list,
      HashSet::from([
        RunDescriptor::Name("bar".to_owned()),
        RunDescriptor::Name("foo".to_owned())
      ])
    );
  }

  #[test]
  fn test_create_child_permissions_with_inherited_denied_list() {
    set_prompter(Box::new(TestPrompter));
    let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
    let mut main_perms = Permissions::from_options(&PermissionsOptions {
      prompt: true,
      ..Default::default()
    })
    .unwrap();
    prompt_value.set(false);
    assert!(main_perms.write.check(&PathBuf::from("foo"), None).is_err());
    let worker_perms = create_child_permissions(
      &mut main_perms.clone(),
      ChildPermissionsArg::none(),
    )
    .unwrap();
    assert_eq!(
      worker_perms.write.flag_denied_list,
      main_perms.write.flag_denied_list
    );
  }

  #[test]
  fn test_handle_empty_value() {
    set_prompter(Box::new(TestPrompter));

    assert!(Permissions::new_unary::<ReadDescriptor>(
      &Some(vec![Default::default()]),
      &None,
      false
    )
    .is_err());
    assert!(Permissions::new_unary::<EnvDescriptor>(
      &Some(vec![Default::default()]),
      &None,
      false
    )
    .is_err());
    assert!(Permissions::new_unary::<NetDescriptor>(
      &Some(vec![Default::default()]),
      &None,
      false
    )
    .is_err());
  }

  #[test]
  fn test_host_parse() {
    let hosts = &[
      ("deno.land", Some(Host::Fqdn(fqdn!("deno.land")))),
      ("DENO.land", Some(Host::Fqdn(fqdn!("deno.land")))),
      ("deno.land.", Some(Host::Fqdn(fqdn!("deno.land")))),
      (
        "1.1.1.1",
        Some(Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)))),
      ),
      (
        "::1",
        Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))),
      ),
      (
        "[::1]",
        Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))),
      ),
      ("[::1", None),
      ("::1]", None),
      ("deno. land", None),
      ("1. 1.1.1", None),
      ("1.1.1.1.", None),
      ("1::1.", None),
      ("deno.land.", Some(Host::Fqdn(fqdn!("deno.land")))),
      (".deno.land", None),
      (
        "::ffff:1.1.1.1",
        Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(
          0, 0, 0, 0, 0, 0xffff, 0x0101, 0x0101,
        )))),
      ),
    ];

    for (host_str, expected) in hosts {
      assert_eq!(host_str.parse::<Host>().ok(), *expected, "{host_str}");
    }
  }

  #[test]
  fn test_net_descriptor_parse() {
    let cases = &[
      (
        "deno.land",
        Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), None)),
      ),
      (
        "DENO.land",
        Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), None)),
      ),
      (
        "deno.land:8000",
        Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), Some(8000))),
      ),
      ("deno.land:", None),
      ("deno.land:a", None),
      ("deno. land:a", None),
      ("deno.land.: a", None),
      (
        "1.1.1.1",
        Some(NetDescriptor(
          Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
          None,
        )),
      ),
      ("1.1.1.1.", None),
      ("1.1.1.1..", None),
      (
        "1.1.1.1:8000",
        Some(NetDescriptor(
          Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
          Some(8000),
        )),
      ),
      ("::", None),
      (":::80", None),
      ("::80", None),
      (
        "[::]",
        Some(NetDescriptor(
          Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0))),
          None,
        )),
      ),
      ("[::1", None),
      ("::1]", None),
      ("::1]", None),
      ("[::1]:", None),
      ("[::1]:a", None),
      (
        "[::1]:443",
        Some(NetDescriptor(
          Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))),
          Some(443),
        )),
      ),
      ("", None),
      ("deno.land..", None),
    ];

    for (input, expected) in cases {
      assert_eq!(input.parse::<NetDescriptor>().ok(), *expected, "'{input}'");
    }
  }
}
