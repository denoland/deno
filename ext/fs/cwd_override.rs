// Copyright 2018-2026 the Deno authors. MIT license.

//! Thread-local working-directory override for embedders.
//!
//! Deno's real filesystem and subprocess helpers normally use the process
//! global cwd (`std::env::current_dir` / `set_current_dir`). When Deno is
//! embedded in another process, mutating that global cwd leaks into the host.
//!
//! An active [`CwdOverrideGuard`] scopes cwd reads and writes to the current
//! thread without changing the host process cwd.

use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

thread_local! {
  static CWD_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// RAII guard that installs a thread-local cwd override for the current thread.
///
/// Dropping the guard restores the previous override, so guards may be nested.
/// The guard must be dropped on the thread where it was created.
pub struct CwdOverrideGuard {
  previous: Option<PathBuf>,
  _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CwdOverrideGuard {
  /// Install `path` as the thread-local cwd override.
  ///
  /// Relative paths are resolved against the current override when present,
  /// otherwise against the real process cwd.
  pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
      return Err(io::Error::new(
        io::ErrorKind::NotFound,
        "working directory path is empty",
      ));
    }
    let absolute = if path.is_absolute() {
      path.to_path_buf()
    } else {
      current_dir()?.join(path)
    };
    #[allow(clippy::disallowed_methods)]
    let absolute = deno_path_util::strip_unc_prefix(
      std::fs::canonicalize(&absolute).unwrap_or(absolute),
    );
    if !absolute.is_dir() {
      return Err(io::Error::new(
        io::ErrorKind::NotADirectory,
        format!("{} is not a directory", absolute.display()),
      ));
    }
    let previous =
      CWD_OVERRIDE.with(|slot| slot.borrow_mut().replace(absolute));
    Ok(Self {
      previous,
      _not_send_or_sync: PhantomData,
    })
  }
}

impl Drop for CwdOverrideGuard {
  fn drop(&mut self) {
    CWD_OVERRIDE.with(|slot| {
      *slot.borrow_mut() = self.previous.take();
    });
  }
}

/// Returns the active thread-local cwd override, if one is installed.
pub fn current_override() -> Option<PathBuf> {
  CWD_OVERRIDE.with(|slot| slot.borrow().clone())
}

/// Returns the thread-local override when set, otherwise the process cwd.
pub fn current_dir() -> io::Result<PathBuf> {
  if let Some(override_cwd) = current_override() {
    return Ok(override_cwd);
  }
  #[allow(clippy::disallowed_methods)]
  std::env::current_dir()
}

