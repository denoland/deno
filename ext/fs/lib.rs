// Copyright 2018-2025 the Deno authors. MIT license.

mod interface;
mod ops;
mod std_fs;
pub mod sync;

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

pub use deno_io::fs::FsError;
use deno_permissions::PermissionCheckError;
pub use interface::CheckedPath;

pub use crate::interface::AccessCheckCb;
pub use crate::interface::AccessCheckFn;
pub use crate::interface::FileSystem;
pub use crate::interface::FileSystemRc;
pub use crate::interface::FsDirEntry;
pub use crate::interface::FsFileType;
pub use crate::interface::OpenOptions;
pub use crate::ops::FsOpsError;
pub use crate::ops::FsOpsErrorKind;
pub use crate::ops::OperationError;
use crate::ops::*;
pub use crate::std_fs::open_options_with_access_check;
pub use crate::std_fs::RealFs;
pub use crate::sync::MaybeSend;
pub use crate::sync::MaybeSync;

pub trait FsPermissions {
  fn check_open<'a>(
    &mut self,
    read: bool,
    write: bool,
    path: Cow<'a, Path>,
    api_name: &str,
    get_path: &'a dyn GetPath,
  ) -> Result<CheckedPath<'a>, FsError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_read(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_read_path<'a>(
    &mut self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError>;
  fn check_read_all(
    &mut self,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
  fn check_read_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write_path<'a>(
    &mut self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write_partial(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
  fn check_write_all(
    &mut self,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
  fn check_write_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;

  fn check<'a>(
    &mut self,
    open_options: &OpenOptions,
    path: Cow<'a, Path>,
    api_name: &str,
    get_path: &'a dyn GetPath,
  ) -> Result<CheckedPath<'a>, FsError> {
    self.check_open(
      open_options.read,
      open_options.write || open_options.append,
      path,
      api_name,
      get_path,
    )
  }

  fn allows_all(&self) -> bool {
    false
  }
}

impl FsPermissions for deno_permissions::PermissionsContainer {
  fn check_open<'a>(
    &mut self,
    read: bool,
    write: bool,
    path: Cow<'a, Path>,
    api_name: &str,
    get_path: &'a dyn GetPath,
  ) -> Result<CheckedPath<'a>, FsError> {
    if self.allows_all() {
      return Ok(CheckedPath::Unresolved(path));
    }

    let (needs_canonicalize, path) = get_path.normalized(path)?;
    // If somehow read or write aren't specified, use read
    let read = read || !write;
    let path = if read {
      FsPermissions::check_read_path(self, path, api_name)
        .map_err(|_| FsError::NotCapable("read"))?
    } else {
      path
    };
    let path = if write {
      FsPermissions::check_write_path(self, path.clone(), api_name)
        .map_err(|_| FsError::NotCapable("write"))?
    } else {
      path
    };

    let resolved_path = if needs_canonicalize {
      Cow::Owned(get_path.resolved(&path)?)
    } else {
      path
    };

    self
      .check_special_file(&resolved_path, api_name)
      .map_err(FsError::NotCapable)?;

    if needs_canonicalize {
      Ok(CheckedPath::Resolved(resolved_path))
    } else {
      Ok(CheckedPath::Unresolved(resolved_path))
    }
  }

  fn check_read(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read(self, path, api_name)
  }

  fn check_read_path<'a>(
    &mut self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read_path(
      self,
      path,
      Some(api_name),
    )
  }
  fn check_read_blind(
    &mut self,
    path: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read_blind(
      self, path, display, api_name,
    )
  }

  fn check_write(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write(self, path, api_name)
  }

  fn check_write_path<'a>(
    &mut self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_path(
      self, path, api_name,
    )
  }

  fn check_write_partial(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_partial(
      self, path, api_name,
    )
  }

  fn check_write_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_blind(
      self, p, display, api_name,
    )
  }

  fn check_read_all(
    &mut self,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read_all(self, api_name)
  }

  fn check_write_all(
    &mut self,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_all(self, api_name)
  }

  fn allows_all(&self) -> bool {
    self.allows_all()
  }
}

pub const UNSTABLE_FEATURE_NAME: &str = "fs";

deno_core::extension!(deno_fs,
  deps = [ deno_web ],
  parameters = [P: FsPermissions],
  ops = [
    op_fs_cwd<P>,
    op_fs_umask,
    op_fs_chdir<P>,

    op_fs_open_sync<P>,
    op_fs_open_async<P>,
    op_fs_mkdir_sync<P>,
    op_fs_mkdir_async<P>,
    op_fs_chmod_sync<P>,
    op_fs_chmod_async<P>,
    op_fs_chown_sync<P>,
    op_fs_chown_async<P>,
    op_fs_remove_sync<P>,
    op_fs_remove_async<P>,
    op_fs_copy_file_sync<P>,
    op_fs_copy_file_async<P>,
    op_fs_stat_sync<P>,
    op_fs_stat_async<P>,
    op_fs_lstat_sync<P>,
    op_fs_lstat_async<P>,
    op_fs_realpath_sync<P>,
    op_fs_realpath_async<P>,
    op_fs_read_dir_sync<P>,
    op_fs_read_dir_async<P>,
    op_fs_rename_sync<P>,
    op_fs_rename_async<P>,
    op_fs_link_sync<P>,
    op_fs_link_async<P>,
    op_fs_symlink_sync<P>,
    op_fs_symlink_async<P>,
    op_fs_read_link_sync<P>,
    op_fs_read_link_async<P>,
    op_fs_truncate_sync<P>,
    op_fs_truncate_async<P>,
    op_fs_utime_sync<P>,
    op_fs_utime_async<P>,
    op_fs_make_temp_dir_sync<P>,
    op_fs_make_temp_dir_async<P>,
    op_fs_make_temp_file_sync<P>,
    op_fs_make_temp_file_async<P>,
    op_fs_write_file_sync<P>,
    op_fs_write_file_async<P>,
    op_fs_read_file_sync<P>,
    op_fs_read_file_async<P>,
    op_fs_read_file_text_sync<P>,
    op_fs_read_file_text_async<P>,

    op_fs_seek_sync,
    op_fs_seek_async,
    op_fs_file_sync_data_sync,
    op_fs_file_sync_data_async,
    op_fs_file_sync_sync,
    op_fs_file_sync_async,
    op_fs_file_stat_sync,
    op_fs_file_stat_async,
    op_fs_fchmod_async,
    op_fs_fchmod_sync,
    op_fs_fchown_async,
    op_fs_fchown_sync,
    op_fs_flock_async,
    op_fs_flock_sync,
    op_fs_funlock_async,
    op_fs_funlock_sync,
    op_fs_ftruncate_sync,
    op_fs_file_truncate_async,
    op_fs_futime_sync,
    op_fs_futime_async,

  ],
  esm = [ "30_fs.js" ],
  options = {
    fs: FileSystemRc,
  },
  state = |state, options| {
    state.put(options.fs);
  },
);

pub trait GetPath {
  fn normalized<'a>(
    &self,
    path: Cow<'a, Path>,
  ) -> Result<(bool, Cow<'a, Path>), FsError>;
  fn resolved(&self, path: &Path) -> Result<PathBuf, FsError>;
}
