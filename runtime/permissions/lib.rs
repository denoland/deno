// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
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
use deno_core::unsync::sync::AtomicFlag;
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
use std::string::ToString;
use std::sync::Arc;

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

#[inline]
fn resolve_from_known_cwd(path: &Path, cwd: &Path) -> PathBuf {
  if path.is_absolute() {
    normalize_path(path)
  } else {
    normalize_path(cwd.join(path))
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
    let msg = if is_standalone() {
      format!(
        "Requires {}, specify the required permissions during compilation using `deno compile --allow-{}`",
        Self::fmt_access(name, info),
        name
      )
    } else {
      format!(
        "Requires {}, run again with the --allow-{} flag",
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

pub trait QueryDescriptor {
  type AllowDesc: AllowDescriptor<Query = Self>;
  type DenyDesc: DenyDescriptor<Query = Self>;

  fn flag_name() -> &'static str;
  fn display_name(&self) -> Cow<str>;

  fn as_allow(&self) -> Self::AllowDesc;
  fn as_deny(&self) -> Self::DenyDesc;

  /// Generic check function to check this descriptor against a `UnaryPermission`.
  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError>;

  /// Gets if this query descriptor should revoke the provided allow descriptor.
  fn revokes(&self, other: &Self::AllowDesc) -> bool;
}

pub trait AllowDescriptor: Eq + Clone + Hash {
  type Query: QueryDescriptor;

  fn as_query(&self) -> Self::Query;
  fn matches(&self, query: &Self::Query) -> bool;
}

pub trait DenyDescriptor: Eq + Clone + Hash {
  type Query: QueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool;
}

#[derive(Debug, Eq, PartialEq)]
pub struct UnaryPermission<TQuery: QueryDescriptor + ?Sized> {
  granted_global: bool,
  granted_list: HashSet<TQuery::AllowDesc>,
  flag_denied_global: bool,
  flag_denied_list: HashSet<TQuery::DenyDesc>,
  prompt_denied_global: bool,
  prompt_denied_list: HashSet<TQuery::DenyDesc>,
  prompt: bool,
}

impl<TQuery: QueryDescriptor> Default for UnaryPermission<TQuery> {
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

impl<TQuery: QueryDescriptor> Clone for UnaryPermission<TQuery> {
  fn clone(&self) -> Self {
    Self {
      granted_global: self.granted_global,
      granted_list: self.granted_list.clone(),
      flag_denied_global: self.flag_denied_global,
      flag_denied_list: self.flag_denied_list.clone(),
      prompt_denied_global: self.prompt_denied_global,
      prompt_denied_list: self.prompt_denied_list.clone(),
      prompt: self.prompt,
    }
  }
}

impl<TQuery: QueryDescriptor> UnaryPermission<TQuery> {
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
    self.check_desc(None, false, api_name)
  }

  fn check_desc(
    &mut self,
    desc: Option<&TQuery>,
    assert_non_partial: bool,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    let (result, prompted, is_allow_all) = self
      .query_desc(desc, AllowPartial::from(!assert_non_partial))
      .check2(
        TQuery::flag_name(),
        api_name,
        || desc.map(|d| format!("\"{}\"", d.display_name())),
        self.prompt,
      );
    if prompted {
      if result.is_ok() {
        if is_allow_all {
          self.insert_granted(None);
        } else {
          self.insert_granted(desc.map(|d| d.as_allow()));
        }
      } else {
        self.insert_prompt_denied(desc.map(|d| d.as_deny()));
      }
    }
    result
  }

  fn query_desc(
    &self,
    desc: Option<&TQuery>,
    allow_partial: AllowPartial,
  ) -> PermissionState {
    if self.is_flag_denied(desc) || self.is_prompt_denied(desc) {
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
    }
  }

  fn request_desc(&mut self, desc: Option<&TQuery>) -> PermissionState {
    let state = self.query_desc(desc, AllowPartial::TreatAsPartialGranted);
    if state == PermissionState::Granted {
      self.insert_granted(desc.map(|d| d.as_allow()));
      return state;
    }
    if state != PermissionState::Prompt {
      return state;
    }
    let mut message = String::with_capacity(40);
    message.push_str(&format!("{} access", TQuery::flag_name()));
    if let Some(desc) = desc {
      message.push_str(&format!(" to \"{}\"", desc.display_name()));
    }
    match permission_prompt(
      &message,
      TQuery::flag_name(),
      Some("Deno.permissions.request()"),
      true,
    ) {
      PromptResponse::Allow => {
        self.insert_granted(desc.map(|d| d.as_allow()));
        PermissionState::Granted
      }
      PromptResponse::Deny => {
        self.insert_prompt_denied(desc.map(|d| d.as_deny()));
        PermissionState::Denied
      }
      PromptResponse::AllowAll => {
        self.insert_granted(None);
        PermissionState::Granted
      }
    }
  }

  fn revoke_desc(&mut self, desc: Option<&TQuery>) -> PermissionState {
    match desc {
      Some(desc) => {
        self.granted_list.retain(|v| desc.revokes(v));
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

  fn is_granted(&self, desc: Option<&TQuery>) -> bool {
    match desc {
      Some(desc) => {
        self.granted_global || self.granted_list.iter().any(|v| v.matches(desc))
      }
      None => self.granted_global,
    }
  }

  fn is_flag_denied(&self, desc: Option<&TQuery>) -> bool {
    match desc {
      Some(desc) => {
        self.flag_denied_global
          || self.flag_denied_list.iter().any(|v| v.matches(desc))
      }
      None => self.flag_denied_global,
    }
  }

  fn is_prompt_denied(&self, desc: Option<&TQuery>) -> bool {
    match desc {
      Some(desc) => self.prompt_denied_list.iter().any(|v| v.matches(desc)),
      None => self.prompt_denied_global || !self.prompt_denied_list.is_empty(),
    }
  }

  fn is_partial_flag_denied(&self, desc: Option<&TQuery>) -> bool {
    match desc {
      None => !self.flag_denied_list.is_empty(),
      Some(desc) => self.flag_denied_list.iter().any(|v| v.matches(desc)),
    }
  }

  fn insert_granted(&mut self, desc: Option<TQuery::AllowDesc>) {
    Self::list_insert(desc, &mut self.granted_global, &mut self.granted_list);
  }

  fn insert_prompt_denied(&mut self, desc: Option<TQuery::DenyDesc>) {
    Self::list_insert(
      desc,
      &mut self.prompt_denied_global,
      &mut self.prompt_denied_list,
    );
  }

  fn list_insert<T: Hash + Eq>(
    desc: Option<T>,
    list_global: &mut bool,
    list: &mut HashSet<T>,
  ) {
    match desc {
      Some(desc) => {
        list.insert(desc);
      }
      None => *list_global = true,
    }
  }

  fn create_child_permissions(
    &mut self,
    flag: ChildUnaryPermissionArg,
    parse: impl Fn(&str) -> Result<TQuery::AllowDesc, AnyError>,
  ) -> Result<UnaryPermission<TQuery>, AnyError> {
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
        perms.granted_list = granted_list
          .iter()
          .map(|i| parse(i))
          .collect::<Result<_, _>>()?;
        if !perms
          .granted_list
          .iter()
          .all(|desc| desc.as_query().check_in_permission(self, None).is_ok())
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
pub struct PathQueryDescriptor {
  pub requested: String,
  pub resolved: PathBuf,
}

impl PathQueryDescriptor {
  pub fn into_ffi(self) -> FfiQueryDescriptor {
    FfiQueryDescriptor(self)
  }

  pub fn into_read(self) -> ReadQueryDescriptor {
    ReadQueryDescriptor(self)
  }

  pub fn into_write(self) -> WriteQueryDescriptor {
    WriteQueryDescriptor(self)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ReadQueryDescriptor(pub PathQueryDescriptor);

impl QueryDescriptor for ReadQueryDescriptor {
  type AllowDesc = ReadDescriptor;
  type DenyDesc = ReadDescriptor;

  fn flag_name() -> &'static str {
    "read"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::Borrowed(self.0.requested.as_str())
  }

  fn as_allow(&self) -> Self::AllowDesc {
    ReadDescriptor(self.0.resolved.clone())
  }

  fn as_deny(&self) -> Self::DenyDesc {
    ReadDescriptor(self.0.resolved.clone())
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    other.0.starts_with(&self.0.resolved)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ReadDescriptor(pub PathBuf);

impl AllowDescriptor for ReadDescriptor {
  type Query = ReadQueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }

  fn as_query(&self) -> Self::Query {
    PathQueryDescriptor {
      requested: self.0.to_string_lossy().into_owned(),
      resolved: self.0.clone(),
    }
    .into_read()
  }
}

impl DenyDescriptor for ReadDescriptor {
  type Query = ReadQueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WriteQueryDescriptor(pub PathQueryDescriptor);

impl QueryDescriptor for WriteQueryDescriptor {
  type AllowDesc = WriteDescriptor;
  type DenyDesc = WriteDescriptor;

  fn flag_name() -> &'static str {
    "write"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::Borrowed(&self.0.requested)
  }

  fn as_allow(&self) -> Self::AllowDesc {
    WriteDescriptor(self.0.resolved.clone())
  }

  fn as_deny(&self) -> Self::DenyDesc {
    WriteDescriptor(self.0.resolved.clone())
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    other.0.starts_with(&self.0.resolved)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WriteDescriptor(pub PathBuf);

impl AllowDescriptor for WriteDescriptor {
  type Query = WriteQueryDescriptor;

  fn as_query(&self) -> Self::Query {
    WriteQueryDescriptor(PathQueryDescriptor {
      requested: self.0.to_string_lossy().into_owned(),
      resolved: self.0.clone(),
    })
  }

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }
}

impl DenyDescriptor for WriteDescriptor {
  type Query = WriteQueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Host {
  Fqdn(FQDN),
  Ip(IpAddr),
}

impl Host {
  // TODO(bartlomieju): rewrite to not use `AnyError` but a specific error implementations
  fn parse(s: &str) -> Result<Self, AnyError> {
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
      let fqdn = {
        use std::str::FromStr;
        FQDN::from_str(&lower)
          .with_context(|| format!("invalid host: '{s}'"))?
      };
      if fqdn.is_root() {
        return Err(uri_error(format!("invalid empty host: '{s}'")));
      }
      Ok(Host::Fqdn(fqdn))
    }
  }

  #[cfg(test)]
  #[track_caller]
  fn must_parse(s: &str) -> Self {
    Self::parse(s).unwrap()
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NetDescriptor(pub Host, pub Option<u16>);

impl QueryDescriptor for NetDescriptor {
  type AllowDesc = NetDescriptor;
  type DenyDesc = NetDescriptor;

  fn flag_name() -> &'static str {
    "net"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::from(format!("{}", self))
  }

  fn as_allow(&self) -> Self::AllowDesc {
    self.clone()
  }

  fn as_deny(&self) -> Self::DenyDesc {
    self.clone()
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    self.0 == other.0 && (other.1.is_none() || self.1 == other.1)
  }
}

impl AllowDescriptor for NetDescriptor {
  type Query = NetDescriptor;

  fn as_query(&self) -> Self::Query {
    self.clone()
  }

  fn matches(&self, query: &Self::Query) -> bool {
    self.0 == query.0 && (self.1.is_none() || self.1 == query.1)
  }
}

impl DenyDescriptor for NetDescriptor {
  type Query = NetDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self.0 == query.0 && (self.1.is_none() || self.1 == query.1)
  }
}

// TODO(bartlomieju): rewrite to not use `AnyError` but a specific error implementations
impl NetDescriptor {
  pub fn parse(hostname: &str) -> Result<Self, AnyError> {
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
    let host = Host::parse(host)?;

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

impl QueryDescriptor for EnvDescriptor {
  type AllowDesc = EnvDescriptor;
  type DenyDesc = EnvDescriptor;

  fn flag_name() -> &'static str {
    "env"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::from(self.0.as_ref())
  }

  fn as_allow(&self) -> Self::AllowDesc {
    self.clone()
  }

  fn as_deny(&self) -> Self::DenyDesc {
    self.clone()
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    self == other
  }
}

impl AllowDescriptor for EnvDescriptor {
  type Query = EnvDescriptor;

  fn as_query(&self) -> Self::Query {
    self.clone()
  }

  fn matches(&self, query: &Self::Query) -> bool {
    self == query
  }
}

impl DenyDescriptor for EnvDescriptor {
  type Query = EnvDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self == query
  }
}

impl AsRef<str> for EnvDescriptor {
  fn as_ref(&self) -> &str {
    self.0.as_ref()
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct RunQueryDescriptor {
  pub requested: String,
  pub resolved: PathBuf,
}

impl QueryDescriptor for RunQueryDescriptor {
  type AllowDesc = AllowRunDescriptor;
  type DenyDesc = DenyRunDescriptor;

  fn flag_name() -> &'static str {
    "run"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::Borrowed(&self.requested)
  }

  fn as_allow(&self) -> Self::AllowDesc {
    AllowRunDescriptor(self.resolved.clone())
  }

  fn as_deny(&self) -> Self::DenyDesc {
    if self.requested.contains('/')
      || cfg!(windows) && self.requested.contains("\\")
    {
      DenyRunDescriptor::Path(self.resolved.clone())
    } else {
      DenyRunDescriptor::Name(self.requested.clone())
    }
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    if self.resolved == other.0 {
      return true;
    }
    if is_path(&self.requested) {
      false
    } else {
      denies_run_name(&self.requested, &other.0)
    }
  }
}

pub enum RunDescriptorArg {
  Name(String),
  Path(PathBuf),
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct AllowRunDescriptor(pub PathBuf);

impl AllowRunDescriptor {
  pub fn parse(text: &str, cwd: &Path) -> Result<Self, AnyError> {
    let text = if cfg!(windows) {
      Cow::Owned(text.to_string())
    } else {
      Cow::Borrowed(text)
    };
    let is_path = is_path(&text);
    // todo(dsherret): canonicalize in #25458
    let path = if is_path {
      resolve_from_known_cwd(Path::new(text.as_ref()), cwd)
    } else {
      which::which(text.as_ref())?
    };
    Ok(AllowRunDescriptor(path))
  }
}

impl AllowDescriptor for AllowRunDescriptor {
  type Query = RunQueryDescriptor;

  fn as_query(&self) -> Self::Query {
    RunQueryDescriptor {
      requested: self.0.to_string_lossy().into_owned(),
      resolved: self.0.clone(),
    }
  }

  fn matches(&self, query: &Self::Query) -> bool {
    query.resolved == self.0
  }
}

// todo(THIS PR): get rid of warning
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum DenyRunDescriptor {
  /// Warning: You may want to construct with `RunDescriptor::from()` for case
  /// handling.
  Name(String),
  /// Warning: You may want to construct with `RunDescriptor::from()` for case
  /// handling.
  Path(PathBuf),
}

impl DenyRunDescriptor {
  pub fn parse(text: &str, cwd: &Path) -> Self {
    if text.contains('/') || cfg!(windows) && text.contains('\\') {
      DenyRunDescriptor::Name(text.to_string())
    } else {
      let path = resolve_from_known_cwd(Path::new(&text), cwd);
      DenyRunDescriptor::Path(path)
    }
  }
}

impl DenyDescriptor for DenyRunDescriptor {
  type Query = RunQueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    match self {
      DenyRunDescriptor::Name(name) => denies_run_name(name, &query.resolved),
      DenyRunDescriptor::Path(path) => query.resolved.starts_with(path),
    }
  }
}

fn is_path(text: &str) -> bool {
  if cfg!(windows) {
    text.contains('/') || text.contains('\\') || Path::new(text).is_absolute()
  } else {
    text.contains('/')
  }
}

fn denies_run_name(name: &str, cmd_path: &Path) -> bool {
  let Some(file_stem) = cmd_path.file_stem() else {
    return false;
  };
  let Some(file_stem) = file_stem.to_str() else {
    return false;
  };
  let Some(remaining) = file_stem.strip_prefix(name) else {
    return false;
  };
  // be broad and consider anything like `deno.something` as matching deny perms
  remaining.is_empty() || remaining.starts_with('.')
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SysDescriptor(pub String);

impl QueryDescriptor for SysDescriptor {
  type AllowDesc = SysDescriptor;
  type DenyDesc = SysDescriptor;

  fn flag_name() -> &'static str {
    "sys"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::from(self.0.to_string())
  }

  fn as_allow(&self) -> Self::AllowDesc {
    self.clone()
  }

  fn as_deny(&self) -> Self::DenyDesc {
    self.clone()
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), false, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    self == other
  }
}

impl AllowDescriptor for SysDescriptor {
  type Query = SysDescriptor;

  fn as_query(&self) -> Self::Query {
    self.clone()
  }

  fn matches(&self, query: &Self::Query) -> bool {
    self == query
  }
}

impl DenyDescriptor for SysDescriptor {
  type Query = SysDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self == query
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
pub struct FfiQueryDescriptor(pub PathQueryDescriptor);

impl QueryDescriptor for FfiQueryDescriptor {
  type AllowDesc = FfiDescriptor;
  type DenyDesc = FfiDescriptor;

  fn flag_name() -> &'static str {
    "ffi"
  }

  fn display_name(&self) -> Cow<str> {
    Cow::Borrowed(&self.0.requested)
  }

  fn as_allow(&self) -> Self::AllowDesc {
    FfiDescriptor(self.0.resolved.clone())
  }

  fn as_deny(&self) -> Self::DenyDesc {
    FfiDescriptor(self.0.resolved.clone())
  }

  fn check_in_permission(
    &self,
    perm: &mut UnaryPermission<Self>,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(perm);
    perm.check_desc(Some(self), true, api_name)
  }

  fn revokes(&self, other: &Self::AllowDesc) -> bool {
    other.0.starts_with(&self.0.resolved)
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FfiDescriptor(pub PathBuf);

impl AllowDescriptor for FfiDescriptor {
  type Query = FfiQueryDescriptor;

  fn as_query(&self) -> Self::Query {
    PathQueryDescriptor {
      requested: self.0.to_string_lossy().into_owned(),
      resolved: self.0.clone(),
    }
    .into_ffi()
  }

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }
}

impl DenyDescriptor for FfiDescriptor {
  type Query = FfiQueryDescriptor;

  fn matches(&self, query: &Self::Query) -> bool {
    self.0.starts_with(&query.0.resolved)
  }
}

impl UnaryPermission<ReadQueryDescriptor> {
  pub fn query(&self, desc: Option<&ReadQueryDescriptor>) -> PermissionState {
    self.query_desc(desc, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(
    &mut self,
    path: Option<&ReadQueryDescriptor>,
  ) -> PermissionState {
    self.request_desc(path)
  }

  pub fn revoke(
    &mut self,
    desc: Option<&ReadQueryDescriptor>,
  ) -> PermissionState {
    self.revoke_desc(desc)
  }

  pub fn check(
    &mut self,
    desc: &ReadQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(desc), true, api_name)
  }

  #[inline]
  pub fn check_partial(
    &mut self,
    desc: &ReadQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(&desc), false, api_name)
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name)
  }
}

impl UnaryPermission<WriteQueryDescriptor> {
  pub fn query(&self, path: Option<&WriteQueryDescriptor>) -> PermissionState {
    self.query_desc(path, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(
    &mut self,
    path: Option<&WriteQueryDescriptor>,
  ) -> PermissionState {
    self.request_desc(path)
  }

  pub fn revoke(
    &mut self,
    path: Option<&WriteQueryDescriptor>,
  ) -> PermissionState {
    self.revoke_desc(path)
  }

  pub fn check(
    &mut self,
    path: &WriteQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(path), true, api_name)
  }

  #[inline]
  pub fn check_partial(
    &mut self,
    path: &WriteQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(path), false, api_name)
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, api_name)
  }
}

impl UnaryPermission<NetDescriptor> {
  pub fn query(&self, host: Option<&NetDescriptor>) -> PermissionState {
    self.query_desc(host, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(&mut self, host: Option<&NetDescriptor>) -> PermissionState {
    self.request_desc(host)
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
    self.check_desc(Some(host), false, api_name)
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
    let host = Host::parse(host)?;
    let port = url.port_or_known_default();
    let descriptor = NetDescriptor(host, port);
    self.check_desc(Some(&descriptor), false, api_name)
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None)
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
    self.request_desc(env.map(EnvDescriptor::new).as_ref())
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
    self.check_desc(Some(&EnvDescriptor::new(env)), false, api_name)
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None)
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
    self.request_desc(kind.map(|k| SysDescriptor(k.to_string())).as_ref())
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
    self.check_desc(Some(&SysDescriptor(kind.to_string())), false, api_name)
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, None)
  }
}

impl UnaryPermission<RunQueryDescriptor> {
  pub fn query(&self, cmd: Option<&RunQueryDescriptor>) -> PermissionState {
    self.query_desc(cmd, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(
    &mut self,
    cmd: Option<&RunQueryDescriptor>,
  ) -> PermissionState {
    self.request_desc(cmd)
  }

  pub fn revoke(
    &mut self,
    cmd: Option<&RunQueryDescriptor>,
  ) -> PermissionState {
    self.revoke_desc(cmd)
  }

  pub fn check(
    &mut self,
    cmd: &RunQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    debug_assert!(cmd.resolved.is_absolute());
    self.check_desc(Some(cmd), false, api_name)
  }

  pub fn check_all(&mut self, api_name: Option<&str>) -> Result<(), AnyError> {
    self.check_desc(None, false, api_name)
  }

  /// Queries without prompting
  pub fn query_all(&mut self, api_name: Option<&str>) -> bool {
    if self.is_allow_all() {
      return true;
    }
    let (result, _prompted, _is_allow_all) =
      self.query_desc(None, AllowPartial::TreatAsDenied).check2(
        RunQueryDescriptor::flag_name(),
        api_name,
        || None,
        /* prompt */ false,
      );
    result.is_ok()
  }
}

impl UnaryPermission<FfiQueryDescriptor> {
  pub fn query(&self, path: Option<&FfiQueryDescriptor>) -> PermissionState {
    self.query_desc(path, AllowPartial::TreatAsPartialGranted)
  }

  pub fn request(
    &mut self,
    path: Option<&FfiQueryDescriptor>,
  ) -> PermissionState {
    self.request_desc(path)
  }

  pub fn revoke(
    &mut self,
    path: Option<&FfiQueryDescriptor>,
  ) -> PermissionState {
    self.revoke_desc(path)
  }

  pub fn check(
    &mut self,
    path: &FfiQueryDescriptor,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(Some(path), true, api_name)
  }

  pub fn check_partial(
    &mut self,
    path: Option<&FfiQueryDescriptor>,
  ) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(path, false, None)
  }

  pub fn check_all(&mut self) -> Result<(), AnyError> {
    skip_check_if_is_permission_fully_granted!(self);
    self.check_desc(None, false, Some("all"))
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Permissions {
  pub read: UnaryPermission<ReadQueryDescriptor>,
  pub write: UnaryPermission<WriteQueryDescriptor>,
  pub net: UnaryPermission<NetDescriptor>,
  pub env: UnaryPermission<EnvDescriptor>,
  pub sys: UnaryPermission<SysDescriptor>,
  pub run: UnaryPermission<RunQueryDescriptor>,
  pub ffi: UnaryPermission<FfiQueryDescriptor>,
  pub all: UnitPermission,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionsOptions {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub allow_net: Option<Vec<String>>,
  pub deny_net: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<String>>,
  pub deny_ffi: Option<Vec<String>>,
  pub allow_read: Option<Vec<String>>,
  pub deny_read: Option<Vec<String>>,
  pub allow_run: Option<Vec<String>>,
  pub deny_run: Option<Vec<String>>,
  pub allow_sys: Option<Vec<String>>,
  pub deny_sys: Option<Vec<String>>,
  pub allow_write: Option<Vec<String>>,
  pub deny_write: Option<Vec<String>>,
  pub prompt: bool,
}

impl Permissions {
  pub fn new_unary<TQuery>(
    allow_list: Option<HashSet<TQuery::AllowDesc>>,
    deny_list: Option<HashSet<TQuery::DenyDesc>>,
    prompt: bool,
  ) -> Result<UnaryPermission<TQuery>, AnyError>
  where
    TQuery: QueryDescriptor,
  {
    Ok(UnaryPermission::<TQuery> {
      granted_global: global_from_option(allow_list.as_ref()),
      granted_list: allow_list.unwrap_or_default(),
      flag_denied_global: global_from_option(deny_list.as_ref()),
      flag_denied_list: deny_list.unwrap_or_default(),
      prompt,
      ..Default::default()
    })
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

  pub fn from_options(
    parser: &dyn PermissionDescriptorParser,
    opts: &PermissionsOptions,
  ) -> Result<Self, AnyError> {
    fn resolve_allow_run(
      allow_run: &[String],
    ) -> Result<HashSet<AllowRunDescriptor>, AnyError> {
      let mut new_allow_run = HashSet::with_capacity(allow_run.len());
      for command_name in allow_run {
        if command_name.is_empty() {
          bail!("Empty command name not allowed in --allow-run=...")
        }
        let command_path_result = which::which(command_name);
        match command_path_result {
          Ok(command_path) => {
            new_allow_run.insert(AllowRunDescriptor(command_path));
          }
          Err(err) => {
            log::info!(
              "{} Failed to resolve '{}' for allow-run: {}",
              colors::gray("Info"),
              command_name,
              err
            );
          }
        }
      }
      Ok(new_allow_run)
    }

    fn parse_maybe_vec<T: Eq + PartialEq + Hash>(
      items: Option<&[String]>,
      parse: impl Fn(&str) -> Result<T, AnyError>,
    ) -> Result<Option<HashSet<T>>, AnyError> {
      match items {
        Some(items) => Ok(Some(
          items
            .iter()
            .map(|item| parse(item))
            .collect::<Result<HashSet<_>, _>>()?,
        )),
        None => Ok(None),
      }
    }

    let mut deny_write = parse_maybe_vec(opts.deny_write.as_deref(), |item| {
      parser.parse_write_descriptor(item)
    })?;
    let allow_run = opts
      .allow_run
      .as_ref()
      .and_then(|raw_allow_run| match resolve_allow_run(raw_allow_run) {
        Ok(resolved_allow_run) => {
          if resolved_allow_run.is_empty() && !raw_allow_run.is_empty() {
            None // convert to no permissions if now empty
          } else {
            Some(Ok(resolved_allow_run))
          }
        }
        Err(err) => Some(Err(err)),
      })
      .transpose()?;
    // add the allow_run list to deno_write
    if let Some(allow_run_vec) = &allow_run {
      if !allow_run_vec.is_empty() {
        let deno_write = deny_write.get_or_insert_with(Default::default);
        deno_write.extend(
          allow_run_vec
            .iter()
            .map(|item| WriteDescriptor(item.0.clone())),
        );
      }
    }

    Ok(Self {
      read: Permissions::new_unary(
        parse_maybe_vec(opts.allow_read.as_deref(), |item| {
          parser.parse_read_descriptor(item)
        })?,
        parse_maybe_vec(opts.deny_read.as_deref(), |item| {
          parser.parse_read_descriptor(item)
        })?,
        opts.prompt,
      )?,
      write: Permissions::new_unary(
        parse_maybe_vec(opts.allow_write.as_deref(), |item| {
          parser.parse_write_descriptor(item)
        })?,
        deny_write,
        opts.prompt,
      )?,
      net: Permissions::new_unary(
        parse_maybe_vec(opts.allow_net.as_deref(), |item| {
          parser.parse_net_descriptor(item)
        })?,
        parse_maybe_vec(opts.deny_net.as_deref(), |item| {
          parser.parse_net_descriptor(item)
        })?,
        opts.prompt,
      )?,
      env: Permissions::new_unary(
        parse_maybe_vec(opts.allow_env.as_deref(), |item| {
          parser.parse_env_descriptor(item)
        })?,
        parse_maybe_vec(opts.deny_env.as_deref(), |text| {
          parser.parse_env_descriptor(text)
        })?,
        opts.prompt,
      )?,
      sys: Permissions::new_unary(
        parse_maybe_vec(opts.allow_sys.as_deref(), |text| {
          parser.parse_sys_descriptor(text)
        })?,
        parse_maybe_vec(opts.deny_sys.as_deref(), |text| {
          parser.parse_sys_descriptor(text)
        })?,
        opts.prompt,
      )?,
      run: Permissions::new_unary(
        allow_run,
        parse_maybe_vec(opts.deny_run.as_deref(), |text| {
          parser.parse_deny_run_descriptor(text)
        })?,
        opts.prompt,
      )?,
      ffi: Permissions::new_unary(
        parse_maybe_vec(opts.allow_ffi.as_deref(), |text| {
          parser.parse_ffi_descriptor(text)
        })?,
        parse_maybe_vec(opts.deny_ffi.as_deref(), |text| {
          parser.parse_ffi_descriptor(text)
        })?,
        opts.prompt,
      )?,
      all: Permissions::new_all(opts.allow_all),
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
      read: Permissions::new_unary(None, None, prompt).unwrap(),
      write: Permissions::new_unary(None, None, prompt).unwrap(),
      net: Permissions::new_unary(None, None, prompt).unwrap(),
      env: Permissions::new_unary(None, None, prompt).unwrap(),
      sys: Permissions::new_unary(None, None, prompt).unwrap(),
      run: Permissions::new_unary(None, None, prompt).unwrap(),
      ffi: Permissions::new_unary(None, None, prompt).unwrap(),
      all: Permissions::new_all(false),
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
        Ok(path) => self.read.check(
          &PathQueryDescriptor {
            requested: path.to_string_lossy().into_owned(),
            resolved: path,
          }
          .into_read(),
          Some("import()"),
        ),
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
pub struct PermissionsContainer {
  descriptor_parser: Arc<dyn PermissionDescriptorParser>,
  pub inner: Arc<Mutex<Permissions>>,
}

impl PermissionsContainer {
  pub fn new(
    descriptor_parser: Arc<dyn PermissionDescriptorParser>,
    perms: Permissions,
  ) -> Self {
    Self {
      descriptor_parser,
      inner: Arc::new(Mutex::new(perms)),
    }
  }

  pub fn allow_all(
    descriptor_parser: Arc<dyn PermissionDescriptorParser>,
  ) -> Self {
    Self::new(descriptor_parser, Permissions::allow_all())
  }

  #[inline(always)]
  pub fn check_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    self.inner.lock().check_specifier(specifier)
  }

  #[must_use]
  #[inline(always)]
  pub fn check_read(
    &self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, AnyError> {
    self.check_read_with_api_name(path, Some(api_name))
  }

  #[must_use]
  #[inline(always)]
  pub fn check_read_with_api_name(
    &self,
    path: &str,
    api_name: Option<&str>,
  ) -> Result<PathBuf, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.read;
    if inner.is_allow_all() {
      Ok(PathBuf::from(path))
    } else {
      let desc = self.descriptor_parser.parse_path_query(path)?.into_read();
      inner.check(&desc, api_name)?;
      Ok(desc.0.resolved)
    }
  }

  #[must_use]
  #[inline(always)]
  pub fn check_read_path<'a>(
    &self,
    path: &'a Path,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.read;
    if inner.is_allow_all() {
      Ok(Cow::Borrowed(path))
    } else {
      let desc = PathQueryDescriptor {
        requested: path.to_string_lossy().into_owned(),
        resolved: path.to_path_buf(),
      }
      .into_read();
      inner.check(&desc, Some(api_name))?;
      Ok(Cow::Owned(desc.0.resolved))
    }
  }

  /// As `check_read()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  #[inline(always)]
  pub fn check_read_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.read;
    skip_check_if_is_permission_fully_granted!(inner);
    inner.check(
      &PathQueryDescriptor {
        requested: format!("<{}>", display),
        resolved: path.to_path_buf(),
      }
      .into_read(),
      Some(api_name),
    )
  }

  #[inline(always)]
  pub fn check_read_all(&self, api_name: &str) -> Result<(), AnyError> {
    self.inner.lock().read.check_all(Some(api_name))
  }

  #[must_use]
  #[inline(always)]
  pub fn check_write(
    &self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, AnyError> {
    self.check_write_with_api_name(path, Some(api_name))
  }

  #[must_use]
  #[inline(always)]
  pub fn check_write_with_api_name(
    &self,
    path: &str,
    api_name: Option<&str>,
  ) -> Result<PathBuf, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.write;
    if inner.is_allow_all() {
      Ok(PathBuf::from(path))
    } else {
      let desc = self.descriptor_parser.parse_path_query(path)?.into_write();
      inner.check(&desc, api_name)?;
      Ok(desc.0.resolved)
    }
  }

  #[must_use]
  #[inline(always)]
  pub fn check_write_path<'a>(
    &self,
    path: &'a Path,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.write;
    if inner.is_allow_all() {
      Ok(Cow::Borrowed(path))
    } else {
      let desc = PathQueryDescriptor {
        requested: path.to_string_lossy().into_owned(),
        resolved: path.to_path_buf(),
      }
      .into_write();
      inner.check(&desc, Some(api_name))?;
      Ok(Cow::Owned(desc.0.resolved))
    }
  }

  #[inline(always)]
  pub fn check_write_all(&self, api_name: &str) -> Result<(), AnyError> {
    self.inner.lock().write.check_all(Some(api_name))
  }

  /// As `check_write()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  #[inline(always)]
  pub fn check_write_blind(
    &self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.write;
    skip_check_if_is_permission_fully_granted!(inner);
    inner.check(
      &PathQueryDescriptor {
        requested: format!("<{}>", display),
        resolved: path.to_path_buf(),
      }
      .into_write(),
      Some(api_name),
    )
  }

  #[inline(always)]
  pub fn check_write_partial(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.write;
    if inner.is_allow_all() {
      Ok(PathBuf::from(path))
    } else {
      let query = self.descriptor_parser.parse_path_query(path)?;
      inner.check_partial(&desc, Some(api_name))?;
      Ok(desc.0.resolved)
    }
  }

  #[inline(always)]
  pub fn check_run(
    &mut self,
    cmd: &RunQueryDescriptor,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.inner.lock().run.check(cmd, Some(api_name))
  }

  #[inline(always)]
  pub fn check_run_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.inner.lock().run.check_all(Some(api_name))
  }

  #[inline(always)]
  pub fn query_run_all(&mut self, api_name: &str) -> bool {
    self.inner.lock().run.query_all(Some(api_name))
  }

  #[inline(always)]
  pub fn check_sys(&self, kind: &str, api_name: &str) -> Result<(), AnyError> {
    self.inner.lock().sys.check(kind, Some(api_name))
  }

  #[inline(always)]
  pub fn check_env(&mut self, var: &str) -> Result<(), AnyError> {
    self.inner.lock().env.check(var, None)
  }

  #[inline(always)]
  pub fn check_env_all(&mut self) -> Result<(), AnyError> {
    self.inner.lock().env.check_all()
  }

  #[inline(always)]
  pub fn check_sys_all(&mut self) -> Result<(), AnyError> {
    self.inner.lock().sys.check_all()
  }

  #[inline(always)]
  pub fn check_ffi_all(&mut self) -> Result<(), AnyError> {
    self.inner.lock().ffi.check_all()
  }

  /// This checks to see if the allow-all flag was passed, not whether all
  /// permissions are enabled!
  #[inline(always)]
  pub fn check_was_allow_all_flag_passed(&mut self) -> Result<(), AnyError> {
    self.inner.lock().all.check()
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
    self.inner.lock().net.check_url(url, Some(api_name))
  }

  #[inline(always)]
  pub fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
    api_name: &str,
  ) -> Result<(), AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.net;
    skip_check_if_is_permission_fully_granted!(inner);
    let hostname = Host::parse(host.0.as_ref())?;
    let descriptor = NetDescriptor(hostname, host.1);
    inner.check(&descriptor, Some(api_name))
  }

  #[inline(always)]
  pub fn check_ffi(&mut self, path: &str) -> Result<PathBuf, AnyError> {
    let mut inner = self.inner.lock();
    let inner = &mut inner.ffi;
    if inner.is_allow_all() {
      Ok(PathBuf::from(path))
    } else {
      let desc = self.descriptor_parser.parse_path_query(path)?.into_ffi();
      inner.check(&desc, None)?;
      Ok(desc.0.resolved)
    }
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

fn global_from_option<T>(flag: Option<&HashSet<T>>) -> bool {
  matches!(flag, Some(v) if v.is_empty())
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

/// Parses and normalizes permissions.
pub trait PermissionDescriptorParser: Debug {
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, AnyError>;

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, AnyError>;

  fn parse_net_descriptor(&self, text: &str)
    -> Result<NetDescriptor, AnyError>;

  fn parse_env_descriptor(&self, text: &str)
    -> Result<EnvDescriptor, AnyError>;

  fn parse_sys_descriptor(&self, text: &str)
    -> Result<SysDescriptor, AnyError>;

  fn parse_allow_run_descriptor(
    &self,
    text: &str,
  ) -> Result<AllowRunDescriptor, AnyError>;

  fn parse_deny_run_descriptor(
    &self,
    text: &str,
  ) -> Result<DenyRunDescriptor, AnyError>;

  fn parse_ffi_descriptor(&self, text: &str)
    -> Result<FfiDescriptor, AnyError>;

  // queries

  fn parse_path_query(
    &self,
    path: &str,
  ) -> Result<PathQueryDescriptor, AnyError>;
}

pub fn create_child_permissions(
  main_perms: &mut Permissions,
  child_permissions_arg: ChildPermissionsArg,
  parser: &dyn PermissionDescriptorParser,
) -> Result<Permissions, AnyError> {
  fn is_granted_unary(arg: &ChildUnaryPermissionArg) -> bool {
    match arg {
      ChildUnaryPermissionArg::Inherit | ChildUnaryPermissionArg::Granted => {
        true
      }
      ChildUnaryPermissionArg::NotGranted
      | ChildUnaryPermissionArg::GrantedList(_) => false,
    }
  }

  let mut worker_perms = Permissions::none_without_prompt();

  worker_perms.all = main_perms
    .all
    .create_child_permissions(ChildUnitPermissionArg::Inherit)?;

  // downgrade the `worker_perms.all` based on the other values
  if worker_perms.all.query() == PermissionState::Granted {
    let unary_perms = [
      &child_permissions_arg.read,
      &child_permissions_arg.write,
      &child_permissions_arg.net,
      &child_permissions_arg.env,
      &child_permissions_arg.sys,
      &child_permissions_arg.run,
      &child_permissions_arg.ffi,
    ];
    let allow_all = unary_perms.into_iter().all(is_granted_unary);
    if !allow_all {
      worker_perms.all.revoke();
    }
  }

  // WARNING: When adding a permission here, ensure it is handled
  // in the worker_perms.all block above
  worker_perms.read = main_perms
    .read
    .create_child_permissions(child_permissions_arg.read, |text| {
      parser.parse_read_descriptor(text)
    })?;
  worker_perms.write = main_perms
    .write
    .create_child_permissions(child_permissions_arg.write, |text| {
      parser.parse_write_descriptor(text)
    })?;
  worker_perms.net = main_perms
    .net
    .create_child_permissions(child_permissions_arg.net, |text| {
      parser.parse_net_descriptor(text)
    })?;
  worker_perms.env = main_perms
    .env
    .create_child_permissions(child_permissions_arg.env, |text| {
      parser.parse_env_descriptor(text)
    })?;
  worker_perms.sys = main_perms
    .sys
    .create_child_permissions(child_permissions_arg.sys, |text| {
      parser.parse_sys_descriptor(text)
    })?;
  worker_perms.run = main_perms
    .run
    .create_child_permissions(child_permissions_arg.run, |text| {
      parser.parse_allow_run_descriptor(text)
    })?;
  worker_perms.ffi = main_perms
    .ffi
    .create_child_permissions(child_permissions_arg.ffi, |text| {
      parser.parse_ffi_descriptor(text)
    })?;

  Ok(worker_perms)
}

static IS_STANDALONE: AtomicFlag = AtomicFlag::lowered();

pub fn mark_standalone() {
  IS_STANDALONE.raise();
}

pub fn is_standalone() -> bool {
  IS_STANDALONE.is_raised()
}

// #[cfg(test)]
// mod tests {
//   use super::*;
//   use deno_core::serde_json::json;
//   use fqdn::fqdn;
//   use prompter::tests::*;
//   use std::net::Ipv4Addr;

//   // Creates vector of strings, Vec<String>
//   macro_rules! svec {
//       ($($x:expr),*) => (vec![$($x.to_string()),*]);
//   }
//   macro_rules! sarr {
//       ($($x:expr),*) => ([$($x.to_string()),*]);
//   }

//   #[test]
//   fn check_paths() {
//     set_prompter(Box::new(TestPrompter));
//     let allowlist = vec![
//       PathBuf::from("/a/specific/dir/name"),
//       PathBuf::from("/a/specific"),
//       PathBuf::from("/b/c"),
//     ];

//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_read: Some(allowlist.clone()),
//       allow_write: Some(allowlist.clone()),
//       allow_ffi: Some(allowlist),
//       ..Default::default()
//     })
//     .unwrap();

//     // Inside of /a/specific and /a/specific/dir/name
//     assert!(perms
//       .read
//       .check(Path::new("/a/specific/dir/name"), None)
//       .is_ok());
//     assert!(perms
//       .write
//       .check(Path::new("/a/specific/dir/name"), None)
//       .is_ok());
//     assert!(perms
//       .ffi
//       .check(Path::new("/a/specific/dir/name"), None)
//       .is_ok());

//     // Inside of /a/specific but outside of /a/specific/dir/name
//     assert!(perms.read.check(Path::new("/a/specific/dir"), None).is_ok());
//     assert!(perms
//       .write
//       .check(Path::new("/a/specific/dir"), None)
//       .is_ok());
//     assert!(perms.ffi.check(Path::new("/a/specific/dir"), None).is_ok());

//     // Inside of /a/specific and /a/specific/dir/name
//     assert!(perms
//       .read
//       .check(Path::new("/a/specific/dir/name/inner"), None)
//       .is_ok());
//     assert!(perms
//       .write
//       .check(Path::new("/a/specific/dir/name/inner"), None)
//       .is_ok());
//     assert!(perms
//       .ffi
//       .check(Path::new("/a/specific/dir/name/inner"), None)
//       .is_ok());

//     // Inside of /a/specific but outside of /a/specific/dir/name
//     assert!(perms
//       .read
//       .check(Path::new("/a/specific/other/dir"), None)
//       .is_ok());
//     assert!(perms
//       .write
//       .check(Path::new("/a/specific/other/dir"), None)
//       .is_ok());
//     assert!(perms
//       .ffi
//       .check(Path::new("/a/specific/other/dir"), None)
//       .is_ok());

//     // Exact match with /b/c
//     assert!(perms.read.check(Path::new("/b/c"), None).is_ok());
//     assert!(perms.write.check(Path::new("/b/c"), None).is_ok());
//     assert!(perms.ffi.check(Path::new("/b/c"), None).is_ok());

//     // Sub path within /b/c
//     assert!(perms.read.check(Path::new("/b/c/sub/path"), None).is_ok());
//     assert!(perms.write.check(Path::new("/b/c/sub/path"), None).is_ok());
//     assert!(perms.ffi.check(Path::new("/b/c/sub/path"), None).is_ok());

//     // Sub path within /b/c, needs normalizing
//     assert!(perms
//       .read
//       .check(Path::new("/b/c/sub/path/../path/."), None)
//       .is_ok());
//     assert!(perms
//       .write
//       .check(Path::new("/b/c/sub/path/../path/."), None)
//       .is_ok());
//     assert!(perms
//       .ffi
//       .check(Path::new("/b/c/sub/path/../path/."), None)
//       .is_ok());

//     // Inside of /b but outside of /b/c
//     assert!(perms.read.check(Path::new("/b/e"), None).is_err());
//     assert!(perms.write.check(Path::new("/b/e"), None).is_err());
//     assert!(perms.ffi.check(Path::new("/b/e"), None).is_err());

//     // Inside of /a but outside of /a/specific
//     assert!(perms.read.check(Path::new("/a/b"), None).is_err());
//     assert!(perms.write.check(Path::new("/a/b"), None).is_err());
//     assert!(perms.ffi.check(Path::new("/a/b"), None).is_err());
//   }

//   #[test]
//   fn test_check_net_with_values() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_net: Some(svec![
//         "localhost",
//         "deno.land",
//         "github.com:3000",
//         "127.0.0.1",
//         "172.16.0.2:8000",
//         "www.github.com:443",
//         "80.example.com:80",
//         "443.example.com:443"
//       ]),
//       ..Default::default()
//     })
//     .unwrap();

//     let domain_tests = vec![
//       ("localhost", 1234, true),
//       ("deno.land", 0, true),
//       ("deno.land", 3000, true),
//       ("deno.lands", 0, false),
//       ("deno.lands", 3000, false),
//       ("github.com", 3000, true),
//       ("github.com", 0, false),
//       ("github.com", 2000, false),
//       ("github.net", 3000, false),
//       ("127.0.0.1", 0, true),
//       ("127.0.0.1", 3000, true),
//       ("127.0.0.2", 0, false),
//       ("127.0.0.2", 3000, false),
//       ("172.16.0.2", 8000, true),
//       ("172.16.0.2", 0, false),
//       ("172.16.0.2", 6000, false),
//       ("172.16.0.1", 8000, false),
//       ("443.example.com", 444, false),
//       ("80.example.com", 81, false),
//       ("80.example.com", 80, true),
//       // Just some random hosts that should err
//       ("somedomain", 0, false),
//       ("192.168.0.1", 0, false),
//     ];

//     for (host, port, is_ok) in domain_tests {
//       let host = Host::parse(host).unwrap();
//       let descriptor = NetDescriptor(host, Some(port));
//       assert_eq!(
//         is_ok,
//         perms.net.check(&descriptor, None).is_ok(),
//         "{descriptor}",
//       );
//     }
//   }

//   #[test]
//   fn test_check_net_only_flag() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_net: Some(svec![]), // this means `--allow-net` is present without values following `=` sign
//       ..Default::default()
//     })
//     .unwrap();

//     let domain_tests = vec![
//       ("localhost", 1234),
//       ("deno.land", 0),
//       ("deno.land", 3000),
//       ("deno.lands", 0),
//       ("deno.lands", 3000),
//       ("github.com", 3000),
//       ("github.com", 0),
//       ("github.com", 2000),
//       ("github.net", 3000),
//       ("127.0.0.1", 0),
//       ("127.0.0.1", 3000),
//       ("127.0.0.2", 0),
//       ("127.0.0.2", 3000),
//       ("172.16.0.2", 8000),
//       ("172.16.0.2", 0),
//       ("172.16.0.2", 6000),
//       ("172.16.0.1", 8000),
//       ("somedomain", 0),
//       ("192.168.0.1", 0),
//     ];

//     for (host_str, port) in domain_tests {
//       let host = Host::parse(host_str).unwrap();
//       let descriptor = NetDescriptor(host, Some(port));
//       assert!(
//         perms.net.check(&descriptor, None).is_ok(),
//         "expected {host_str}:{port} to pass"
//       );
//     }
//   }

//   #[test]
//   fn test_check_net_no_flag() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_net: None,
//       ..Default::default()
//     })
//     .unwrap();

//     let domain_tests = vec![
//       ("localhost", 1234),
//       ("deno.land", 0),
//       ("deno.land", 3000),
//       ("deno.lands", 0),
//       ("deno.lands", 3000),
//       ("github.com", 3000),
//       ("github.com", 0),
//       ("github.com", 2000),
//       ("github.net", 3000),
//       ("127.0.0.1", 0),
//       ("127.0.0.1", 3000),
//       ("127.0.0.2", 0),
//       ("127.0.0.2", 3000),
//       ("172.16.0.2", 8000),
//       ("172.16.0.2", 0),
//       ("172.16.0.2", 6000),
//       ("172.16.0.1", 8000),
//       ("somedomain", 0),
//       ("192.168.0.1", 0),
//     ];

//     for (host_str, port) in domain_tests {
//       let host = Host::parse(host_str).unwrap();
//       let descriptor = NetDescriptor(host, Some(port));
//       assert!(
//         perms.net.check(&descriptor, None).is_err(),
//         "expected {host_str}:{port} to fail"
//       );
//     }
//   }

//   #[test]
//   fn test_check_net_url() {
//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_net: Some(svec![
//         "localhost",
//         "deno.land",
//         "github.com:3000",
//         "127.0.0.1",
//         "172.16.0.2:8000",
//         "www.github.com:443"
//       ]),
//       ..Default::default()
//     })
//     .unwrap();

//     let url_tests = vec![
//       // Any protocol + port for localhost should be ok, since we don't specify
//       ("http://localhost", true),
//       ("https://localhost", true),
//       ("https://localhost:4443", true),
//       ("tcp://localhost:5000", true),
//       ("udp://localhost:6000", true),
//       // Correct domain + any port and protocol should be ok incorrect shouldn't
//       ("https://deno.land/std/example/welcome.ts", true),
//       ("https://deno.land:3000/std/example/welcome.ts", true),
//       ("https://deno.lands/std/example/welcome.ts", false),
//       ("https://deno.lands:3000/std/example/welcome.ts", false),
//       // Correct domain + port should be ok all other combinations should err
//       ("https://github.com:3000/denoland/deno", true),
//       ("https://github.com/denoland/deno", false),
//       ("https://github.com:2000/denoland/deno", false),
//       ("https://github.net:3000/denoland/deno", false),
//       // Correct ipv4 address + any port should be ok others should err
//       ("tcp://127.0.0.1", true),
//       ("https://127.0.0.1", true),
//       ("tcp://127.0.0.1:3000", true),
//       ("https://127.0.0.1:3000", true),
//       ("tcp://127.0.0.2", false),
//       ("https://127.0.0.2", false),
//       ("tcp://127.0.0.2:3000", false),
//       ("https://127.0.0.2:3000", false),
//       // Correct address + port should be ok all other combinations should err
//       ("tcp://172.16.0.2:8000", true),
//       ("https://172.16.0.2:8000", true),
//       ("tcp://172.16.0.2", false),
//       ("https://172.16.0.2", false),
//       ("tcp://172.16.0.2:6000", false),
//       ("https://172.16.0.2:6000", false),
//       ("tcp://172.16.0.1:8000", false),
//       ("https://172.16.0.1:8000", false),
//       // Testing issue #6531 (Network permissions check doesn't account for well-known default ports) so we dont regress
//       ("https://www.github.com:443/robots.txt", true),
//     ];

//     for (url_str, is_ok) in url_tests {
//       let u = url::Url::parse(url_str).unwrap();
//       assert_eq!(is_ok, perms.net.check_url(&u, None).is_ok(), "{}", u);
//     }
//   }

//   #[test]
//   fn check_specifiers() {
//     set_prompter(Box::new(TestPrompter));
//     let read_allowlist = if cfg!(target_os = "windows") {
//       vec![PathBuf::from("C:\\a")]
//     } else {
//       vec![PathBuf::from("/a")]
//     };
//     let mut perms = Permissions::from_options(&PermissionsOptions {
//       allow_read: Some(read_allowlist),
//       allow_net: Some(svec!["localhost"]),
//       ..Default::default()
//     })
//     .unwrap();

//     let mut fixtures = vec![
//       (
//         ModuleSpecifier::parse("http://localhost:4545/mod.ts").unwrap(),
//         true,
//       ),
//       (
//         ModuleSpecifier::parse("http://deno.land/x/mod.ts").unwrap(),
//         false,
//       ),
//       (
//         ModuleSpecifier::parse("data:text/plain,Hello%2C%20Deno!").unwrap(),
//         true,
//       ),
//     ];

//     if cfg!(target_os = "windows") {
//       fixtures
//         .push((ModuleSpecifier::parse("file:///C:/a/mod.ts").unwrap(), true));
//       fixtures.push((
//         ModuleSpecifier::parse("file:///C:/b/mod.ts").unwrap(),
//         false,
//       ));
//     } else {
//       fixtures
//         .push((ModuleSpecifier::parse("file:///a/mod.ts").unwrap(), true));
//       fixtures
//         .push((ModuleSpecifier::parse("file:///b/mod.ts").unwrap(), false));
//     }

//     for (specifier, expected) in fixtures {
//       assert_eq!(
//         perms.check_specifier(&specifier).is_ok(),
//         expected,
//         "{}",
//         specifier,
//       );
//     }
//   }

//   #[test]
//   fn check_invalid_specifiers() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::allow_all();

//     let mut test_cases = vec![];

//     if cfg!(target_os = "windows") {
//       test_cases.push("file://");
//       test_cases.push("file:///");
//     } else {
//       test_cases.push("file://remotehost/");
//     }

//     for url in test_cases {
//       assert!(perms
//         .check_specifier(&ModuleSpecifier::parse(url).unwrap())
//         .is_err());
//     }
//   }

//   #[test]
//   fn test_query() {
//     set_prompter(Box::new(TestPrompter));
//     let perms1 = Permissions::allow_all();
//     let perms2 = Permissions {
//       read: Permissions::new_unary(Some(&[PathBuf::from("/foo")]), None, false)
//         .unwrap(),
//       write: Permissions::new_unary(
//         Some(&[PathBuf::from("/foo")]),
//         None,
//         false,
//       )
//       .unwrap(),
//       ffi: Permissions::new_unary(Some(&[PathBuf::from("/foo")]), None, false)
//         .unwrap(),
//       net: Permissions::new_unary(Some(&sarr!["127.0.0.1:8000"]), None, false)
//         .unwrap(),
//       env: Permissions::new_unary(Some(&sarr!["HOME"]), None, false).unwrap(),
//       sys: Permissions::new_unary(Some(&sarr!["hostname"]), None, false)
//         .unwrap(),
//       run: Permissions::new_unary(
//         Some(&["deno".to_string().into()]),
//         None,
//         false,
//       )
//       .unwrap(),
//       all: Permissions::new_all(false),
//     };
//     let perms3 = Permissions {
//       read: Permissions::new_unary(None, Some(&[PathBuf::from("/foo")]), false)
//         .unwrap(),
//       write: Permissions::new_unary(
//         None,
//         Some(&[PathBuf::from("/foo")]),
//         false,
//       )
//       .unwrap(),
//       ffi: Permissions::new_unary(None, Some(&[PathBuf::from("/foo")]), false)
//         .unwrap(),
//       net: Permissions::new_unary(None, Some(&sarr!["127.0.0.1:8000"]), false)
//         .unwrap(),
//       env: Permissions::new_unary(None, Some(&sarr!["HOME"]), false).unwrap(),
//       sys: Permissions::new_unary(None, Some(&sarr!["hostname"]), false)
//         .unwrap(),
//       run: Permissions::new_unary(
//         None,
//         Some(&["deno".to_string().into()]),
//         false,
//       )
//       .unwrap(),
//       all: Permissions::new_all(false),
//     };
//     let perms4 = Permissions {
//       read: Permissions::new_unary(
//         Some(&[]),
//         Some(&[PathBuf::from("/foo")]),
//         false,
//       )
//       .unwrap(),
//       write: Permissions::new_unary(
//         Some(&[]),
//         Some(&[PathBuf::from("/foo")]),
//         false,
//       )
//       .unwrap(),
//       ffi: Permissions::new_unary(
//         Some(&[]),
//         Some(&[PathBuf::from("/foo")]),
//         false,
//       )
//       .unwrap(),
//       net: Permissions::new_unary(
//         Some(&[]),
//         Some(&sarr!["127.0.0.1:8000"]),
//         false,
//       )
//       .unwrap(),
//       env: Permissions::new_unary(Some(&[]), Some(&sarr!["HOME"]), false)
//         .unwrap(),
//       sys: Permissions::new_unary(Some(&[]), Some(&sarr!["hostname"]), false)
//         .unwrap(),
//       run: Permissions::new_unary(
//         Some(&[]),
//         Some(&["deno".to_string().into()]),
//         false,
//       )
//       .unwrap(),
//       all: Permissions::new_all(false),
//     };
//     #[rustfmt::skip]
//     {
//       assert_eq!(perms1.read.query(None), PermissionState::Granted);
//       assert_eq!(perms1.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.read.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.read.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.read.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
//       assert_eq!(perms3.read.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.read.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms3.read.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.read.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.read.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms4.read.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.read.query(Some(Path::new("/bar"))), PermissionState::Granted);
//       assert_eq!(perms1.write.query(None), PermissionState::Granted);
//       assert_eq!(perms1.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.write.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.write.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.write.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
//       assert_eq!(perms3.write.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.write.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms3.write.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.write.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.write.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms4.write.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.write.query(Some(Path::new("/bar"))), PermissionState::Granted);
//       assert_eq!(perms1.ffi.query(None), PermissionState::Granted);
//       assert_eq!(perms1.ffi.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.ffi.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.ffi.query(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms2.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Granted);
//       assert_eq!(perms3.ffi.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.ffi.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms3.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.ffi.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.ffi.query(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms4.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Denied);
//       assert_eq!(perms4.ffi.query(Some(Path::new("/bar"))), PermissionState::Granted);
//       assert_eq!(perms1.net.query(None), PermissionState::Granted);
//       assert_eq!(perms1.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), None))), PermissionState::Granted);
//       assert_eq!(perms2.net.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)))), PermissionState::Granted);
//       assert_eq!(perms3.net.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)))), PermissionState::Denied);
//       assert_eq!(perms4.net.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)))), PermissionState::Denied);
//       assert_eq!(perms4.net.query(Some(&NetDescriptor(Host::must_parse("192.168.0.1"), Some(8000)))), PermissionState::Granted);
//       assert_eq!(perms1.env.query(None), PermissionState::Granted);
//       assert_eq!(perms1.env.query(Some("HOME")), PermissionState::Granted);
//       assert_eq!(perms2.env.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.env.query(Some("HOME")), PermissionState::Granted);
//       assert_eq!(perms3.env.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.env.query(Some("HOME")), PermissionState::Denied);
//       assert_eq!(perms4.env.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.env.query(Some("HOME")), PermissionState::Denied);
//       assert_eq!(perms4.env.query(Some("AWAY")), PermissionState::Granted);
//       assert_eq!(perms1.sys.query(None), PermissionState::Granted);
//       assert_eq!(perms1.sys.query(Some("HOME")), PermissionState::Granted);
//       assert_eq!(perms2.sys.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.sys.query(Some("hostname")), PermissionState::Granted);
//       assert_eq!(perms3.sys.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.sys.query(Some("hostname")), PermissionState::Denied);
//       assert_eq!(perms4.sys.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.sys.query(Some("hostname")), PermissionState::Denied);
//       assert_eq!(perms4.sys.query(Some("uid")), PermissionState::Granted);
//       assert_eq!(perms1.run.query(None), PermissionState::Granted);
//       assert_eq!(perms1.run.query(Some("deno")), PermissionState::Granted);
//       assert_eq!(perms2.run.query(None), PermissionState::Prompt);
//       assert_eq!(perms2.run.query(Some("deno")), PermissionState::Granted);
//       assert_eq!(perms3.run.query(None), PermissionState::Prompt);
//       assert_eq!(perms3.run.query(Some("deno")), PermissionState::Denied);
//       assert_eq!(perms4.run.query(None), PermissionState::GrantedPartial);
//       assert_eq!(perms4.run.query(Some("deno")), PermissionState::Denied);
//       assert_eq!(perms4.run.query(Some("node")), PermissionState::Granted);
//     };
//   }

//   #[test]
//   fn test_request() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms: Permissions = Permissions::none_without_prompt();
//     #[rustfmt::skip]
//     {
//       let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
//       prompt_value.set(true);
//       assert_eq!(perms.read.request(Some(Path::new("/foo"))), PermissionState::Granted);
//       assert_eq!(perms.read.query(None), PermissionState::Prompt);
//       prompt_value.set(false);
//       assert_eq!(perms.read.request(Some(Path::new("/foo/bar"))), PermissionState::Granted);
//       prompt_value.set(false);
//       assert_eq!(perms.write.request(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms.write.query(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
//       prompt_value.set(true);
//       assert_eq!(perms.write.request(None), PermissionState::Denied);
//       prompt_value.set(false);
//       assert_eq!(perms.ffi.request(Some(Path::new("/foo"))), PermissionState::Denied);
//       assert_eq!(perms.ffi.query(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
//       prompt_value.set(true);
//       assert_eq!(perms.ffi.request(None), PermissionState::Denied);
//       prompt_value.set(true);
//       assert_eq!(perms.net.request(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), None))), PermissionState::Granted);
//       prompt_value.set(false);
//       assert_eq!(perms.net.request(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)))), PermissionState::Granted);
//       prompt_value.set(true);
//       assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
//       assert_eq!(perms.env.query(None), PermissionState::Prompt);
//       prompt_value.set(false);
//       assert_eq!(perms.env.request(Some("HOME")), PermissionState::Granted);
//       prompt_value.set(true);
//       assert_eq!(perms.sys.request(Some("hostname")), PermissionState::Granted);
//       assert_eq!(perms.sys.query(None), PermissionState::Prompt);
//       prompt_value.set(false);
//       assert_eq!(perms.sys.request(Some("hostname")), PermissionState::Granted);
//       prompt_value.set(true);
//       assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
//       assert_eq!(perms.run.query(None), PermissionState::Prompt);
//       prompt_value.set(false);
//       assert_eq!(perms.run.request(Some("deno")), PermissionState::Granted);
//     };
//   }

//   #[test]
//   fn test_revoke() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions {
//       read: Permissions::new_unary(
//         Some(&[PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
//         None,
//         false,
//       )
//       .unwrap(),
//       write: Permissions::new_unary(
//         Some(&[PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
//         None,
//         false,
//       )
//       .unwrap(),
//       ffi: Permissions::new_unary(
//         Some(&[PathBuf::from("/foo"), PathBuf::from("/foo/baz")]),
//         None,
//         false,
//       )
//       .unwrap(),
//       net: Permissions::new_unary(
//         Some(&sarr!["127.0.0.1", "127.0.0.1:8000"]),
//         None,
//         false,
//       )
//       .unwrap(),
//       env: Permissions::new_unary(Some(&sarr!["HOME"]), None, false).unwrap(),
//       sys: Permissions::new_unary(Some(&sarr!["hostname"]), None, false)
//         .unwrap(),
//       run: Permissions::new_unary(
//         Some(&["deno".to_string().into()]),
//         None,
//         false,
//       )
//       .unwrap(),
//       all: Permissions::new_all(false),
//     };
//     #[rustfmt::skip]
//     {
//       assert_eq!(perms.read.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
//       assert_eq!(perms.read.query(Some(Path::new("/foo"))), PermissionState::Prompt);
//       assert_eq!(perms.read.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
//       assert_eq!(perms.write.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
//       assert_eq!(perms.write.query(Some(Path::new("/foo"))), PermissionState::Prompt);
//       assert_eq!(perms.write.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
//       assert_eq!(perms.ffi.revoke(Some(Path::new("/foo/bar"))), PermissionState::Prompt);
//       assert_eq!(perms.ffi.query(Some(Path::new("/foo"))), PermissionState::Prompt);
//       assert_eq!(perms.ffi.query(Some(Path::new("/foo/baz"))), PermissionState::Granted);
//       assert_eq!(perms.net.revoke(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(9000)))), PermissionState::Prompt);
//       assert_eq!(perms.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), None))), PermissionState::Prompt);
//       assert_eq!(perms.net.query(Some(&NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)))), PermissionState::Granted);
//       assert_eq!(perms.env.revoke(Some("HOME")), PermissionState::Prompt);
//       assert_eq!(perms.env.revoke(Some("hostname")), PermissionState::Prompt);
//       assert_eq!(perms.run.revoke(Some("deno")), PermissionState::Prompt);
//     };
//   }

//   #[test]
//   fn test_check() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::none_with_prompt();
//     let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

//     prompt_value.set(true);
//     assert!(perms.read.check(Path::new("/foo"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.read.check(Path::new("/foo"), None).is_ok());
//     assert!(perms.read.check(Path::new("/bar"), None).is_err());

//     prompt_value.set(true);
//     assert!(perms.write.check(Path::new("/foo"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.write.check(Path::new("/foo"), None).is_ok());
//     assert!(perms.write.check(Path::new("/bar"), None).is_err());

//     prompt_value.set(true);
//     assert!(perms.ffi.check(Path::new("/foo"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.ffi.check(Path::new("/foo"), None).is_ok());
//     assert!(perms.ffi.check(Path::new("/bar"), None).is_err());

//     prompt_value.set(true);
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)),
//         None
//       )
//       .is_ok());
//     prompt_value.set(false);
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)),
//         None
//       )
//       .is_ok());
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8001)),
//         None
//       )
//       .is_err());
//     assert!(perms
//       .net
//       .check(&NetDescriptor(Host::must_parse("127.0.0.1"), None), None)
//       .is_err());
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("deno.land"), Some(8000)),
//         None
//       )
//       .is_err());
//     assert!(perms
//       .net
//       .check(&NetDescriptor(Host::must_parse("deno.land"), None), None)
//       .is_err());

//     #[allow(clippy::disallowed_methods)]
//     let cwd = std::env::current_dir().unwrap();
//     prompt_value.set(true);
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "cat",
//           resolved: &cwd.join("cat")
//         },
//         None
//       )
//       .is_ok());
//     prompt_value.set(false);
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "cat",
//           resolved: &cwd.join("cat")
//         },
//         None
//       )
//       .is_ok());
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "ls",
//           resolved: &cwd.join("ls")
//         },
//         None
//       )
//       .is_err());

//     prompt_value.set(true);
//     assert!(perms.env.check("HOME", None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.env.check("HOME", None).is_ok());
//     assert!(perms.env.check("PATH", None).is_err());

//     prompt_value.set(true);
//     assert!(perms.env.check("hostname", None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.env.check("hostname", None).is_ok());
//     assert!(perms.env.check("osRelease", None).is_err());
//   }

//   #[test]
//   fn test_check_fail() {
//     set_prompter(Box::new(TestPrompter));
//     let mut perms = Permissions::none_with_prompt();
//     let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();

//     prompt_value.set(false);
//     assert!(perms.read.check(Path::new("/foo"), None).is_err());
//     prompt_value.set(true);
//     assert!(perms.read.check(Path::new("/foo"), None).is_err());
//     assert!(perms.read.check(Path::new("/bar"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.read.check(Path::new("/bar"), None).is_ok());

//     prompt_value.set(false);
//     assert!(perms.write.check(Path::new("/foo"), None).is_err());
//     prompt_value.set(true);
//     assert!(perms.write.check(Path::new("/foo"), None).is_err());
//     assert!(perms.write.check(Path::new("/bar"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.write.check(Path::new("/bar"), None).is_ok());

//     prompt_value.set(false);
//     assert!(perms.ffi.check(Path::new("/foo"), None).is_err());
//     prompt_value.set(true);
//     assert!(perms.ffi.check(Path::new("/foo"), None).is_err());
//     assert!(perms.ffi.check(Path::new("/bar"), None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.ffi.check(Path::new("/bar"), None).is_ok());

//     prompt_value.set(false);
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)),
//         None
//       )
//       .is_err());
//     prompt_value.set(true);
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8000)),
//         None
//       )
//       .is_err());
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8001)),
//         None
//       )
//       .is_ok());
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("deno.land"), Some(8000)),
//         None
//       )
//       .is_ok());
//     prompt_value.set(false);
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("127.0.0.1"), Some(8001)),
//         None
//       )
//       .is_ok());
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("deno.land"), Some(8000)),
//         None
//       )
//       .is_ok());

