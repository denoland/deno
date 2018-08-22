use std;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;

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
