use crate::compilers::CompiledModule;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;
use std::collections::HashMap;
use std::io::Result;

#[derive(Deserialize)]
struct Map(pub HashMap<String, String>);

pub struct Lockfile {
  read: bool,
  map: Map,
  pub filename: String,
}

impl Lockfile {
  pub fn from_flag(flag: &Option<String>) -> Option<Lockfile> {
    if let Some(filename) = flag {
      Some(Self::new(filename.to_string()))
    } else {
      None
    }
  }

  pub fn new(filename: String) -> Lockfile {
    Lockfile {
      map: Map(HashMap::new()),
      filename,
      read: false,
    }
  }

  pub fn write(&self) -> Result<()> {
    let j = json!(self.map.0);
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
    self.map.0 = serde_json::from_str(&s)?;
    self.read = true;
    Ok(())
  }

  /// Lazily reads the filename, checks the given module is included.
  /// Returns Ok(true) if check passed
  pub fn check(&mut self, m: &CompiledModule) -> Result<bool> {
    if !self.read {
      self.read()?;
    }

    Ok(if let Some(lockfile_checksum) = self.map.0.get(&m.name) {
      let compiled_checksum = crate::checksum::gen2(&m.code);
      lockfile_checksum == &compiled_checksum
    } else {
      false
    })
  }

  pub fn insert(&mut self, m: &CompiledModule) -> bool {
    let checksum = crate::checksum::gen2(&m.code);
    self.map.0.insert(m.name.clone(), checksum).is_none()
  }
}
