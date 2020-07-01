use serde_json::json;
pub use serde_json::Value;
use std::collections::HashMap;
use std::io::Result;
use url::Url;

pub struct Lockfile {
  write: bool,
  map: HashMap<String, String>,
  pub filename: String,
}

impl Lockfile {
  pub fn new(filename: String, write: bool) -> Result<Lockfile> {
    debug!("lockfile \"{}\", write: {}", filename, write);

    let map = if write {
      HashMap::new()
    } else {
      let s = std::fs::read_to_string(&filename)?;
      serde_json::from_str(&s)?
    };

    Ok(Lockfile {
      write,
      map,
      filename,
    })
  }

  // TODO(bartlomieju): write in alphabetical order
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

  pub fn check_or_insert(&mut self, url: &Url, code: Vec<u8>) -> bool {
    if self.write {
      self.insert(url, code)
    } else {
      self.check(url, code)
    }
  }

  /// Checks the given module is included.
  /// Returns Ok(true) if check passed.
  fn check(&mut self, url: &Url, code: Vec<u8>) -> bool {
    let url_str = url.to_string();
    if url_str.starts_with("file:") {
      return true;
    }
    if let Some(lockfile_checksum) = self.map.get(&url_str) {
      let compiled_checksum = crate::checksum::gen(&[&code]);
      lockfile_checksum == &compiled_checksum
    } else {
      false
    }
  }

  fn insert(&mut self, url: &Url, code: Vec<u8>) -> bool {
    let url_str = url.to_string();
    if url_str.starts_with("file:") {
      return true;
    }
    let checksum = crate::checksum::gen(&[&code]);
    self.map.insert(url_str, checksum);
    true
  }
}
