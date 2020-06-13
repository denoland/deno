// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::flags::Flags;
use crate::fs::resolve_from_cwd;
use crate::op_error::OpError;
use serde::de;
use serde::Deserialize;
use std::collections::HashSet;
use std::env::current_dir;
use std::fmt;
#[cfg(not(test))]
use std::io;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::sync::Mutex;
use url::Url;

const PERMISSION_EMOJI: &str = "⚠️";

/// Tri-state value for storing permission state
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum PermissionState {
  Allow = 0,
  Ask = 1,
  Deny = 2,
}

impl PermissionState {
  /// Checks the permission state and returns the result.
  pub fn check(self, msg: &str, flag_name: &str) -> Result<(), OpError> {
    if self == PermissionState::Allow {
      log_perm_access(msg);
      return Ok(());
    }
    let m = format!("{}, run again with the {} flag", msg, flag_name);
    Err(OpError::permission_denied(m))
  }
  pub fn is_allow(self) -> bool {
    self == PermissionState::Allow
  }
  /// If the state is "Allow" walk it back to the default "Ask"
  /// Don't do anything if state is "Deny"
  pub fn revoke(&mut self) {
    if *self == PermissionState::Allow {
      *self = PermissionState::Ask;
    }
  }
  /// Requests the permission.
  pub fn request(&mut self, msg: &str) -> PermissionState {
    if *self != PermissionState::Ask {
      return *self;
    }
    if permission_prompt(msg) {
      *self = PermissionState::Allow;
    } else {
      *self = PermissionState::Deny;
    }
    *self
  }

  pub fn fork(self, value: bool) -> Result<PermissionState, OpError> {
    if value && self == PermissionState::Deny {
      Err(OpError::permission_denied(
        "Arguments escalate parent permissions.".to_string(),
      ))
    } else if value {
      Ok(PermissionState::Allow)
    } else {
      Ok(PermissionState::Deny)
    }
  }
}

impl From<usize> for PermissionState {
  fn from(val: usize) -> Self {
    match val {
      0 => PermissionState::Allow,
      1 => PermissionState::Ask,
      2 => PermissionState::Deny,
      _ => unreachable!(),
    }
  }
}

impl From<bool> for PermissionState {
  fn from(val: bool) -> Self {
    if val {
      PermissionState::Allow
    } else {
      PermissionState::Ask
    }
  }
}

impl fmt::Display for PermissionState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PermissionState::Allow => f.pad("granted"),
      PermissionState::Ask => f.pad("prompt"),
      PermissionState::Deny => f.pad("denied"),
    }
  }
}

impl Default for PermissionState {
  fn default() -> Self {
    PermissionState::Ask
  }
}

struct BoolPermVisitor;

fn deserialize_permission_state<'de, D>(
  d: D,
) -> Result<PermissionState, D::Error>
where
  D: de::Deserializer<'de>,
{
  impl<'de> de::Visitor<'de> for BoolPermVisitor {
    type Value = PermissionState;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a boolean value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      if value {
        Ok(PermissionState::Allow)
      } else {
        Ok(PermissionState::Deny)
      }
    }
  }
  d.deserialize_bool(BoolPermVisitor)
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Permissions {
  // Keep in sync with cli/js/permissions.ts
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_read: PermissionState,
  pub read_allowlist: HashSet<PathBuf>,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_write: PermissionState,
  pub write_allowlist: HashSet<PathBuf>,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_net: PermissionState,
  pub net_allowlist: HashSet<String>,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_env: PermissionState,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_run: PermissionState,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_plugin: PermissionState,
  #[serde(deserialize_with = "deserialize_permission_state")]
  pub allow_hrtime: PermissionState,
}

fn resolve_fs_allowlist(allowlist: &[PathBuf]) -> HashSet<PathBuf> {
  allowlist
    .iter()
    .map(|raw_path| resolve_from_cwd(Path::new(&raw_path)).unwrap())
    .collect()
}

