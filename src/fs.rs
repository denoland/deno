// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std;
use std::fs::{create_dir, DirBuilder, File, OpenOptions};
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};

use rand;
use rand::Rng;

#[cfg(any(unix))]
use std::os::unix::fs::DirBuilderExt;
#[cfg(any(unix))]
use std::os::unix::fs::PermissionsExt;

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  perm: u32,
) -> std::io::Result<()> {
  let is_append = perm & (1 << 31) != 0;
  let mut file = OpenOptions::new()
    .read(false)
    .write(true)
    .append(is_append)
    .truncate(!is_append)
    .create(true)
    .open(filename)?;

  set_permissions(&mut file, perm)?;
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
  }.join("_");
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

pub fn mkdir(path: &Path, perm: u32) -> std::io::Result<()> {
  debug!("mkdir -p {}", path.display());
  let mut builder = DirBuilder::new();
  builder.recursive(true);
  set_dir_permission(&mut builder, perm);
  builder.create(path).or_else(|err| match err.kind() {
    std::io::ErrorKind::AlreadyExists => Ok(()),
    _ => Err(err),
  })
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
