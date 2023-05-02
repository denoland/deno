// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod interface;
mod ops;
mod std_fs;

pub use crate::interface::FileSystem;
pub use crate::interface::FsDirEntry;
pub use crate::interface::FsFileType;
pub use crate::interface::OpenOptions;
use crate::ops::*;

pub use crate::std_fs::RealFs;

use deno_core::error::AnyError;
use deno_core::OpState;
use std::cell::RefCell;
use std::convert::From;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

pub trait FsPermissions {
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_read_all(&mut self, api_name: &str) -> Result<(), AnyError>;
  fn check_read_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError>;
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_write_all(&mut self, api_name: &str) -> Result<(), AnyError>;
  fn check_write_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError>;

  fn check(
    &mut self,
    open_options: &OpenOptions,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    if open_options.read {
      self.check_read(path, api_name)?;
    }
    if open_options.write || open_options.append {
      self.check_write(path, api_name)?;
    }
    Ok(())
  }
}

struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{api_name}'. The --unstable flag must be provided."
      );
      std::process::exit(70);
    }
  }
}

/// Helper for checking unstable features. Used for sync ops.
pub(crate) fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
pub(crate) fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

deno_core::extension!(deno_fs,
  deps = [ deno_web ],
  parameters = [P: FsPermissions],
  ops = [
    op_cwd<P>,
    op_umask,
    op_chdir<P>,

    op_open_sync<P>,
    op_open_async<P>,
    op_mkdir_sync<P>,
    op_mkdir_async<P>,
    op_chmod_sync<P>,
    op_chmod_async<P>,
    op_chown_sync<P>,
    op_chown_async<P>,
    op_remove_sync<P>,
    op_remove_async<P>,
    op_copy_file_sync<P>,
    op_copy_file_async<P>,
    op_stat_sync<P>,
    op_stat_async<P>,
    op_lstat_sync<P>,
    op_lstat_async<P>,
    op_realpath_sync<P>,
    op_realpath_async<P>,
    op_read_dir_sync<P>,
    op_read_dir_async<P>,
    op_rename_sync<P>,
    op_rename_async<P>,
    op_link_sync<P>,
    op_link_async<P>,
    op_symlink_sync<P>,
    op_symlink_async<P>,
    op_read_link_sync<P>,
    op_read_link_async<P>,
    op_truncate_sync<P>,
    op_truncate_async<P>,
    op_utime_sync<P>,
    op_utime_async<P>,
    op_make_temp_dir_sync<P>,
    op_make_temp_dir_async<P>,
    op_make_temp_file_sync<P>,
    op_make_temp_file_async<P>,
    op_write_file_sync<P>,
    op_write_file_async<P>,
    op_read_file_sync<P>,
    op_read_file_async<P>,
    op_read_file_text_sync<P>,
    op_read_file_text_async<P>,

    op_seek_sync,
    op_seek_async,
    op_fdatasync_sync,
    op_fdatasync_async,
    op_fsync_sync,
    op_fsync_async,
    op_fstat_sync,
    op_fstat_async,
    op_flock_sync,
    op_flock_async,
    op_funlock_sync,
    op_funlock_async,
    op_ftruncate_sync,
    op_ftruncate_async,
    op_futime_sync,
    op_futime_async,

  ],
  esm = [ "30_fs.js" ],
  options = {
    unstable: bool,
    fs: Option<Arc<dyn FileSystem>>,
  },
  state = |state, options| {
    state.put(UnstableChecker { unstable: options.unstable });
    state.put(options.fs.unwrap_or_else(|| Arc::new(RealFs)));
  },
);
