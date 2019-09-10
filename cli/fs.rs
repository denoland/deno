// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std;
use std::fs::{create_dir, DirBuilder, File, OpenOptions};
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};

use deno::ErrBox;
use rand;
use rand::Rng;
use url::Url;

#[cfg(unix)]
use nix::unistd::{chown as unix_chown, Gid, Uid};
#[cfg(any(unix))]
use std::os::unix::fs::DirBuilderExt;
#[cfg(any(unix))]
use std::os::unix::fs::PermissionsExt;

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  perm: u32,
) -> std::io::Result<()> {
  write_file_2(filename, data, true, perm, true, false)
}

pub fn write_file_2<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  update_perm: bool,
  perm: u32,
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

  if update_perm {
    set_permissions(&mut file, perm)?;
  }

  file.write_all(data.as_ref())
}

#[cfg(any(unix))]
fn set_permissions(file: &mut File, perm: u32) -> std::io::Result<()> {
  debug!("set file perm to {}", perm);
  file.set_permissions(PermissionsExt::from_mode(perm & 0o777))
}
#[cfg(not(any(unix)))]
fn set_permissions(_file: &mut File, _perm: u32) -> std::io::Result<()> {
  // NOOP on windows
  Ok(())
}

pub fn make_temp_dir(
  dir: Option<&Path>,
  prefix: Option<&str>,
  suffix: Option<&str>,
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
    // TODO: on posix, set mode flags to 0o700.
    let r = create_dir(buf.as_path());
    match r {
      Err(ref e) if e.kind() == ErrorKind::AlreadyExists => continue,
      Ok(_) => return Ok(buf),
      Err(e) => return Err(e),
    }
  }
}

pub fn mkdir(path: &Path, perm: u32, recursive: bool) -> std::io::Result<()> {
  debug!("mkdir -p {}", path.display());
  let mut builder = DirBuilder::new();
  builder.recursive(recursive);
  set_dir_permission(&mut builder, perm);
  builder.create(path)
}

#[cfg(any(unix))]
fn set_dir_permission(builder: &mut DirBuilder, perm: u32) {
  debug!("set dir perm to {}", perm);
  builder.mode(perm & 0o777);
}

#[cfg(not(any(unix)))]
fn set_dir_permission(_builder: &mut DirBuilder, _perm: u32) {
  // NOOP on windows
}

pub fn normalize_path(path: &Path) -> String {
  let s = String::from(path.to_str().unwrap());
  if cfg!(windows) {
    // TODO This isn't correct. Probbly should iterate over components.
    s.replace("\\", "/")
  } else {
    s
  }
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
  // Noop
  // TODO: implement chown for Windows
  Err(crate::deno_error::op_not_implemented())
}

pub fn resolve_from_cwd(path: &str) -> Result<(PathBuf, String), ErrBox> {
  let candidate_path = Path::new(path);

  let resolved_path = if candidate_path.is_absolute() {
    candidate_path.to_owned()
  } else {
    let cwd = std::env::current_dir().unwrap();
    cwd.join(path)
  };

  // HACK: `Url::parse` is used here because it normalizes the path.
  // Joining `/dev/deno/" with "./tests" using `PathBuf` yields `/deno/dev/./tests/`.
  // On the other hand joining `/dev/deno/" with "./tests" using `Url` yields "/dev/deno/tests"
  // - and that's what we want.
  // There exists similar method on `PathBuf` - `PathBuf.canonicalize`, but the problem
  // is `canonicalize` resolves symlinks and we don't want that.
  // We just want to normalize the path...
  // This only works on absolute paths - not worth extracting as a public utility.
  let resolved_url =
    Url::from_file_path(resolved_path).expect("Path should be absolute");
  let normalized_url = Url::parse(resolved_url.as_str())
    .expect("String from a URL should parse to a URL");
  let normalized_path = normalized_url
    .to_file_path()
    .expect("URL from a path should contain a valid path");

  let path_string = normalized_path.to_str().unwrap().to_string();

  Ok((normalized_path, path_string))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd("a").unwrap().0, cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd(".").unwrap().0, cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(resolve_from_cwd("a/..").unwrap().0, cwd);
  }

  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = if cfg!(windows) {
      Path::new("C:\\a")
    } else {
      Path::new("/a")
    };
    assert_eq!(resolve_from_cwd("/a").unwrap().0, expected);
  }
}
