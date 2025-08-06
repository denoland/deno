// Copyright 2018-2025 the Deno authors. MIT license.

mod interface;
mod ops;
mod std_fs;
pub mod sync;

use std::borrow::Cow;
use std::path::Path;

pub use deno_io::fs::FsError;
use deno_permissions::CheckedPath;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionCheckError;

pub use crate::interface::FileSystem;
pub use crate::interface::FileSystemRc;
pub use crate::interface::FsDirEntry;
pub use crate::interface::FsFileType;
pub use crate::interface::OpenOptions;
pub use crate::ops::FsOpsError;
pub use crate::ops::FsOpsErrorKind;
pub use crate::ops::OperationError;
use crate::ops::*;
pub use crate::std_fs::RealFs;
pub use crate::std_fs::open_options_for_checked_path;
pub use crate::sync::MaybeSend;
pub use crate::sync::MaybeSync;

pub trait FsPermissions {
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_open<'a>(
    &self,
    path: Cow<'a, Path>,
    access_kind: OpenAccessKind,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_open_blind<'a>(
    &self,
    path: Cow<'a, Path>,
    access_kind: OpenAccessKind,
    display: &str,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError>;
  fn check_read_all(&self, api_name: &str) -> Result<(), PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write_partial<'a>(
    &self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError>;
  fn check_write_all(&self, api_name: &str)
  -> Result<(), PermissionCheckError>;

  fn allows_all(&self) -> bool {
    false
  }
}

impl FsPermissions for deno_permissions::PermissionsContainer {
  fn check_open<'a>(
    &self,
    path: Cow<'a, Path>,
    access_kind: OpenAccessKind,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_open(
      self,
      path,
      access_kind,
      Some(api_name),
    )
  }

  fn check_open_blind<'a>(
    &self,
    path: Cow<'a, Path>,
    access_kind: OpenAccessKind,
    display: &str,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_open_blind(
      self,
      path,
      access_kind,
      display,
      Some(api_name),
    )
  }

  fn check_write_partial<'a>(
    &self,
    path: Cow<'a, Path>,
    api_name: &str,
  ) -> Result<CheckedPath<'a>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_partial(
      self, path, api_name,
    )
  }

  fn check_read_all(&self, api_name: &str) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read_all(self, api_name)
  }

  fn check_write_all(
    &self,
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
    op_fs_cwd,
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