//     prompt_value.set(false);
//     #[allow(clippy::disallowed_methods)]
//     let cwd = std::env::current_dir().unwrap();
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "cat",
//           resolved: &cwd.join("cat")
//         },
//         None
//       )
//       .is_err());
//     prompt_value.set(true);
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "cat",
//           resolved: &cwd.join("cat")
//         },
//         None
//       )
//       .is_err());
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "ls",
//           resolved: &cwd.join("ls")
//         },
//         None
//       )
//       .is_ok());
//     prompt_value.set(false);
//     assert!(perms
//       .run
//       .check(
//         RunQueryDescriptor {
//           requested: "ls",
//           resolved: &cwd.join("ls")
//         },
//         None
//       )
//       .is_ok());

//     prompt_value.set(false);
//     assert!(perms.env.check("HOME", None).is_err());
//     prompt_value.set(true);
//     assert!(perms.env.check("HOME", None).is_err());
//     assert!(perms.env.check("PATH", None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.env.check("PATH", None).is_ok());

//     prompt_value.set(false);
//     assert!(perms.sys.check("hostname", None).is_err());
//     prompt_value.set(true);
//     assert!(perms.sys.check("hostname", None).is_err());
//     assert!(perms.sys.check("osRelease", None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.sys.check("osRelease", None).is_ok());
//   }

