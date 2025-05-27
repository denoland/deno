// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsString;
use std::path::PathBuf;

use which::sys::Sys;
pub use which::Error;

pub fn which_in(
  sys: impl WhichSys,
  binary_name: &str,
  path: Option<OsString>,
  cwd: PathBuf,
) -> Result<PathBuf, which::Error> {
  let sys = WhichSysAdapter(sys);
  let config = which::WhichConfig::new_with_sys(sys)
    .custom_cwd(cwd)
    .binary_name(OsString::from(binary_name));
  let config = match path {
    Some(path) => config.custom_path_list(path),
    None => config,
  };
  config.first_result()
}

#[sys_traits::auto_impl]
pub trait WhichSys:
  sys_traits::EnvHomeDir
  + sys_traits::EnvCurrentDir
  + sys_traits::EnvVar
  + sys_traits::FsReadDir
  + sys_traits::FsMetadata
  + Clone
  + 'static
{
}

#[derive(Clone)]
pub struct WhichSysAdapter<TSys: WhichSys>(TSys);

impl<TSys: WhichSys> Sys for WhichSysAdapter<TSys> {
  type ReadDirEntry = WhichReadDirEntrySysAdapter<TSys::ReadDirEntry>;

  type Metadata = WhichMetadataSysAdapter<TSys::Metadata>;

  fn is_windows(&self) -> bool {
    sys_traits::impls::is_windows()
  }

  fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
    self.0.env_current_dir()
  }

  fn home_dir(&self) -> Option<std::path::PathBuf> {
    self.0.env_home_dir()
  }

  fn env_split_paths(
    &self,
    paths: &std::ffi::OsStr,
  ) -> Vec<std::path::PathBuf> {
    if cfg!(target_arch = "wasm32") && self.is_windows() {
      // not perfect, but good enough
      paths
        .to_string_lossy()
        .split(";")
        .map(PathBuf::from)
        .collect()
    } else {
      std::env::split_paths(paths).collect()
    }
  }

  fn env_var_os(&self, name: &str) -> Option<std::ffi::OsString> {
    self.0.env_var_os(name)
  }

  fn metadata(
    &self,
    path: &std::path::Path,
  ) -> std::io::Result<Self::Metadata> {
    self.0.fs_metadata(path).map(WhichMetadataSysAdapter)
  }

  fn symlink_metadata(
    &self,
    path: &std::path::Path,
  ) -> std::io::Result<Self::Metadata> {
    self
      .0
      .fs_symlink_metadata(path)
      .map(WhichMetadataSysAdapter)
  }

  fn read_dir(
    &self,
    path: &std::path::Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>> + '_>,
  > {
    let iter = self.0.fs_read_dir(path)?;
    let test = Box::new(
      iter
        .into_iter()
        .map(|value| value.map(WhichReadDirEntrySysAdapter)),
    );
    Ok(test)
  }

  #[cfg(not(windows))]
  fn is_valid_executable(
    &self,
    path: &std::path::Path,
  ) -> std::io::Result<bool> {
    todo!()
  }

  #[cfg(windows)]
  fn is_valid_executable(
    &self,
    path: &std::path::Path,
  ) -> std::io::Result<bool> {
    use std::os::windows::ffi::OsStrExt;

    let name = path
      .as_os_str()
      .encode_wide()
      .chain(Some(0))
      .collect::<Vec<u16>>();
    let mut bt: winapi::shared::minwindef::DWORD = 0;
    // SAFETY: winapi call
    unsafe {
      Ok(
        windows_sys::Win32::Storage::FileSystem::GetBinaryTypeW(
          name.as_ptr(),
          &mut bt,
        ) != 0,
      )
    }
  }
}

pub struct WhichReadDirEntrySysAdapter<TFsDirEntry: sys_traits::FsDirEntry>(
  TFsDirEntry,
);

impl<TFsDirEntry: sys_traits::FsDirEntry> which::sys::SysReadDirEntry
  for WhichReadDirEntrySysAdapter<TFsDirEntry>
{
  fn file_name(&self) -> std::ffi::OsString {
    self.0.file_name().into_owned()
  }

  fn path(&self) -> std::path::PathBuf {
    self.0.path().into_owned()
  }
}

pub struct WhichMetadataSysAdapter<TMetadata: sys_traits::FsMetadataValue>(
  TMetadata,
);

impl<TMetadata: sys_traits::FsMetadataValue> which::sys::SysMetadata
  for WhichMetadataSysAdapter<TMetadata>
{
  fn is_symlink(&self) -> bool {
    self.0.file_type().is_symlink()
  }

  fn is_file(&self) -> bool {
    self.0.file_type().is_file()
  }
}
