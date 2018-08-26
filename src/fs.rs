use std;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn write_file_sync(path: &Path, content: &[u8]) -> std::io::Result<()> {
  let mut f = File::create(path)?;
  f.write_all(content)
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