//   #[test]
//   #[cfg(windows)]
//   fn test_env_windows() {
//     set_prompter(Box::new(TestPrompter));
//     let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
//     let mut perms = Permissions::allow_all();
//     perms.env = UnaryPermission {
//       granted_global: false,
//       ..Permissions::new_unary(Some(&sarr!["HOME"]), None, false).unwrap()
//     };

//     prompt_value.set(true);
//     assert!(perms.env.check("HOME", None).is_ok());
//     prompt_value.set(false);
//     assert!(perms.env.check("HOME", None).is_ok());
//     assert!(perms.env.check("hOmE", None).is_ok());

//     assert_eq!(perms.env.revoke(Some("HomE")), PermissionState::Prompt);
//   }

//   #[test]
//   fn test_check_partial_denied() {
//     let mut perms = Permissions {
//       read: Permissions::new_unary(
//         Some(&[]),
//         Some(&[PathBuf::from("/foo/bar")]),
//         false,
//       )
//       .unwrap(),
//       write: Permissions::new_unary(
//         Some(&[]),
//         Some(&[PathBuf::from("/foo/bar")]),
//         false,
//       )
//       .unwrap(),
//       ..Permissions::none_without_prompt()
//     };

//     perms.read.check_partial(Path::new("/foo"), None).unwrap();
//     assert!(perms.read.check(Path::new("/foo"), None).is_err());

