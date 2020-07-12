use serde_json::json;
pub use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Result;

pub struct Lockfile {
  write: bool,
  map: BTreeMap<String, String>,
  pub filename: String,
}

impl Lockfile {
  pub fn new(filename: String, write: bool) -> Result<Lockfile> {
    debug!("lockfile \"{}\", write: {}", filename, write);

    let map = if write {
      BTreeMap::new()
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

  // Synchronize lock file to disk - noop if --lock-write file is not specified.
  pub fn write(&self) -> Result<()> {
    if !self.write {
      return Ok(());
    }
    // Will perform sort so output is deterministic
    let map: BTreeMap<_, _> = self.map.iter().collect();
    let j = json!(map);
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

  pub fn check_or_insert(&mut self, specifier: &str, code: &str) -> bool {
    if self.write {
      // In case --lock-write is specified check always passes
      self.insert(specifier, code);
      true
    } else {
      self.check(specifier, code)
    }
  }

  /// Checks the given module is included.
  /// Returns Ok(true) if check passed.
  fn check(&mut self, specifier: &str, code: &str) -> bool {
    if specifier.starts_with("file:") {
      return true;
    }
    if let Some(lockfile_checksum) = self.map.get(specifier) {
      let compiled_checksum = crate::checksum::gen(&[code.as_bytes()]);
      lockfile_checksum == &compiled_checksum
    } else {
      false
    }
  }

  fn insert(&mut self, specifier: &str, code: &str) {
    if specifier.starts_with("file:") {
      return;
    }
    let checksum = crate::checksum::gen(&[code.as_bytes()]);
    self.map.insert(specifier.to_string(), checksum);
  }
}
