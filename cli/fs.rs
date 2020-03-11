// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use std;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use deno_core::ErrBox;
use rand;
use rand::Rng;
use walkdir::WalkDir;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
use nix::unistd::{chown as unix_chown, Gid, Uid};

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  write_file_2(filename, data, true, mode, true, false)
}

pub fn write_file_2<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  update_mode: bool,
  mode: u32,
  is_create: bool,
  is_append: bool,
) -> std::io::Result<()> {
  let mut file = OpenOptions::new()
    .read(false)
    .write(true)
    .append(is_append)
    .truncate(!is_append)
    .create(is_create)
    .open(filename)?;

  if update_mode {
    set_permissions(&mut file, mode)?;
  }

  file.write_all(data.as_ref())
}

#[cfg(unix)]
fn set_permissions(file: &mut File, mode: u32) -> std::io::Result<()> {
  debug!("set file mode to {}", mode);
  file.set_permissions(PermissionsExt::from_mode(mode & 0o777))
}

#[cfg(not(unix))]
fn set_permissions(_file: &mut File, _mode: u32) -> std::io::Result<()> {
  // NOOP on windows
  Ok(())
}

pub fn make_temp(
  dir: Option<&Path>,
  prefix: Option<&str>,
  suffix: Option<&str>,
  is_dir: bool,
) -> std::io::Result<PathBuf> {
  let prefix_ = prefix.unwrap_or("");
  let suffix_ = suffix.unwrap_or("");
  let mut buf: PathBuf = match dir {
    Some(ref p) => p.to_path_buf(),
    None => std::env::temp_dir(),
  }
  .join("_");
  let mut rng = rand::thread_rng();
  loop {
    let unique = rng.gen::<u32>();
    buf.set_file_name(format!("{}{:08x}{}", prefix_, unique, suffix_));
    let r = if is_dir {
      let mut builder = DirBuilder::new();
      set_dir_permission(&mut builder, 0o700);
      builder.create(buf.as_path())
    } else {
      let mut open_options = OpenOptions::new();
      open_options.write(true).create_new(true);
      #[cfg(unix)]
      open_options.mode(0o600);
      open_options.open(buf.as_path())?;
      Ok(())
    };
    match r {
      Err(ref e) if e.kind() == ErrorKind::AlreadyExists => continue,
      Ok(_) => return Ok(buf),
      Err(e) => return Err(e),
    }
  }
}

pub fn mkdir(path: &Path, mode: u32, recursive: bool) -> std::io::Result<()> {
  debug!("mkdir -p {}", path.display());
  let mut builder = DirBuilder::new();
  builder.recursive(recursive);
  set_dir_permission(&mut builder, mode);
  builder.create(path)
}

#[cfg(unix)]
fn set_dir_permission(builder: &mut DirBuilder, mode: u32) {
  debug!("set dir mode to {}", mode);
  builder.mode(mode & 0o777);
}

#[cfg(not(unix))]
fn set_dir_permission(_builder: &mut DirBuilder, _mode: u32) {
  // NOOP on windows
}

#[cfg(unix)]
pub fn chown(path: &str, uid: u32, gid: u32) -> Result<(), ErrBox> {
  let nix_uid = Uid::from_raw(uid);
  let nix_gid = Gid::from_raw(gid);
  unix_chown(path, Option::Some(nix_uid), Option::Some(nix_gid))
    .map_err(ErrBox::from)
}

#[cfg(not(unix))]
pub fn chown(_path: &str, _uid: u32, _gid: u32) -> Result<(), ErrBox> {
  // FAIL on Windows
  // TODO: implement chown for Windows
  let e = std::io::Error::new(
    std::io::ErrorKind::Other,
    "Not implemented".to_string(),
  );
  Err(ErrBox::from(e))
}

/// Normalize all itermediate components of the path (ie. remove "./" and "../" components).
/// Similar to `fs::canonicalize()` but doesn't resolve symlinks.
///
/// Taken from Cargo
/// https://github.com/rust-lang/cargo/blob/af307a38c20a753ec60f0ad18be5abed3db3c9ac/src/cargo/util/paths.rs#L60-L85
pub fn normalize_path(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret =
    if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
      components.next();
      PathBuf::from(c.as_os_str())
    } else {
      PathBuf::new()
    };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      }
      Component::CurDir => {}
      Component::ParentDir => {
        ret.pop();
      }
      Component::Normal(c) => {
        ret.push(c);
      }
    }
  }
  ret
}

pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, ErrBox> {
  let resolved_path = if path.is_absolute() {
    path.to_owned()
  } else {
    let cwd = std::env::current_dir().unwrap();
    cwd.join(path)
  };

  Ok(normalize_path(&resolved_path))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a/..")).unwrap(), cwd);
  }

  #[test]
  fn test_normalize_path() {
    assert_eq!(normalize_path(Path::new("a/../b")), PathBuf::from("b"));
    assert_eq!(normalize_path(Path::new("a/./b/")), PathBuf::from("a/b/"));
    assert_eq!(
      normalize_path(Path::new("a/./b/../c")),
      PathBuf::from("a/c")
    );

    if cfg!(windows) {
      assert_eq!(
        normalize_path(Path::new("C:\\a\\.\\b\\..\\c")),
        PathBuf::from("C:\\a\\c")
      );
    }
  }

  // TODO: Get a good expected value here for Windows.
  #[cfg(not(windows))]
  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("/a");
    assert_eq!(resolve_from_cwd(expected).unwrap(), expected);
  }
}

pub fn files_in_subtree<F>(root: PathBuf, filter: F) -> Vec<PathBuf>
where
  F: Fn(&Path) -> bool,
{
  assert!(root.is_dir());

  WalkDir::new(root)
    .into_iter()
    .filter_map(|e| e.ok())
    .map(|e| e.path().to_owned())
    .filter(|p| if p.is_dir() { false } else { filter(&p) })
    .collect()
}