//     perms.write.check_partial(Path::new("/foo"), None).unwrap();
//     assert!(perms.write.check(Path::new("/foo"), None).is_err());
//   }

//   #[test]
//   fn test_net_fully_qualified_domain_name() {
//     let mut perms = Permissions {
//       net: Permissions::new_unary(
//         Some(&["allowed.domain".to_string(), "1.1.1.1".to_string()]),
//         Some(&["denied.domain".to_string(), "2.2.2.2".to_string()]),
//         false,
//       )
//       .unwrap(),
//       ..Permissions::none_without_prompt()
//     };

//     perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("allowed.domain."), None),
//         None,
//       )
//       .unwrap();
//     perms
//       .net
//       .check(&NetDescriptor(Host::must_parse("1.1.1.1"), None), None)
//       .unwrap();
//     assert!(perms
//       .net
//       .check(
//         &NetDescriptor(Host::must_parse("denied.domain."), None),
//         None
//       )
//       .is_err());
//     assert!(perms
//       .net
//       .check(&NetDescriptor(Host::must_parse("2.2.2.2"), None), None)
//       .is_err());
//   }

//   #[test]
//   fn test_deserialize_child_permissions_arg() {
//     set_prompter(Box::new(TestPrompter));
//     assert_eq!(
//       ChildPermissionsArg::inherit(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::Inherit,
//         net: ChildUnaryPermissionArg::Inherit,
//         ffi: ChildUnaryPermissionArg::Inherit,
//         read: ChildUnaryPermissionArg::Inherit,
//         run: ChildUnaryPermissionArg::Inherit,
//         sys: ChildUnaryPermissionArg::Inherit,
//         write: ChildUnaryPermissionArg::Inherit,
//       }
//     );
//     assert_eq!(
//       ChildPermissionsArg::none(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::NotGranted,
//         net: ChildUnaryPermissionArg::NotGranted,
//         ffi: ChildUnaryPermissionArg::NotGranted,
//         read: ChildUnaryPermissionArg::NotGranted,
//         run: ChildUnaryPermissionArg::NotGranted,
//         sys: ChildUnaryPermissionArg::NotGranted,
//         write: ChildUnaryPermissionArg::NotGranted,
//       }
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!("inherit")).unwrap(),
//       ChildPermissionsArg::inherit()
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!("none")).unwrap(),
//       ChildPermissionsArg::none()
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!({})).unwrap(),
//       ChildPermissionsArg::none()
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!({
//         "env": ["foo", "bar"],
//       }))
//       .unwrap(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
//         ..ChildPermissionsArg::none()
//       }
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!({
//         "env": true,
//         "net": true,
//         "ffi": true,
//         "read": true,
//         "run": true,
//         "sys": true,
//         "write": true,
//       }))
//       .unwrap(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::Granted,
//         net: ChildUnaryPermissionArg::Granted,
//         ffi: ChildUnaryPermissionArg::Granted,
//         read: ChildUnaryPermissionArg::Granted,
//         run: ChildUnaryPermissionArg::Granted,
//         sys: ChildUnaryPermissionArg::Granted,
//         write: ChildUnaryPermissionArg::Granted,
//       }
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!({
//         "env": false,
//         "net": false,
//         "ffi": false,
//         "read": false,
//         "run": false,
//         "sys": false,
//         "write": false,
//       }))
//       .unwrap(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::NotGranted,
//         net: ChildUnaryPermissionArg::NotGranted,
//         ffi: ChildUnaryPermissionArg::NotGranted,
//         read: ChildUnaryPermissionArg::NotGranted,
//         run: ChildUnaryPermissionArg::NotGranted,
//         sys: ChildUnaryPermissionArg::NotGranted,
//         write: ChildUnaryPermissionArg::NotGranted,
//       }
//     );
//     assert_eq!(
//       serde_json::from_value::<ChildPermissionsArg>(json!({
//         "env": ["foo", "bar"],
//         "net": ["foo", "bar:8000"],
//         "ffi": ["foo", "file:///bar/baz"],
//         "read": ["foo", "file:///bar/baz"],
//         "run": ["foo", "file:///bar/baz", "./qux"],
//         "sys": ["hostname", "osRelease"],
//         "write": ["foo", "file:///bar/baz"],
//       }))
//       .unwrap(),
//       ChildPermissionsArg {
//         env: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
//         net: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar:8000"]),
//         ffi: ChildUnaryPermissionArg::GrantedList(svec![
//           "foo",
//           "file:///bar/baz"
//         ]),
//         read: ChildUnaryPermissionArg::GrantedList(svec![
//           "foo",
//           "file:///bar/baz"
//         ]),
//         run: ChildUnaryPermissionArg::GrantedList(svec![
//           "foo",
//           "file:///bar/baz",
//           "./qux"
//         ]),
//         sys: ChildUnaryPermissionArg::GrantedList(svec![
//           "hostname",
//           "osRelease"
//         ]),
//         write: ChildUnaryPermissionArg::GrantedList(svec![
//           "foo",
//           "file:///bar/baz"
//         ]),
//       }
//     );
//   }

//   #[test]
//   fn test_create_child_permissions() {
//     set_prompter(Box::new(TestPrompter));
//     let mut main_perms = Permissions {
//       env: Permissions::new_unary(Some(&[]), None, false).unwrap(),
//       net: Permissions::new_unary(Some(&sarr!["foo", "bar"]), None, false)
//         .unwrap(),
//       ..Permissions::none_without_prompt()
//     };
//     assert_eq!(
//       create_child_permissions(
//         &mut main_perms.clone(),
//         ChildPermissionsArg {
//           env: ChildUnaryPermissionArg::Inherit,
//           net: ChildUnaryPermissionArg::GrantedList(svec!["foo"]),
//           ffi: ChildUnaryPermissionArg::NotGranted,
//           ..ChildPermissionsArg::none()
//         }
//       )
//       .unwrap(),
//       Permissions {
//         env: Permissions::new_unary(Some(&[]), None, false).unwrap(),
//         net: Permissions::new_unary(Some(&sarr!["foo"]), None, false).unwrap(),
//         ..Permissions::none_without_prompt()
//       }
//     );
//     assert!(create_child_permissions(
//       &mut main_perms.clone(),
//       ChildPermissionsArg {
//         net: ChildUnaryPermissionArg::Granted,
//         ..ChildPermissionsArg::none()
//       }
//     )
//     .is_err());
//     assert!(create_child_permissions(
//       &mut main_perms.clone(),
//       ChildPermissionsArg {
//         net: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar", "baz"]),
//         ..ChildPermissionsArg::none()
//       }
//     )
//     .is_err());
//     assert!(create_child_permissions(
//       &mut main_perms,
//       ChildPermissionsArg {
//         ffi: ChildUnaryPermissionArg::GrantedList(svec!["foo"]),
//         ..ChildPermissionsArg::none()
//       }
//     )
//     .is_err());
//   }

//   #[test]
//   fn test_create_child_permissions_with_prompt() {
//     set_prompter(Box::new(TestPrompter));
//     let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
//     let mut main_perms = Permissions::from_options(&PermissionsOptions {
//       prompt: true,
//       ..Default::default()
//     })
//     .unwrap();
//     prompt_value.set(true);
//     let worker_perms = create_child_permissions(
//       &mut main_perms,
//       ChildPermissionsArg {
//         read: ChildUnaryPermissionArg::Granted,
//         run: ChildUnaryPermissionArg::GrantedList(svec!["foo", "bar"]),
//         ..ChildPermissionsArg::none()
//       },
//     )
//     .unwrap();
//     assert_eq!(main_perms, worker_perms);
//     assert_eq!(
//       main_perms.run.granted_list,
//       HashSet::from([
//         RunDescriptor::Name("bar".to_owned()),
//         RunDescriptor::Name("foo".to_owned())
//       ])
//     );
//   }

//   #[test]
//   fn test_create_child_permissions_with_inherited_denied_list() {
//     set_prompter(Box::new(TestPrompter));
//     let prompt_value = PERMISSION_PROMPT_STUB_VALUE_SETTER.lock();
//     let mut main_perms = Permissions::from_options(&PermissionsOptions {
//       prompt: true,
//       ..Default::default()
//     })
//     .unwrap();
//     prompt_value.set(false);
//     assert!(main_perms.write.check(&PathBuf::from("foo"), None).is_err());
//     let worker_perms = create_child_permissions(
//       &mut main_perms.clone(),
//       ChildPermissionsArg::none(),
//     )
//     .unwrap();
//     assert_eq!(
//       worker_perms.write.flag_denied_list,
//       main_perms.write.flag_denied_list
//     );
//   }

//   #[test]
//   fn test_handle_empty_value() {
//     set_prompter(Box::new(TestPrompter));

//     assert!(Permissions::new_unary::<ReadDescriptor>(
//       Some(&[Default::default()]),
//       None,
//       false
//     )
//     .is_err());
//     assert!(Permissions::new_unary::<EnvDescriptor>(
//       Some(&[Default::default()]),
//       None,
//       false
//     )
//     .is_err());
//     assert!(Permissions::new_unary::<NetDescriptor>(
//       Some(&[Default::default()]),
//       None,
//       false
//     )
//     .is_err());
//   }

//   #[test]
//   fn test_host_parse() {
//     let hosts = &[
//       ("deno.land", Some(Host::Fqdn(fqdn!("deno.land")))),
//       ("DENO.land", Some(Host::Fqdn(fqdn!("deno.land")))),
//       ("deno.land.", Some(Host::Fqdn(fqdn!("deno.land")))),
//       (
//         "1.1.1.1",
//         Some(Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)))),
//       ),
//       (
//         "::1",
//         Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))),
//       ),
//       (
//         "[::1]",
//         Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))),
//       ),
//       ("[::1", None),
//       ("::1]", None),
//       ("deno. land", None),
//       ("1. 1.1.1", None),
//       ("1.1.1.1.", None),
//       ("1::1.", None),
//       ("deno.land.", Some(Host::Fqdn(fqdn!("deno.land")))),
//       (".deno.land", None),
//       (
//         "::ffff:1.1.1.1",
//         Some(Host::Ip(IpAddr::V6(Ipv6Addr::new(
//           0, 0, 0, 0, 0, 0xffff, 0x0101, 0x0101,
//         )))),
//       ),
//     ];

//     for (host_str, expected) in hosts {
//       assert_eq!(Host::parse(host_str).ok(), *expected, "{host_str}");
//     }
//   }

//   #[test]
//   fn test_net_descriptor_parse() {
//     let cases = &[
//       (
//         "deno.land",
//         Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), None)),
//       ),
//       (
//         "DENO.land",
//         Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), None)),
//       ),
//       (
//         "deno.land:8000",
//         Some(NetDescriptor(Host::Fqdn(fqdn!("deno.land")), Some(8000))),
//       ),
//       ("deno.land:", None),
//       ("deno.land:a", None),
//       ("deno. land:a", None),
//       ("deno.land.: a", None),
//       (
//         "1.1.1.1",
//         Some(NetDescriptor(
//           Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
//           None,
//         )),
//       ),
//       ("1.1.1.1.", None),
//       ("1.1.1.1..", None),
//       (
//         "1.1.1.1:8000",
//         Some(NetDescriptor(
//           Host::Ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
//           Some(8000),
//         )),
//       ),
//       ("::", None),
//       (":::80", None),
//       ("::80", None),
//       (
//         "[::]",
//         Some(NetDescriptor(
//           Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0))),
//           None,
//         )),
//       ),
//       ("[::1", None),
//       ("::1]", None),
//       ("::1]", None),
//       ("[::1]:", None),
//       ("[::1]:a", None),
//       (
//         "[::1]:443",
//         Some(NetDescriptor(
//           Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))),
//           Some(443),
//         )),
//       ),
//       ("", None),
//       ("deno.land..", None),
//     ];

//     for (input, expected) in cases {
//       assert_eq!(NetDescriptor::parse(input).ok(), *expected, "'{input}'");
//     }
//   }
// }