/// Updates the thread-local override when one is active; otherwise changes the
/// process cwd.
pub fn set_current_dir(path: impl AsRef<Path>) -> io::Result<()> {
  let path = path.as_ref();
  if path.as_os_str().is_empty() {
    return Err(io::Error::new(
      io::ErrorKind::NotFound,
      "working directory path is empty",
    ));
  }
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    current_dir()?.join(path)
  };
  #[allow(clippy::disallowed_methods)]
  let absolute = deno_path_util::strip_unc_prefix(
    std::fs::canonicalize(&absolute).unwrap_or(absolute),
  );
  if !absolute.is_dir() {
    return Err(io::Error::new(
      io::ErrorKind::NotADirectory,
      format!("{} is not a directory", absolute.display()),
    ));
  }

  let replaced = CWD_OVERRIDE.with(|slot| {
    let mut slot = slot.borrow_mut();
    if slot.is_some() {
      *slot = Some(absolute.clone());
      true
    } else {
      false
    }
  });
  if replaced {
    return Ok(());
  }
  #[allow(clippy::disallowed_methods)]
  std::env::set_current_dir(absolute)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::FileSystem;
  use crate::OpenOptions;
  use crate::RealFs;
  use deno_permissions::CheckedPath;
  use std::borrow::Cow;
  use std::fs;
  use std::sync::Mutex;
  use std::time::{SystemTime, UNIX_EPOCH};

  // Serialize tests that touch process cwd / TLS.
  static LOCK: Mutex<()> = Mutex::new(());

  fn temp_subdir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path =
      std::env::temp_dir().join(format!("deno-cwd-override-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
  }

  fn canonicalize(path: impl AsRef<Path>) -> PathBuf {
    deno_path_util::strip_unc_prefix(std::fs::canonicalize(path).unwrap())
  }

  #[test]
  fn override_does_not_mutate_process_cwd() {
    let _lock = LOCK.lock().unwrap();
    let previous = std::env::current_dir().unwrap();
    let temp = temp_subdir("primary");
    let temp = canonicalize(&temp);

    {
      let _guard = CwdOverrideGuard::new(&temp).unwrap();
      assert_eq!(current_dir().unwrap(), temp);
      assert_eq!(std::env::current_dir().unwrap(), previous);
    }

    assert_eq!(current_dir().unwrap(), previous);
    assert_eq!(std::env::current_dir().unwrap(), previous);
    let _ = fs::remove_dir_all(temp);
  }

  #[test]
  fn set_current_dir_updates_override_only() {
    let _lock = LOCK.lock().unwrap();
    let previous = std::env::current_dir().unwrap();
    let first = canonicalize(temp_subdir("first"));
    let second = canonicalize(temp_subdir("second"));

    {
      let _guard = CwdOverrideGuard::new(&first).unwrap();
      set_current_dir(&second).unwrap();
      assert_eq!(current_dir().unwrap(), second);
      assert_eq!(std::env::current_dir().unwrap(), previous);
    }

    assert_eq!(current_dir().unwrap(), previous);
    let _ = fs::remove_dir_all(first);
    let _ = fs::remove_dir_all(second);
  }

  #[test]
  fn nested_override_restores_previous_override() {
    let _lock = LOCK.lock().unwrap();
    let previous = std::env::current_dir().unwrap();
    let first = canonicalize(temp_subdir("outer"));
    let second = canonicalize(temp_subdir("inner"));

    {
      let _outer = CwdOverrideGuard::new(&first).unwrap();
      assert_eq!(current_override().as_deref(), Some(first.as_path()));
      {
        let _inner = CwdOverrideGuard::new(&second).unwrap();
        assert_eq!(current_dir().unwrap(), second);
        assert_eq!(std::env::current_dir().unwrap(), previous);
      }
      assert_eq!(current_dir().unwrap(), first);
      assert_eq!(std::env::current_dir().unwrap(), previous);
    }

    assert_eq!(current_override(), None);
    assert_eq!(std::env::current_dir().unwrap(), previous);
    let _ = fs::remove_dir_all(first);
    let _ = fs::remove_dir_all(second);
  }

  #[test]
  fn relative_real_fs_writes_use_override() {
    let _lock = LOCK.lock().unwrap();
    let temp = canonicalize(temp_subdir("relative-write"));
    let path =
      CheckedPath::unsafe_new(Cow::Borrowed(Path::new("relative.txt")));

    {
      let _guard = CwdOverrideGuard::new(&temp).unwrap();
      RealFs
        .write_file_sync(
          &path,
          OpenOptions {
            write: true,
            create: true,
            truncate: true,
            ..Default::default()
          },
          b"override",
        )
        .unwrap();
    }

    assert_eq!(fs::read(temp.join("relative.txt")).unwrap(), b"override");
    let _ = fs::remove_dir_all(temp);
  }

  #[test]
  fn empty_real_fs_path_does_not_resolve_to_override() {
    let _lock = LOCK.lock().unwrap();
    let temp = canonicalize(temp_subdir("empty-path"));
    let path = CheckedPath::unsafe_new(Cow::Borrowed(Path::new("")));

    {
      let _guard = CwdOverrideGuard::new(&temp).unwrap();
      assert!(RealFs.remove_sync(&path, true).is_err());
    }

    assert!(temp.is_dir());
    let _ = fs::remove_dir_all(temp);
  }

  #[test]
  fn empty_cwd_path_remains_invalid() {
    let _lock = LOCK.lock().unwrap();
    let temp = canonicalize(temp_subdir("empty-cwd"));

    let Err(error) = CwdOverrideGuard::new("") else {
      panic!("empty cwd override should fail");
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    {
      let _guard = CwdOverrideGuard::new(&temp).unwrap();
      assert_eq!(
        set_current_dir("").unwrap_err().kind(),
        io::ErrorKind::NotFound
      );
      assert_eq!(current_dir().unwrap(), temp);
    }

    let _ = fs::remove_dir_all(temp);
  }
}
