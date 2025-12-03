// Copyright 2018-2025 the Deno authors. MIT license.

mod interface;
mod ops;
mod std_fs;

pub use deno_io::fs::FsError;
pub use deno_maybe_sync as sync;
pub use deno_maybe_sync::MaybeSend;
pub use deno_maybe_sync::MaybeSync;

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

pub const UNSTABLE_FEATURE_NAME: &str = "fs";

deno_core::extension!(deno_fs,
  deps = [ deno_web ],
  ops = [
    op_fs_cwd,
    op_fs_umask,
    op_fs_chdir,

    op_fs_open_sync,
    op_fs_open_async,
    op_fs_mkdir_sync,
    op_fs_mkdir_async,
    op_fs_chmod_sync,
    op_fs_chmod_async,
    op_fs_chown_sync,
    op_fs_chown_async,
    op_fs_remove_sync,
    op_fs_remove_async,
    op_fs_copy_file_sync,
    op_fs_copy_file_async,
    op_fs_stat_sync,
    op_fs_stat_async,
    op_fs_lstat_sync,
    op_fs_lstat_async,
    op_fs_realpath_sync,
    op_fs_realpath_async,
    op_fs_read_dir_sync,
    op_fs_read_dir_async,
    op_fs_rename_sync,
    op_fs_rename_async,
    op_fs_link_sync,
    op_fs_link_async,
    op_fs_symlink_sync,
    op_fs_symlink_async,
    op_fs_read_link_sync,
    op_fs_read_link_async,
    op_fs_truncate_sync,
    op_fs_truncate_async,
    op_fs_utime_sync,
    op_fs_utime_async,
    op_fs_make_temp_dir_sync,
    op_fs_make_temp_dir_async,
    op_fs_make_temp_file_sync,
    op_fs_make_temp_file_async,
    op_fs_write_file_sync,
    op_fs_write_file_async,
    op_fs_read_file_sync,
    op_fs_read_file_async,
    op_fs_read_file_text_sync,
    op_fs_read_file_text_async,

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
