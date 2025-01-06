// Copyright 2018-2025 the Deno authors. MIT license.

// todo(dsherret): this should instead use conditional compilation and directly
// surface the underlying implementation.
//
// The problem atm is that there's no way to have conditional compilation for
// denort or the deno binary. We should extract out denort to a separate binary.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::boxed::BoxedFsDirEntry;
use sys_traits::boxed::BoxedFsFile;
use sys_traits::boxed::BoxedFsMetadataValue;
use sys_traits::boxed::FsMetadataBoxed;
use sys_traits::boxed::FsOpenBoxed;
use sys_traits::boxed::FsReadDirBoxed;
use sys_traits::CreateDirOptions;

use crate::standalone::DenoCompileFileSystem;

#[derive(Debug, Clone)]
pub enum CliSys {
  #[allow(dead_code)] // will be dead code for denort
  #[allow(clippy::disallowed_types)] // ok because sys impl
  Real(sys_traits::impls::RealSys),
  #[allow(dead_code)] // will be dead code for deno
  DenoCompile(DenoCompileFileSystem),
}

impl Default for CliSys {
  fn default() -> Self {
    Self::Real(sys_traits::impls::RealSys)
  }
}

impl deno_runtime::deno_node::ExtNodeSys for CliSys {}

impl sys_traits::BaseFsCloneFile for CliSys {
  fn base_fs_clone_file(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_clone_file(src, dst),
      Self::DenoCompile(sys) => sys.base_fs_clone_file(src, dst),
    }
  }
}

impl sys_traits::BaseFsSymlinkDir for CliSys {
  fn base_fs_symlink_dir(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_symlink_dir(src, dst),
      Self::DenoCompile(sys) => sys.base_fs_symlink_dir(src, dst),
    }
  }
}

impl sys_traits::BaseFsCopy for CliSys {
  fn base_fs_copy(&self, src: &Path, dst: &Path) -> std::io::Result<u64> {
    match self {
      Self::Real(sys) => sys.base_fs_copy(src, dst),
      Self::DenoCompile(sys) => sys.base_fs_copy(src, dst),
    }
  }
}

impl sys_traits::BaseFsHardLink for CliSys {
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_hard_link(src, dst),
      Self::DenoCompile(sys) => sys.base_fs_hard_link(src, dst),
    }
  }
}

impl sys_traits::BaseFsRead for CliSys {
  fn base_fs_read(&self, p: &Path) -> std::io::Result<Cow<'static, [u8]>> {
    match self {
      Self::Real(sys) => sys.base_fs_read(p),
      Self::DenoCompile(sys) => sys.base_fs_read(p),
    }
  }
}

impl sys_traits::BaseFsReadDir for CliSys {
  type ReadDirEntry = BoxedFsDirEntry;

  fn base_fs_read_dir(
    &self,
    p: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>> + '_>,
  > {
    match self {
      Self::Real(sys) => sys.fs_read_dir_boxed(p),
      Self::DenoCompile(sys) => sys.fs_read_dir_boxed(p),
    }
  }
}

impl sys_traits::BaseFsCanonicalize for CliSys {
  fn base_fs_canonicalize(&self, p: &Path) -> std::io::Result<PathBuf> {
    match self {
      Self::Real(sys) => sys.base_fs_canonicalize(p),
      Self::DenoCompile(sys) => sys.base_fs_canonicalize(p),
    }
  }
}

impl sys_traits::BaseFsMetadata for CliSys {
  type Metadata = BoxedFsMetadataValue;

  fn base_fs_metadata(&self, path: &Path) -> std::io::Result<Self::Metadata> {
    match self {
      Self::Real(sys) => sys.fs_metadata_boxed(path),
      Self::DenoCompile(sys) => sys.fs_metadata_boxed(path),
    }
  }

  fn base_fs_symlink_metadata(
    &self,
    path: &Path,
  ) -> std::io::Result<Self::Metadata> {
    match self {
      Self::Real(sys) => sys.fs_symlink_metadata_boxed(path),
      Self::DenoCompile(sys) => sys.fs_symlink_metadata_boxed(path),
    }
  }
}

impl sys_traits::BaseFsCreateDir for CliSys {
  fn base_fs_create_dir(
    &self,
    p: &Path,
    options: &CreateDirOptions,
  ) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_create_dir(p, options),
      Self::DenoCompile(sys) => sys.base_fs_create_dir(p, options),
    }
  }
}

impl sys_traits::BaseFsOpen for CliSys {
  type File = BoxedFsFile;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &sys_traits::OpenOptions,
  ) -> std::io::Result<Self::File> {
    match self {
      Self::Real(sys) => sys.fs_open_boxed(path, options),
      Self::DenoCompile(sys) => sys.fs_open_boxed(path, options),
    }
  }
}

impl sys_traits::BaseFsRemoveFile for CliSys {
  fn base_fs_remove_file(&self, p: &Path) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_remove_file(p),
      Self::DenoCompile(sys) => sys.base_fs_remove_file(p),
    }
  }
}

impl sys_traits::BaseFsRename for CliSys {
  fn base_fs_rename(&self, old: &Path, new: &Path) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.base_fs_rename(old, new),
      Self::DenoCompile(sys) => sys.base_fs_rename(old, new),
    }
  }
}

impl sys_traits::SystemRandom for CliSys {
  fn sys_random(&self, buf: &mut [u8]) -> std::io::Result<()> {
    match self {
      Self::Real(sys) => sys.sys_random(buf),
      Self::DenoCompile(sys) => sys.sys_random(buf),
    }
  }
}

impl sys_traits::SystemTimeNow for CliSys {
  fn sys_time_now(&self) -> std::time::SystemTime {
    match self {
      Self::Real(sys) => sys.sys_time_now(),
      Self::DenoCompile(sys) => sys.sys_time_now(),
    }
  }
}

impl sys_traits::ThreadSleep for CliSys {
  fn thread_sleep(&self, dur: std::time::Duration) {
    match self {
      Self::Real(sys) => sys.thread_sleep(dur),
      Self::DenoCompile(sys) => sys.thread_sleep(dur),
    }
  }
}

impl sys_traits::EnvCurrentDir for CliSys {
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    match self {
      Self::Real(sys) => sys.env_current_dir(),
      Self::DenoCompile(sys) => sys.env_current_dir(),
    }
  }
}

impl sys_traits::BaseEnvVar for CliSys {
  fn base_env_var_os(
    &self,
    key: &std::ffi::OsStr,
  ) -> Option<std::ffi::OsString> {
    match self {
      Self::Real(sys) => sys.base_env_var_os(key),
      Self::DenoCompile(sys) => sys.base_env_var_os(key),
    }
  }
}

impl sys_traits::EnvHomeDir for CliSys {
  fn env_home_dir(&self) -> Option<PathBuf> {
    #[allow(clippy::disallowed_types)] // ok because sys impl
    sys_traits::impls::RealSys.env_home_dir()
  }
}
