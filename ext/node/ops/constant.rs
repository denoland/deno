// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// https://github.com/nodejs/node/blob/ce4a16f50ae289bf6c7834b592ca47ad4634dd79/src/node_constants.cc#L1044-L1225
pub struct FsConstants {
  uv_fs_symlink_dir: i32,
  uv_fs_symlink_junction: i32,
  o_rdonly: i32,
  o_wronly: i32,
  o_rdwr: i32,
  uv_dirent_unknown: i32,
  uv_dirent_file: i32,
  uv_dirent_dir: i32,
  uv_dirent_link: i32,
  uv_dirent_fifo: i32,
  uv_dirent_socket: i32,
  uv_dirent_char: i32,
  uv_dirent_block: i32,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifmt: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifreg: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifdir: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifchr: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifblk: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ififo: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_iflnk: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ifsock: Option<i32>,
  o_creat: i32,
  o_excl: i32,
  uv_fs_o_filemap: i32,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_noctty: Option<i32>,
  o_trunc: i32,
  o_append: i32,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_directory: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_noatime: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_nofollow: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_sync: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_dsync: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_symlink: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_direct: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o_nonblock: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_irwxu: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_irusr: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_iwusr: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ixusr: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_irwxg: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_irgrp: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_iwgrp: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ixgrp: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_irwxo: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_iroth: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_iwoth: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  s_ixoth: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  f_ok: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  r_ok: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  w_ok: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  x_ok: Option<i32>,
  uv_fs_copyfile_excl: i32,
  copyfile_excl: i32,
  uv_fs_copyfile_ficlone: i32,
  copyfile_ficlone: i32,
  uv_fs_copyfile_ficlone_force: i32,
  copyfile_ficlone_force: i32,
}

// https://github.com/nodejs/node/blob/9d2368f64329bf194c4e82b349e76fdad879d32a/deps/uv/include/uv.h#L1616-L1626
pub const UV_FS_SYMLINK_DIR: i32 = 1;
pub const UV_FS_SYMLINK_JUNCTION: i32 = 2;

// https://github.com/nodejs/node/blob/9d2368f64329bf194c4e82b349e76fdad879d32a/deps/uv/include/uv.h#L1254-L1263
pub const UV_DIRENT_UNKNOWN: i32 = 0;
pub const UV_DIRENT_FILE: i32 = 1;
pub const UV_DIRENT_DIR: i32 = 2;
pub const UV_DIRENT_LINK: i32 = 3;
pub const UV_DIRENT_FIFO: i32 = 4;
pub const UV_DIRENT_SOCKET: i32 = 5;
pub const UV_DIRENT_CHAR: i32 = 6;
pub const UV_DIRENT_BLOCK: i32 = 7;

// https://github.com/nodejs/node/blob/9d2368f64329bf194c4e82b349e76fdad879d32a/deps/uv/include/uv.h#L1485-L1501
pub const UV_FS_COPYFILE_EXCL: i32 = 1;
pub const UV_FS_COPYFILE_FICLONE: i32 = 2;
pub const UV_FS_COPYFILE_FICLONE_FORCE: i32 = 4;

// https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/include/uv/errno.h#L35-L48
pub const UV_EAI_ADDRFAMILY: i32 = -3000;
pub const UV_EAI_AGAIN: i32 = -3001;
pub const UV_EAI_BADFLAGS: i32 = -3002;
pub const UV_EAI_CANCELED: i32 = -3003;
pub const UV_EAI_FAIL: i32 = -3004;
pub const UV_EAI_FAMILY: i32 = -3005;
pub const UV_EAI_MEMORY: i32 = -3006;
pub const UV_EAI_NODATA: i32 = -3007;
pub const UV_EAI_NONAME: i32 = -3008;
pub const UV_EAI_OVERFLOW: i32 = -3009;
pub const UV_EAI_SERVICE: i32 = -3010;
pub const UV_EAI_SOCKTYPE: i32 = -3011;
pub const UV_EAI_BADHINTS: i32 = -3013;
pub const UV_EAI_PROTOCOL: i32 = -3014;

#[cfg(unix)]
/// https://github.com/nodejs/node/blob/9d2368f64329bf194c4e82b349e76fdad879d32a/deps/uv/include/uv/unix.h#L506
pub const UV_FS_O_FILEMAP: i32 = 0;
#[cfg(not(unix))]
/// https://github.com/nodejs/node/blob/4dafa7747f7d2804aed3f3400d04f1ec6af24160/deps/uv/include/uv/win.h#L678
pub const UV_FS_O_FILEMAP: i32 = 0x20000000;

