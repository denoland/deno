// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod interface;
mod ops;
mod std_fs;

pub use crate::interface::File;
pub use crate::interface::FileSystem;
pub use crate::interface::FsDirEntry;
pub use crate::interface::FsError;
pub use crate::interface::FsFileType;
pub use crate::interface::FsPermissions;
pub use crate::interface::FsResult;
pub use crate::interface::FsStat;
pub use crate::interface::OpenOptions;
use crate::ops::*;

pub use crate::std_fs::StdFs;

use deno_core::OpState;
use std::cell::RefCell;
use std::convert::From;
use std::rc::Rc;

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
  parameters = [Fs: FileSystem, P: FsPermissions],
  ops = [
    op_cwd<Fs, P>,
    op_umask<Fs>,
    op_chdir<Fs, P>,

    op_open_sync<Fs, P>,
    op_open_async<Fs, P>,
    op_mkdir_sync<Fs, P>,
    op_mkdir_async<Fs, P>,
    op_chmod_sync<Fs, P>,
    op_chmod_async<Fs, P>,
    op_chown_sync<Fs, P>,
    op_chown_async<Fs, P>,
    op_remove_sync<Fs, P>,
    op_remove_async<Fs, P>,
    op_copy_file_sync<Fs, P>,
    op_copy_file_async<Fs, P>,
    op_stat_sync<Fs, P>,
    op_stat_async<Fs, P>,
    op_lstat_sync<Fs, P>,
    op_lstat_async<Fs, P>,
    op_realpath_sync<Fs, P>,
    op_realpath_async<Fs, P>,
    op_read_dir_sync<Fs, P>,
    op_read_dir_async<Fs, P>,
    op_rename_sync<Fs, P>,
    op_rename_async<Fs, P>,
    op_link_sync<Fs, P>,
    op_link_async<Fs, P>,
    op_symlink_sync<Fs, P>,
    op_symlink_async<Fs, P>,
    op_read_link_sync<Fs, P>,
    op_read_link_async<Fs, P>,
    op_truncate_sync<Fs, P>,
    op_truncate_async<Fs, P>,
    op_utime_sync<Fs, P>,
    op_utime_async<Fs, P>,
    op_make_temp_dir_sync<Fs, P>,
    op_make_temp_dir_async<Fs, P>,
    op_make_temp_file_sync<Fs, P>,
    op_make_temp_file_async<Fs, P>,
    op_write_file_sync<Fs, P>,
    op_write_file_async<Fs, P>,
    op_read_file_sync<Fs, P>,
    op_read_file_async<Fs, P>,
    op_read_file_text_sync<Fs, P>,
    op_read_file_text_async<Fs, P>,

    op_seek_sync<Fs>,
    op_seek_async<Fs>,
    op_fdatasync_sync<Fs>,
    op_fdatasync_async<Fs>,
    op_fsync_sync<Fs>,
    op_fsync_async<Fs>,
    op_fstat_sync<Fs>,
    op_fstat_async<Fs>,
    op_flock_sync<Fs>,
    op_flock_async<Fs>,
    op_funlock_sync<Fs>,
    op_funlock_async<Fs>,
    op_ftruncate_sync<Fs>,
    op_ftruncate_async<Fs>,
    op_futime_sync<Fs>,
    op_futime_async<Fs>,

  ],
  esm = [ "30_fs.js" ],
  options = {
    unstable: bool,
    fs: Fs,
  },
  state = |state, options| {
    state.put(UnstableChecker { unstable: options.unstable });
    state.put(options.fs);
  },
);