impl Permissions {
  pub fn from_flags(flags: &Flags) -> Self {
    Self {
      allow_read: PermissionState::from(flags.allow_read),
      read_allowlist: resolve_fs_allowlist(&flags.read_allowlist),
      allow_write: PermissionState::from(flags.allow_write),
      write_allowlist: resolve_fs_allowlist(&flags.write_allowlist),
      allow_net: PermissionState::from(flags.allow_net),
      net_allowlist: flags.net_allowlist.iter().cloned().collect(),
      allow_env: PermissionState::from(flags.allow_env),
      allow_run: PermissionState::from(flags.allow_run),
      allow_plugin: PermissionState::from(flags.allow_plugin),
      allow_hrtime: PermissionState::from(flags.allow_hrtime),
    }
  }

  /// Arbitrary helper. Resolves the path from CWD, and also gets a path that
  /// can be displayed without leaking the CWD when not allowed.
  fn resolved_and_display_path(&self, path: &Path) -> (PathBuf, PathBuf) {
    let resolved_path = resolve_from_cwd(path).unwrap();
    let display_path = if path.is_absolute() {
      path.to_path_buf()
    } else {
      match self
        .get_state_read(&Some(&current_dir().unwrap()))
        .check("", "")
      {
        Ok(_) => resolved_path.clone(),
        Err(_) => path.to_path_buf(),
      }
    };
    (resolved_path, display_path)
  }

  pub fn allow_all() -> Self {
    Self {
      allow_read: PermissionState::from(true),
      allow_write: PermissionState::from(true),
      allow_net: PermissionState::from(true),
      allow_env: PermissionState::from(true),
      allow_run: PermissionState::from(true),
      allow_plugin: PermissionState::from(true),
      allow_hrtime: PermissionState::from(true),
      ..Default::default()
    }
  }

  pub fn check_run(&self) -> Result<(), OpError> {
    self
      .allow_run
      .check("access to run a subprocess", "--allow-run")
  }

  fn get_state_read(&self, path: &Option<&Path>) -> PermissionState {
    if path.map_or(false, |f| check_path_white_list(f, &self.read_allowlist)) {
      return PermissionState::Allow;
    }
    self.allow_read
  }

  pub fn check_read(&self, path: &Path) -> Result<(), OpError> {
    let (resolved_path, display_path) = self.resolved_and_display_path(path);
    self.get_state_read(&Some(&resolved_path)).check(
      &format!("read access to \"{}\"", display_path.display()),
      "--allow-read",
    )
  }

