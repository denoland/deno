use serde_json::json;
pub use serde_json::Value;
use std::collections::HashMap;
use std::io::Result;
use url::Url;

pub struct Lockfile {
  need_read: bool,
  map: HashMap<String, String>,
  pub filename: String,
}

impl Lockfile {
  pub fn new(filename: String) -> Lockfile {
    Lockfile {
      map: HashMap::new(),
      filename,
      need_read: true,
    }
  }

  pub fn write(&self) -> Result<()> {
    let j = json!(self.map);
    let s = serde_json::to_string_pretty(&j).unwrap();
    let mut f = std::fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .open(&self.filename)?;
    use std::io::Write;
    f.write_all(s.as_bytes())?;
    debug!("lockfile write {}", self.filename);
    Ok(())
  }

  pub fn read(&mut self) -> Result<()> {
    debug!("lockfile read {}", self.filename);
    let s = std::fs::read_to_string(&self.filename)?;
    self.map = serde_json::from_str(&s)?;
    self.need_read = false;
    Ok(())
  }

  /// Lazily reads the filename, checks the given module is included.
  /// Returns Ok(true) if check passed
  pub fn check(&mut self, url: &Url, code: Vec<u8>) -> Result<bool> {
    let url_str = url.to_string();
    if url_str.starts_with("file:") {
      return Ok(true);
    }
    if self.need_read {
      self.read()?;
    }
    assert!(!self.need_read);
    Ok(if let Some(lockfile_checksum) = self.map.get(&url_str) {
      let compiled_checksum = crate::checksum::gen(vec![&code]);
      lockfile_checksum == &compiled_checksum
    } else {
      false
    })
  }

  // Returns true if module was not already inserted.
  pub fn insert(&mut self, url: &Url, code: Vec<u8>) -> bool {
    let url_str = url.to_string();
    if url_str.starts_with("file:") {
      return false;
    }
    let checksum = crate::checksum::gen(vec![&code]);
    self.map.insert(url_str, checksum).is_none()
  }
}
