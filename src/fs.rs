use std;
use std::fs::{create_dir, File, OpenOptions};
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};

use rand;
use rand::Rng;

#[cfg(any(unix))]
use std::os::unix::fs::PermissionsExt;

pub fn write_file(
  filename: &Path,
  data: &[u8],
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
  file.write_all(data)
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

pub fn mkdir(path: &Path) -> std::io::Result<()> {
  debug!("mkdir -p {}", path.display());
  assert!(path.has_root(), "non-has_root not yet implemented");
  std::fs::create_dir_all(path).or_else(|err| {
    if err.kind() == std::io::ErrorKind::AlreadyExists {
      Ok(())
    } else {
      Err(err)
    }
  })
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
