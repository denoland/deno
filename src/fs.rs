use std;

use std::fs::{create_dir, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use rand;
use rand::Rng;

#[cfg(target_os = "unix")]
use std::os::unix::fs::PermissionsExt;

pub fn read_file_sync(path: &Path) -> std::io::Result<Vec<u8>> {
  File::open(path).and_then(|mut f| {
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
  })
}

pub fn read_file_sync_string(path: &Path) -> std::io::Result<String> {
  let vec = read_file_sync(path)?;
  String::from_utf8(vec)
    .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

pub fn write_file_sync(path: &Path, content: &[u8], perm: u32) -> std::io::Result<()> {
  let is_append = perm & (1 << 31) != 0;
  let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .append(is_append)
            .truncate(!is_append)
            .create(true)
            .open(path)?;
  
  set_permissions(perm);
  file.write_all(content)
}

#[cfg(target_os = "unix")]
fn set_permissions(perm: u32) {
  file.set_permissions(PermissionsExt::from_mode(perm & 0o777))?;
}

#[cfg(not(target_os = "unix"))]
fn set_permissions(perm: u32) {
  // Windows does not work with mode bits for files like Unixes does, so this is a noop
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