impl Default for FsConstants {
  fn default() -> Self {
    FsConstants {
      uv_fs_symlink_dir: UV_FS_SYMLINK_DIR,
      uv_fs_symlink_junction: UV_FS_SYMLINK_JUNCTION,
      o_rdonly: libc::O_RDONLY,
      o_wronly: libc::O_WRONLY,
      o_rdwr: libc::O_RDWR,
      uv_dirent_unknown: UV_DIRENT_UNKNOWN,
      uv_dirent_file: UV_DIRENT_FILE,
      uv_dirent_dir: UV_DIRENT_DIR,
      uv_dirent_link: UV_DIRENT_LINK,
      uv_dirent_fifo: UV_DIRENT_FIFO,
      uv_dirent_socket: UV_DIRENT_SOCKET,
      uv_dirent_char: UV_DIRENT_CHAR,
      uv_dirent_block: UV_DIRENT_BLOCK,
      s_ifmt: None,
      s_ifreg: None,
      s_ifdir: None,
      s_ifchr: None,
      s_ifblk: None,
      s_ififo: None,
      s_iflnk: None,
      s_ifsock: None,
      o_creat: libc::O_CREAT,
      o_excl: libc::O_EXCL,
      uv_fs_o_filemap: UV_FS_O_FILEMAP,
      o_noctty: None,
      o_trunc: libc::O_TRUNC,
      o_append: libc::O_APPEND,
      o_directory: None,
      o_noatime: None,
      o_nofollow: None,
      o_sync: None,
      o_dsync: None,
      o_symlink: None,
      o_direct: None,
      o_nonblock: None,
      s_irwxu: None,
      s_irusr: None,
      s_iwusr: None,
      s_ixusr: None,
      s_irwxg: None,
      s_irgrp: None,
      s_iwgrp: None,
      s_ixgrp: None,
      s_irwxo: None,
      s_iroth: None,
      s_iwoth: None,
      s_ixoth: None,
      f_ok: None,
      r_ok: None,
      w_ok: None,
      x_ok: None,
      uv_fs_copyfile_excl: UV_FS_COPYFILE_EXCL,
      copyfile_excl: UV_FS_COPYFILE_EXCL,
      uv_fs_copyfile_ficlone: UV_FS_COPYFILE_FICLONE,
      copyfile_ficlone: UV_FS_COPYFILE_FICLONE,
      uv_fs_copyfile_ficlone_force: UV_FS_COPYFILE_FICLONE_FORCE,
      copyfile_ficlone_force: UV_FS_COPYFILE_FICLONE_FORCE,
    }
  }
}

#[cfg(unix)]
fn common_unix_fs_constants() -> FsConstants {
  FsConstants {
    s_ifmt: Some(libc::S_IFMT as i32),
    s_ifreg: Some(libc::S_IFREG as i32),
    s_ifdir: Some(libc::S_IFDIR as i32),
    s_ifchr: Some(libc::S_IFCHR as i32),
    s_ifblk: Some(libc::S_IFBLK as i32),
    s_ififo: Some(libc::S_IFIFO as i32),
    s_iflnk: Some(libc::S_IFLNK as i32),
    s_ifsock: Some(libc::S_IFSOCK as i32),
    o_noctty: Some(libc::O_NOCTTY),
    o_directory: Some(libc::O_DIRECTORY),
    o_nofollow: Some(libc::O_NOFOLLOW),
    o_sync: Some(libc::O_SYNC),
    o_dsync: Some(libc::O_DSYNC),
    o_nonblock: Some(libc::O_NONBLOCK),
    s_irwxu: Some(libc::S_IRWXU as i32),
    s_irusr: Some(libc::S_IRUSR as i32),
    s_iwusr: Some(libc::S_IWUSR as i32),
    s_ixusr: Some(libc::S_IXUSR as i32),
    s_irwxg: Some(libc::S_IRWXG as i32),
    s_irgrp: Some(libc::S_IRGRP as i32),
    s_iwgrp: Some(libc::S_IWGRP as i32),
    s_ixgrp: Some(libc::S_IXGRP as i32),
    s_irwxo: Some(libc::S_IRWXO as i32),
    s_iroth: Some(libc::S_IROTH as i32),
    s_iwoth: Some(libc::S_IWOTH as i32),
    s_ixoth: Some(libc::S_IXOTH as i32),
    f_ok: Some(libc::F_OK),
    r_ok: Some(libc::R_OK),
    w_ok: Some(libc::W_OK),
    x_ok: Some(libc::X_OK),
    ..Default::default()
  }
}

#[cfg(target_os = "macos")]
#[op2]
#[serde]
pub fn op_node_fs_constants() -> FsConstants {
  let mut constants = common_unix_fs_constants();
  constants.o_symlink = Some(libc::O_SYMLINK);
  constants
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[op2]
#[serde]
pub fn op_node_fs_constants() -> FsConstants {
  let mut constants = common_unix_fs_constants();
  constants.o_noatime = Some(libc::O_NOATIME);
  constants.o_direct = Some(libc::O_DIRECT);
  constants
}

#[cfg(windows)]
#[op2]
#[serde]
pub fn op_node_fs_constants() -> FsConstants {
  let mut constants = FsConstants::default();
  // https://github.com/nodejs/node/blob/4dafa7747f7d2804aed3f3400d04f1ec6af24160/deps/uv/include/uv/win.h#L65-L68
  // https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_stat_lx_information
  const S_IFIFO: i32 = 0x1000;
  // https://github.com/nodejs/node/blob/4dafa7747f7d2804aed3f3400d04f1ec6af24160/deps/uv/include/uv/win.h#L61-L63
  const S_IFLNK: i32 = 0xA000;
  // https://github.com/nodejs/node/blob/4dafa7747f7d2804aed3f3400d04f1ec6af24160/deps/uv/include/uv/win.h#L661-L672
  const F_OK: i32 = 0;
  const R_OK: i32 = 4;
  const W_OK: i32 = 2;
  const X_OK: i32 = 1;

  // https://github.com/nodejs/node/blob/9d2368f64329bf194c4e82b349e76fdad879d32a/src/node_constants.cc#L52-L57
  constants.s_irusr = Some(libc::S_IREAD);
  constants.s_iwusr = Some(libc::S_IWRITE);

  constants.s_ifmt = Some(libc::S_IFMT);
  constants.s_ifreg = Some(libc::S_IFREG);
  constants.s_ifdir = Some(libc::S_IFDIR);
  constants.s_ifchr = Some(libc::S_IFCHR);
  constants.f_ok = Some(F_OK);
  constants.r_ok = Some(R_OK);
  constants.w_ok = Some(W_OK);
  constants.x_ok = Some(X_OK);
  constants.s_iflnk = Some(S_IFLNK);
  constants.s_ififo = Some(S_IFIFO);
  constants
}
