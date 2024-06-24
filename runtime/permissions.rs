// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;

use deno_core::error::AnyError;
use deno_core::url::Url;
pub use deno_io::fs::FsError;
pub use deno_permissions::create_child_permissions;
pub use deno_permissions::parse_sys_kind;
pub use deno_permissions::set_prompt_callbacks;
pub use deno_permissions::ChildPermissionsArg;
pub use deno_permissions::Permissions;
pub use deno_permissions::PermissionsOptions;

// NOTE: Temporary permissions container to satisfy traits. We are migrating to the deno_permissions
// crate.
#[derive(Debug, Clone)]

pub struct PermissionsContainer(pub deno_permissions::PermissionsContainer);

impl PermissionsContainer {
  pub fn new(permissions: deno_permissions::Permissions) -> Self {
    Self(deno_permissions::PermissionsContainer::new(permissions))
  }

  pub fn allow_all() -> Self {
    Self(deno_permissions::PermissionsContainer::allow_all())
  }
}

impl std::ops::Deref for PermissionsContainer {
  type Target = deno_permissions::PermissionsContainer;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::ops::DerefMut for PermissionsContainer {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl deno_node::NodePermissions for PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_net_url(url, api_name)
  }

  #[inline(always)]
  fn check_read_with_api_name(
    &self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    self.0.check_read_with_api_name(path, api_name)
  }

  #[inline(always)]
  fn check_write_with_api_name(
    &self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    self.0.check_write_with_api_name(path, api_name)
  }

  fn check_sys(&self, kind: &str, api_name: &str) -> Result<(), AnyError> {
    self.0.check_sys(kind, api_name)
  }
}

impl deno_fetch::FetchPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_net_url(url, api_name)
  }

  #[inline(always)]
  fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_read(path, api_name)
  }
}

impl deno_net::NetPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_net(host, api_name)
  }

  #[inline(always)]
  fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_read(path, api_name)
  }

  #[inline(always)]
  fn check_write(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_write(path, api_name)
  }
}

impl deno_web::TimersPermission for PermissionsContainer {
  #[inline(always)]
  fn allow_hrtime(&mut self) -> bool {
    self.0.allow_hrtime()
  }
}

impl deno_websocket::WebSocketPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_net_url(url, api_name)
  }
}

impl deno_fs::FsPermissions for PermissionsContainer {
  fn check_open<'a>(
    &mut self,
    resolved: bool,
    read: bool,
    write: bool,
    path: &'a Path,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, FsError> {
    if resolved {
      self.check_special_file(path, api_name).map_err(|_| {
        std::io::Error::from(std::io::ErrorKind::PermissionDenied)
      })?;
      return Ok(Cow::Borrowed(path));
    }

    // If somehow read or write aren't specified, use read
    let read = read || !write;
    if read {
      deno_fs::FsPermissions::check_read(self, path, api_name)
        .map_err(|_| FsError::PermissionDenied("read"))?;
    }
    if write {
      deno_fs::FsPermissions::check_write(self, path, api_name)
        .map_err(|_| FsError::PermissionDenied("write"))?;
    }
    Ok(Cow::Borrowed(path))
  }

  fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_read(path, api_name)
  }

  fn check_read_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_read_blind(path, display, api_name)
  }

  fn check_write(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_write(path, api_name)
  }

  fn check_write_partial(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_write_partial(path, api_name)
  }

  fn check_write_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_write_blind(p, display, api_name)
  }

  fn check_read_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.0.check_read_all(api_name)
  }

  fn check_write_all(&mut self, api_name: &str) -> Result<(), AnyError> {
    self.0.check_write_all(api_name)
  }
}

// NOTE(bartlomieju): for now, NAPI uses `--allow-ffi` flag, but that might
// change in the future.
impl deno_napi::NapiPermissions for PermissionsContainer {
  #[inline(always)]
  fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    self.0.check_ffi(path)
  }
}

impl deno_ffi::FfiPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_partial(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
    self.0.check_ffi_partial(path)
  }
}

impl deno_kv::sqlite::SqliteDbHandlerPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError> {
    self.0.check_read(p, api_name)
  }

  #[inline(always)]
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError> {
    self.0.check_write(p, api_name)
  }
}

impl deno_kv::remote::RemoteDbHandlerPermissions for PermissionsContainer {
  #[inline(always)]
  fn check_env(&mut self, var: &str) -> Result<(), AnyError> {
    self.0.check_env(var)
  }

  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    self.0.check_net_url(url, api_name)
  }
}
