// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::fs;
use std::fs::File;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::BufReader;
use std::io::ErrorKind;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use sys_traits::EnvTempDir;

const NATIVE_ADDON_CACHE_DIR_NAME: &str = "deno-native-addons";

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
  #[class(inherit)]
  #[error(transparent)]
  Io(std::io::Error),
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
        let cache_dir = native_addon_cache_dir().map_err(LoadError::Io)?;
        let real_path =
          resolve_temp_file_name(cache_dir, &exe_name, path, &bytes);
        let cache_dir = real_path.parent().ok_or_else(|| {
          LoadError::Io(std::io::Error::other(
            "Native addon cache path had no parent",
          ))
        })?;
        ensure_private_native_addon_dir(cache_dir).map_err(LoadError::Io)?;
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
  let path_metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(_) => return false,
  };
  if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
    return false;
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::MetadataExt;
    if path_metadata.uid() != current_uid() {
      return false;
    }
  }

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

#[cfg(unix)]
fn current_uid() -> u32 {
  unsafe { libc::geteuid() }
}

fn resolve_temp_file_name(
  cache_dir: &Path,
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
  cache_dir.join(&file_name)
}

fn native_addon_cache_dir() -> std::io::Result<&'static Path> {
  static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

  if let Some(cache_dir) = CACHE_DIR.get() {
    return Ok(cache_dir);
  }

  let temp_dir = sys_traits::impls::RealSys.env_temp_dir()?;
  let cache_dir = create_native_addon_cache_dir(&temp_dir)?;
  Ok(CACHE_DIR.get_or_init(|| cache_dir))
}

fn create_native_addon_cache_dir(temp_dir: &Path) -> std::io::Result<PathBuf> {
  let preferred = temp_dir.join(NATIVE_ADDON_CACHE_DIR_NAME);
  if ensure_private_native_addon_dir(&preferred).is_ok() {
    return Ok(preferred);
  }

  let mut builder = tempfile::Builder::new();
  builder.prefix("deno-native-addons-");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    builder.permissions(fs::Permissions::from_mode(0o700));
  }
  let fallback = builder.tempdir_in(temp_dir)?;
  let fallback = fallback.into_path();
  validate_private_native_addon_dir(&fallback)?;
  Ok(fallback)
}

#[cfg(unix)]
fn create_private_native_addon_dir(path: &Path) -> std::io::Result<()> {
  use std::os::unix::fs::DirBuilderExt;

  let mut builder = fs::DirBuilder::new();
  builder.mode(0o700);
  match builder.create(path) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(()),
    Err(err) => Err(err),
  }
}

#[cfg(not(unix))]
fn create_private_native_addon_dir(path: &Path) -> std::io::Result<()> {
  match fs::create_dir(path) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(()),
    Err(err) => Err(err),
  }
}

fn ensure_private_native_addon_dir(path: &Path) -> std::io::Result<()> {
  create_private_native_addon_dir(path)?;
  validate_private_native_addon_dir(path)
}

fn validate_private_native_addon_dir(path: &Path) -> std::io::Result<()> {
  let metadata = fs::symlink_metadata(path)?;
  if metadata.file_type().is_symlink() || !metadata.is_dir() {
    return Err(std::io::Error::new(
      ErrorKind::PermissionDenied,
      format!(
        "Native addon cache path '{}' is not a private directory",
        path.display()
      ),
    ));
  }

  #[cfg(unix)]
  {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    if metadata.uid() != current_uid() {
      return Err(std::io::Error::new(
        ErrorKind::PermissionDenied,
        format!(
          "Native addon cache directory '{}' is not owned by the current user",
          path.display()
        ),
      ));
    }

    if metadata.permissions().mode() & 0o777 != 0o700 {
      fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
      let metadata = fs::symlink_metadata(path)?;
      if metadata.permissions().mode() & 0o777 != 0o700 {
        return Err(std::io::Error::new(
          ErrorKind::PermissionDenied,
          format!(
            "Native addon cache directory '{}' is not private",
            path.display()
          ),
        ));
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod test {
  #![allow(clippy::disallowed_methods, reason = "test code")]

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

  #[cfg(unix)]
  #[test]
  fn test_file_matches_bytes_rejects_symlink() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let target = tempdir.path().join("target.node");
    let link = tempdir.path().join("link.node");
    let bytes = b"native addon";
    std::fs::write(&target, bytes).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert!(!file_matches_bytes(&link, bytes));
  }

  #[test]
  fn test_resolve_temp_file_name() {
    let cache_dir = PathBuf::from("/native-addons");
    let file_path = PathBuf::from("/test/test.node");
    let bytes: [u8; 3] = [1, 2, 3];
    let temp_file =
      resolve_temp_file_name(&cache_dir, "exe_name", &file_path, &bytes);
    assert_eq!(
      temp_file,
      cache_dir.join("exe_name1805603793990095570513255480333703631005.node")
    );
  }

  #[cfg(unix)]
  #[test]
  fn test_native_addon_cache_dir_falls_back_from_symlink() {
    use std::os::unix::fs::PermissionsExt;

    let tempdir = tempfile::TempDir::new().unwrap();
    let target = tempdir.path().join("target");
    let preferred = tempdir.path().join(NATIVE_ADDON_CACHE_DIR_NAME);
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &preferred).unwrap();

    let cache_dir = create_native_addon_cache_dir(tempdir.path()).unwrap();

    assert_ne!(cache_dir, preferred);
    assert_eq!(cache_dir.parent(), Some(tempdir.path()));
    assert!(
      cache_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("deno-native-addons-")
    );
    let metadata = std::fs::symlink_metadata(&cache_dir).unwrap();
    assert!(metadata.is_dir());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
    assert!(std::fs::symlink_metadata(&preferred).unwrap().is_symlink());
  }

  #[cfg(unix)]
  #[test]
  fn test_ensure_private_native_addon_dir_creates_private_dir() {
    use std::os::unix::fs::PermissionsExt;

    let tempdir = tempfile::TempDir::new().unwrap();
    let path = tempdir.path().join("cache");

    ensure_private_native_addon_dir(&path).unwrap();

    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_dir());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
  }

  #[cfg(unix)]
  #[test]
  fn test_ensure_private_native_addon_dir_repairs_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let tempdir = tempfile::TempDir::new().unwrap();
    let path = tempdir.path().join("cache");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777))
      .unwrap();

    ensure_private_native_addon_dir(&path).unwrap();

    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
  }

  #[cfg(unix)]
  #[test]
  fn test_ensure_private_native_addon_dir_rejects_symlink() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let target = tempdir.path().join("target");
    let link = tempdir.path().join("cache");
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let err = ensure_private_native_addon_dir(&link).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::PermissionDenied);
  }
}
