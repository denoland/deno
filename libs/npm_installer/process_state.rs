// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use serde::Deserialize;
use serde::Serialize;
use sys_traits::BoxableFsFile;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NpmProcessStateKind {
  Snapshot(deno_npm::resolution::SerializedNpmResolutionSnapshot),
  Byonm,
}

#[sys_traits::auto_impl]
pub trait NpmProcessStateFromEnvVarSys: sys_traits::FsOpen {}

/// The serialized npm process state which can be written to a file and then
/// the FD or path can be passed to a spawned deno process via the
/// `DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE_FD` env var.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub kind: NpmProcessStateKind,
  pub local_node_modules_path: Option<String>,
}

impl NpmProcessState {
  pub fn new_managed(
    snapshot: ValidSerializedNpmResolutionSnapshot,
    node_modules_path: Option<&Path>,
  ) -> Self {
    NpmProcessState {
      kind: NpmProcessStateKind::Snapshot(snapshot.into_serialized()),
      local_node_modules_path: node_modules_path
        .map(|p| p.to_string_lossy().into_owned()),
    }
  }

  pub fn new_local(
    snapshot: ValidSerializedNpmResolutionSnapshot,
    node_modules_path: &Path,
  ) -> Self {
    NpmProcessState::new_managed(snapshot, Some(node_modules_path))
  }

  pub fn from_env_var(
    sys: &impl NpmProcessStateFromEnvVarSys,
    value: OsString,
  ) -> std::io::Result<Self> {
    /// Allows for passing either a file descriptor or file path.
    enum FdOrPath {
      Fd(usize),
      Path(PathBuf),
    }

    impl FdOrPath {
      pub fn parse(value: &OsStr) -> Self {
        match value.to_string_lossy().parse::<usize>() {
          Ok(value) => FdOrPath::Fd(value),
          Err(_) => FdOrPath::Path(PathBuf::from(value)),
        }
      }

      pub fn open(
        &self,
        sys: &impl NpmProcessStateFromEnvVarSys,
      ) -> std::io::Result<sys_traits::boxed::BoxedFsFile> {
        match self {
          FdOrPath::Fd(fd) => {
            #[cfg(target_arch = "wasm32")]
            {
              let _fd = fd;
              return Err(std::io::Error::new(
                ErrorKind::Unsupported,
                "Cannot pass fd for npm process state to Wasm. Use a file path instead.",
              ));
            }
            #[cfg(all(unix, not(target_arch = "wasm32")))]
            return Ok(
              // SAFETY: Assume valid file descriptor
              unsafe {
                sys_traits::impls::RealFsFile::from_raw(
                  <std::fs::File as std::os::unix::io::FromRawFd>::from_raw_fd(
                    *fd as _,
                  ),
                )
                .into_boxed()
              },
            );
            #[cfg(windows)]
            Ok(
              // SAFETY: Assume valid file descriptor
              unsafe {
                sys_traits::impls::RealFsFile::from_raw(<std::fs::File as std::os::windows::io::FromRawHandle>::from_raw_handle(*fd as _)).into_boxed()
              },
            )
          }
          FdOrPath::Path(path) => Ok(
            sys
              .fs_open(path, &sys_traits::OpenOptions::new_read())?
              .into_boxed(),
          ),
        }
      }
    }

    let fd_or_path = FdOrPath::parse(&value);
    let mut file = fd_or_path.open(sys)?;
    let mut buf = Vec::new();
    // seek to beginning. after the file is written the position will be inherited by this subprocess,
    // and also this file might have been read before
    file.seek(std::io::SeekFrom::Start(0))?;
    file.read_to_end(&mut buf).map_err(|err| {
      std::io::Error::new(
        err.kind(),
        format!(
          "failed to reading from {}: {}",
          match fd_or_path {
            FdOrPath::Fd(fd) => format!("fd {}", fd),
            FdOrPath::Path(path) => path.display().to_string(),
          },
          err,
        ),
      )
    })?;
    let state: NpmProcessState =
      serde_json::from_slice(&buf).map_err(|err| {
        std::io::Error::new(
          ErrorKind::InvalidData,
          format!(
            "failed to deserialize npm process state: {}\n{}",
            err,
            String::from_utf8_lossy(&buf)
          ),
        )
      })?;
    Ok(state)
  }

  pub fn as_serialized(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}
