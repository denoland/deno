use std;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[allow(dead_code)]
pub fn read_file_sync(path: &Path) -> std::io::Result<String> {
  File::open(path).and_then(|mut f| {
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    Ok(contents)
  })
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
