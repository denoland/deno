// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::fs::File;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LoadError {
  #[class(generic)]
  #[error("Failed to write native addon (Deno FFI/Node API) '{0}' to '{1}' because the file system was readonly. This is a limitation of native addons with deno compile.", executable_path.display(), real_path.display())]
  ReadOnlyFilesystem {
    real_path: PathBuf,
    executable_path: PathBuf,
  },
  #[class(generic)]
  #[error("Failed to write native addon (Deno FFI/Node API) '{0}' to '{1}'.", executable_path.display(), real_path.display())]
  FailedWriting {
    real_path: PathBuf,
    executable_path: PathBuf,
    #[source]
    source: std::io::Error,
  },
}

pub type DenoRtNativeAddonLoaderRc = Arc<dyn DenoRtNativeAddonLoader>;

/// Loads native addons in `deno compile`.
///
/// The implementation should provide the bytes from the binary
/// of the native file.
pub trait DenoRtNativeAddonLoader: Send + Sync {
  fn load_if_in_vfs(&self, path: &Path) -> Option<Cow<'static, [u8]>>;

  fn load_and_resolve_path<'a>(
    &self,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, LoadError> {
    match self.load_if_in_vfs(path) {
      Some(bytes) => {
        let exe_name = std::env::current_exe().ok();
        let exe_name = exe_name
          .as_ref()
          .and_then(|p| p.file_stem())
          .map(|s| s.to_string_lossy())
          .unwrap_or("denort".into());
        let real_path = resolve_temp_file_name(&exe_name, path, &bytes);
        if let Err(err) = deno_path_util::fs::atomic_write_file(
          &sys_traits::impls::RealSys,
          &real_path,
          &bytes,
          0o644,
        ) {
          if err.kind() == std::io::ErrorKind::ReadOnlyFilesystem {
            return Err(LoadError::ReadOnlyFilesystem {
              real_path,
              executable_path: path.to_path_buf(),
            });
          }

          // another process might be using it... so only surface
          // the error if the files aren't equivalent
          if !file_matches_bytes(&real_path, &bytes) {
            return Err(LoadError::FailedWriting {
              executable_path: path.to_path_buf(),
              real_path,
              source: err,
            });
          }
        }
        Ok(Cow::Owned(real_path))
      }
      None => Ok(Cow::Borrowed(path)),
    }
  }
}

fn file_matches_bytes(path: &Path, expected_bytes: &[u8]) -> bool {
  let file = match File::open(path) {
    Ok(f) => f,
    Err(_) => return false,
  };
  let len_on_disk = match file.metadata() {
    Ok(m) => m.len(),
    Err(_) => return false,
  };
  if len_on_disk as usize != expected_bytes.len() {
    return false; // bail early
  }

  // Stream‑compare in fixed‑size chunks.
  const CHUNK: usize = 8 * 1024;
  let mut reader = BufReader::with_capacity(CHUNK, file);
  let mut buf = [0u8; CHUNK];
  let mut offset = 0;

  loop {
    match reader.read(&mut buf) {
      Ok(0) => return offset == expected_bytes.len(),
      Ok(n) => {
        let next_offset = offset + n;
        if next_offset > expected_bytes.len()
          || buf[..n] != expected_bytes[offset..next_offset]
        {
          return false;
        }
        offset = next_offset;
      }
      Err(_) => return false,
    }
  }
}

fn resolve_temp_file_name(
  current_exe_name: &str,
  path: &Path,
  bytes: &[u8],
) -> PathBuf {
  // should be deterministic
  let path_hash = {
    let mut hasher = twox_hash::XxHash64::default();
    path.hash(&mut hasher);
    hasher.finish()
  };
  let bytes_hash = {
    let mut hasher = twox_hash::XxHash64::default();
    bytes.hash(&mut hasher);
    hasher.finish()
  };
  let mut file_name =
    format!("{}{}{}", current_exe_name, path_hash, bytes_hash);
  if let Some(ext) = path.extension() {
    file_name.push('.');
    file_name.push_str(&ext.to_string_lossy());
  }
  std::env::temp_dir().join(&file_name)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_file_matches_bytes() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let path = tempdir.path().join("file.txt");
    let mut bytes = vec![0u8; 17892];
    for (i, byte) in bytes.iter_mut().enumerate() {
      *byte = i as u8;
    }
    std::fs::write(&path, &bytes).unwrap();
    assert!(file_matches_bytes(&path, &bytes));
    bytes[17192] = 9;
    assert!(!file_matches_bytes(&path, &bytes));
  }

  #[test]
  fn test_resolve_temp_file_name() {
    let file_path = PathBuf::from("/test/test.node");
    let bytes: [u8; 3] = [1, 2, 3];
    let temp_file = resolve_temp_file_name("exe_name", &file_path, &bytes);
    assert_eq!(
      temp_file,
      std::env::temp_dir()
        .join("exe_name1805603793990095570513255480333703631005.node")
    );
  }
}