  /// As `check_read()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  pub fn check_read_blind(
    &self,
    path: &Path,
    display: &str,
  ) -> Result<(), OpError> {
    let resolved_path = resolve_from_cwd(path).unwrap();
    self
      .get_state_read(&Some(&resolved_path))
      .check(&format!("read access to <{}>", display), "--allow-read")
  }

  fn get_state_write(&self, path: &Option<&Path>) -> PermissionState {
    if path.map_or(false, |f| check_path_white_list(f, &self.write_allowlist)) {
      return PermissionState::Allow;
    }
    self.allow_write
  }

  pub fn check_write(&self, path: &Path) -> Result<(), OpError> {
    let (resolved_path, display_path) = self.resolved_and_display_path(path);
    self.get_state_write(&Some(&resolved_path)).check(
      &format!("write access to \"{}\"", display_path.display()),
      "--allow-write",
    )
  }

  fn get_state_net(&self, host: &str, port: Option<u16>) -> PermissionState {
    if check_host_and_port_allowlist(host, port, &self.net_allowlist) {
      return PermissionState::Allow;
    }
    self.allow_net
  }

  fn get_state_net_url(
    &self,
    url: &Option<&str>,
  ) -> Result<PermissionState, OpError> {
    if url.is_none() {
      return Ok(self.allow_net);
    }
    let url: &str = url.unwrap();
    // If url is invalid, then throw a TypeError.
    let parsed = Url::parse(url).map_err(OpError::from)?;
    Ok(
      self.get_state_net(&format!("{}", parsed.host().unwrap()), parsed.port()),
    )
  }

  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), OpError> {
    self.get_state_net(hostname, Some(port)).check(
      &format!("network access to \"{}:{}\"", hostname, port),
      "--allow-net",
    )
  }

  pub fn check_net_url(&self, url: &url::Url) -> Result<(), OpError> {
    let host = url
      .host_str()
      .ok_or_else(|| OpError::uri_error("missing host".to_owned()))?;
    self
      .get_state_net(host, url.port())
      .check(&format!("network access to \"{}\"", url), "--allow-net")
  }

  pub fn check_env(&self) -> Result<(), OpError> {
    self
      .allow_env
      .check("access to environment variables", "--allow-env")
  }

  pub fn check_plugin(&self, path: &Path) -> Result<(), OpError> {
    let (_, display_path) = self.resolved_and_display_path(path);
    self.allow_plugin.check(
      &format!("access to open a plugin: {}", display_path.display()),
      "--allow-plugin",
    )
  }

  pub fn request_run(&mut self) -> PermissionState {
    self
      .allow_run
      .request("Deno requests to access to run a subprocess")
  }

  pub fn request_read(&mut self, path: &Option<&Path>) -> PermissionState {
    let paths = path.map(|p| self.resolved_and_display_path(p));
    if let Some((p, _)) = paths.as_ref() {
      if check_path_white_list(&p, &self.read_allowlist) {
        return PermissionState::Allow;
      }
    };
    self.allow_read.request(&match paths {
      None => "Deno requests read access".to_string(),
      Some((_, display_path)) => format!(
        "Deno requests read access to \"{}\"",
        display_path.display()
      ),
    })
  }

  pub fn request_write(&mut self, path: &Option<&Path>) -> PermissionState {
    let paths = path.map(|p| self.resolved_and_display_path(p));
    if let Some((p, _)) = paths.as_ref() {
      if check_path_white_list(&p, &self.write_allowlist) {
        return PermissionState::Allow;
      }
    };
    self.allow_write.request(&match paths {
      None => "Deno requests write access".to_string(),
      Some((_, display_path)) => format!(
        "Deno requests write access to \"{}\"",
        display_path.display()
      ),
    })
  }

  pub fn request_net(
    &mut self,
    url: &Option<&str>,
  ) -> Result<PermissionState, OpError> {
    if self.get_state_net_url(url)? == PermissionState::Ask {
      return Ok(self.allow_net.request(&match url {
        None => "Deno requests network access".to_string(),
        Some(url) => format!("Deno requests network access to \"{}\"", url),
      }));
    };
    self.get_state_net_url(url)
  }

  pub fn request_env(&mut self) -> PermissionState {
    self
      .allow_env
      .request("Deno requests to access to environment variables")
  }

  pub fn request_hrtime(&mut self) -> PermissionState {
    self
      .allow_hrtime
      .request("Deno requests to access to high precision time")
  }

  pub fn request_plugin(&mut self) -> PermissionState {
    self.allow_plugin.request("Deno requests to open plugins")
  }

  pub fn get_permission_state(
    &self,
    name: &str,
    url: &Option<&str>,
    path: &Option<&Path>,
  ) -> Result<PermissionState, OpError> {
    let path = path.map(|p| resolve_from_cwd(p).unwrap());
    let path = path.as_deref();
    match name {
      "run" => Ok(self.allow_run),
      "read" => Ok(self.get_state_read(&path)),
      "write" => Ok(self.get_state_write(&path)),
      "net" => self.get_state_net_url(url),
      "env" => Ok(self.allow_env),
      "plugin" => Ok(self.allow_plugin),
      "hrtime" => Ok(self.allow_hrtime),
      n => Err(OpError::other(format!("No such permission name: {}", n))),
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub fn fork(
    &self,
    allow_read: bool,
    read_allowlist: HashSet<PathBuf>,
    allow_write: bool,
    write_allowlist: HashSet<PathBuf>,
    allow_net: bool,
    net_allowlist: HashSet<String>,
    allow_env: bool,
    allow_run: bool,
    allow_plugin: bool,
    allow_hrtime: bool,
  ) -> Result<Permissions, OpError> {
    let allow_read = self.allow_read.fork(allow_read)?;
    let allow_write = self.allow_write.fork(allow_write)?;
    let allow_net = self.allow_net.fork(allow_net)?;
    let allow_env = self.allow_env.fork(allow_env)?;
    let allow_run = self.allow_run.fork(allow_run)?;
    let allow_plugin = self.allow_plugin.fork(allow_plugin)?;
    let allow_hrtime = self.allow_hrtime.fork(allow_hrtime)?;
    if !(read_allowlist.is_subset(&self.read_allowlist)) {
      Err(OpError::permission_denied(format!(
        "Arguments escalate parent permissions. Parent Permissions have only {:?} in `read_allowlist`",
        self.read_allowlist
      )))
    } else if !(write_allowlist.is_subset(&self.write_allowlist)) {
      Err(OpError::permission_denied(format!(
        "Arguments escalate parent permissions. Parent Permissions have only {:?} in `write_allowlist`",
        self.write_allowlist
      )))
    } else if !(net_allowlist.is_subset(&self.net_allowlist)) {
      Err(OpError::permission_denied(format!(
        "Arguments escalate parent permissions. Parent Permissions have only {:?} in `net_allowlist`",
        self.net_allowlist
      )))
    } else {
      Ok(Permissions {
        allow_read,
        read_allowlist,
        allow_write,
        write_allowlist,
        allow_net,
        net_allowlist,
        allow_env,
        allow_run,
        allow_plugin,
        allow_hrtime,
      })
    }
  }
}

/// Shows the permission prompt and returns the answer according to the user input.
/// This loops until the user gives the proper input.
#[cfg(not(test))]
fn permission_prompt(message: &str) -> bool {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    return false;
  };
  let msg = format!(
    "️{}  {}. Grant? [g/d (g = grant, d = deny)] ",
    PERMISSION_EMOJI, message
  );
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{}", colors::bold(msg));
  loop {
    let mut input = String::new();
    let stdin = io::stdin();
    let result = stdin.read_line(&mut input);
    if result.is_err() {
      return false;
    };
    let ch = input.chars().next().unwrap();
    match ch.to_ascii_lowercase() {
      'g' => return true,
      'd' => return false,
      _ => {
        // If we don't get a recognized option try again.
        let msg_again =
          format!("Unrecognized option '{}' [g/d (g = grant, d = deny)] ", ch);
        eprint!("{}", colors::bold(msg_again));
      }
    };
  }
}

#[cfg(test)]
lazy_static! {
  /// Lock this when you use `set_prompt_result` in a test case.
  static ref PERMISSION_PROMPT_GUARD: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
static STUB_PROMPT_VALUE: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
fn set_prompt_result(value: bool) {
  STUB_PROMPT_VALUE.store(value, Ordering::SeqCst);
}

// When testing, permission prompt returns the value of STUB_PROMPT_VALUE
// which we set from the test functions.
#[cfg(test)]
fn permission_prompt(_message: &str) -> bool {
  STUB_PROMPT_VALUE.load(Ordering::SeqCst)
}

fn log_perm_access(message: &str) {
  debug!(
    "{}",
    colors::bold(format!("{}️  Granted {}", PERMISSION_EMOJI, message))
  );
}

fn check_path_white_list(path: &Path, white_list: &HashSet<PathBuf>) -> bool {
  let mut path_buf = PathBuf::from(path);
  loop {
    if white_list.contains(&path_buf) {
      return true;
    }
    if !path_buf.pop() {
      break;
    }
  }
  false
}

fn check_host_and_port_allowlist(
  host: &str,
  port: Option<u16>,
  allowlist: &HashSet<String>,
) -> bool {
  allowlist.contains(host)
    || (port.is_some()
      && allowlist.contains(&format!("{}:{}", host, port.unwrap())))
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
    let allowlist = vec![
      PathBuf::from("/a/specific/dir/name"),
      PathBuf::from("/a/specific"),
      PathBuf::from("/b/c"),
    ];

    let perms = Permissions::from_flags(&Flags {
      read_allowlist: allowlist.clone(),
      write_allowlist: allowlist,
      ..Default::default()
    });

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms.check_read(Path::new("/a/specific/dir/name")).is_ok());
    assert!(perms.check_write(Path::new("/a/specific/dir/name")).is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.check_read(Path::new("/a/specific/dir")).is_ok());
    assert!(perms.check_write(Path::new("/a/specific/dir")).is_ok());

    // Inside of /a/specific and /a/specific/dir/name
    assert!(perms
      .check_read(Path::new("/a/specific/dir/name/inner"))
      .is_ok());
    assert!(perms
      .check_write(Path::new("/a/specific/dir/name/inner"))
      .is_ok());

    // Inside of /a/specific but outside of /a/specific/dir/name
    assert!(perms.check_read(Path::new("/a/specific/other/dir")).is_ok());
    assert!(perms
      .check_write(Path::new("/a/specific/other/dir"))
      .is_ok());

    // Exact match with /b/c
    assert!(perms.check_read(Path::new("/b/c")).is_ok());
    assert!(perms.check_write(Path::new("/b/c")).is_ok());

    // Sub path within /b/c
    assert!(perms.check_read(Path::new("/b/c/sub/path")).is_ok());
    assert!(perms.check_write(Path::new("/b/c/sub/path")).is_ok());

    // Sub path within /b/c, needs normalizing
    assert!(perms
      .check_read(Path::new("/b/c/sub/path/../path/."))
      .is_ok());
    assert!(perms
      .check_write(Path::new("/b/c/sub/path/../path/."))
      .is_ok());

    // Inside of /b but outside of /b/c
    assert!(perms.check_read(Path::new("/b/e")).is_err());
    assert!(perms.check_write(Path::new("/b/e")).is_err());

    // Inside of /a but outside of /a/specific
    assert!(perms.check_read(Path::new("/a/b")).is_err());
    assert!(perms.check_write(Path::new("/a/b")).is_err());
  }

  #[test]
  fn test_check_net() {
    let perms = Permissions::from_flags(&Flags {
      net_allowlist: svec![
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

  #[test]
  fn test_permissions_request_run() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let mut perms0 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(perms0.request_run(), PermissionState::Allow);

    let mut perms1 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(perms1.request_run(), PermissionState::Deny);
    drop(guard);
  }

  #[test]
  fn test_permissions_request_read() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let allowlist = vec![PathBuf::from("/foo/bar")];
    let mut perms0 = Permissions::from_flags(&Flags {
      read_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(false);
    // If the allowlist contains the path, then the result is `allow`
    // regardless of prompt result
    assert_eq!(
      perms0.request_read(&Some(Path::new("/foo/bar"))),
      PermissionState::Allow
    );

    let mut perms1 = Permissions::from_flags(&Flags {
      read_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(
      perms1.request_read(&Some(Path::new("/foo/baz"))),
      PermissionState::Allow
    );

    let mut perms2 = Permissions::from_flags(&Flags {
      read_allowlist: allowlist,
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(
      perms2.request_read(&Some(Path::new("/foo/baz"))),
      PermissionState::Deny
    );
    drop(guard);
  }

  #[test]
  fn test_permissions_request_write() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let allowlist = vec![PathBuf::from("/foo/bar")];
    let mut perms0 = Permissions::from_flags(&Flags {
      write_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(false);
    // If the allowlist contains the path, then the result is `allow`
    // regardless of prompt result
    assert_eq!(
      perms0.request_write(&Some(Path::new("/foo/bar"))),
      PermissionState::Allow
    );

    let mut perms1 = Permissions::from_flags(&Flags {
      write_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(
      perms1.request_write(&Some(Path::new("/foo/baz"))),
      PermissionState::Allow
    );

    let mut perms2 = Permissions::from_flags(&Flags {
      write_allowlist: allowlist,
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(
      perms2.request_write(&Some(Path::new("/foo/baz"))),
      PermissionState::Deny
    );
    drop(guard);
  }

  #[test]
  fn test_permission_request_net() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let allowlist = svec!["localhost:8080"];

    let mut perms0 = Permissions::from_flags(&Flags {
      net_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(false);
    // If the url matches the allowlist item, then the result is `allow`
    // regardless of prompt result
    assert_eq!(
      perms0
        .request_net(&Some("http://localhost:8080/"))
        .expect("Testing expect"),
      PermissionState::Allow
    );

    let mut perms1 = Permissions::from_flags(&Flags {
      net_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(
      perms1
        .request_net(&Some("http://deno.land/"))
        .expect("Testing expect"),
      PermissionState::Allow
    );

    let mut perms2 = Permissions::from_flags(&Flags {
      net_allowlist: allowlist.clone(),
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(
      perms2
        .request_net(&Some("http://deno.land/"))
        .expect("Testing expect"),
      PermissionState::Deny
    );

    let mut perms3 = Permissions::from_flags(&Flags {
      net_allowlist: allowlist,
      ..Default::default()
    });
    set_prompt_result(true);
    assert!(perms3.request_net(&Some(":")).is_err());
    drop(guard);
  }

  #[test]
  fn test_permissions_request_env() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let mut perms0 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(perms0.request_env(), PermissionState::Allow);

    let mut perms1 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(perms1.request_env(), PermissionState::Deny);
    drop(guard);
  }

  #[test]
  fn test_permissions_request_plugin() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let mut perms0 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(perms0.request_plugin(), PermissionState::Allow);

    let mut perms1 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(perms1.request_plugin(), PermissionState::Deny);
    drop(guard);
  }

  #[test]
  fn test_permissions_request_hrtime() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let mut perms0 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(perms0.request_hrtime(), PermissionState::Allow);

    let mut perms1 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(false);
    assert_eq!(perms1.request_hrtime(), PermissionState::Deny);
    drop(guard);
  }

  #[test]
  fn test_deserialize_perms() {
    let json_perms = r#"
    {
      "allow_read": true,
      "read_allowlist": [],
      "allow_write": true,
      "write_allowlist": [],
      "allow_net": true,
      "net_allowlist": [],
      "allow_env": true,
      "allow_run": true,
      "allow_plugin": true,
      "allow_hrtime": true
    }
    "#;
    let perms0 = Permissions {
      allow_read: PermissionState::Allow,
      allow_write: PermissionState::Allow,
      allow_net: PermissionState::Allow,
      allow_hrtime: PermissionState::Allow,
      allow_env: PermissionState::Allow,
      allow_plugin: PermissionState::Allow,
      allow_run: PermissionState::Allow,
      read_allowlist: HashSet::new(),
      write_allowlist: HashSet::new(),
      net_allowlist: HashSet::new(),
    };
    let deserialized_perms: Permissions =
      serde_json::from_str(json_perms).unwrap();
    assert_eq!(perms0, deserialized_perms);
  }

  #[test]
  fn test_fork() {
    let guard = PERMISSION_PROMPT_GUARD.lock().unwrap();
    let perms0 = Permissions::from_flags(&Flags {
      ..Default::default()
    });
    set_prompt_result(true);
    assert_eq!(
      perms0
        .fork(
          true,
          HashSet::new(),
          true,
          HashSet::new(),
          true,
          HashSet::new(),
          true,
          true,
          false,
          false,
        )
        .expect("Testing expect"),
      Permissions {
        allow_read: PermissionState::Allow,
        read_allowlist: HashSet::new(),
        allow_write: PermissionState::Allow,
        write_allowlist: HashSet::new(),
        allow_net: PermissionState::Allow,
        net_allowlist: HashSet::new(),
        allow_env: PermissionState::Allow,
        allow_run: PermissionState::Allow,
        allow_plugin: PermissionState::Deny,
        allow_hrtime: PermissionState::Deny,
      }
    );
    set_prompt_result(false);
    assert_eq!(
      perms0
        .fork(
          true,
          HashSet::new(),
          true,
          HashSet::new(),
          true,
          HashSet::new(),
          true,
          true,
          false,
          false,
        )
        .expect("Testing expect"),
      Permissions {
        allow_read: PermissionState::Allow,
        read_allowlist: HashSet::new(),
        allow_write: PermissionState::Allow,
        write_allowlist: HashSet::new(),
        allow_net: PermissionState::Allow,
        net_allowlist: HashSet::new(),
        allow_env: PermissionState::Allow,
        allow_run: PermissionState::Allow,
        allow_plugin: PermissionState::Deny,
        allow_hrtime: PermissionState::Deny,
      }
    );
    drop(guard);
  }
}
