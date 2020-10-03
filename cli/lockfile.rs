// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use deno_core::serde_json::json;
use std::collections::BTreeMap;
use std::io::Result;

#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Write;

  #[test]
  fn new_nonexistent_lockfile() {
    let file_path = String::from("nonexistent_lock_file.json");
    assert!(Lockfile::new(file_path, false).is_err());
  }

  #[test]
  fn new_valid_lockfile() {
    // create a valid lockfile for us to load
    let t = tempfile::TempDir::new().expect("tempdir fail");
    let file_path = t.path().join("valid_lockfile.json");
    let mut file = File::create(file_path).expect("write file fail");
    writeln!(file, "{{").expect("write line fail");
    writeln!(file, "  \"https://deno.land/std@0.71.0/textproto/mod.ts\": \"3118d7a42c03c242c5a49c2ad91c8396110e14acca1324e7aaefd31a999b71a4\",").expect("write line fail");
    writeln!(file, "  \"https://deno.land/std@0.71.0/io/util.ts\": \"ae133d310a0fdcf298cea7bc09a599c49acb616d34e148e263bcb02976f80dee\",").expect("write line fail");
    writeln!(file, "  \"https://deno.land/std@0.71.0/async/delay.ts\": \"35957d585a6e3dd87706858fb1d6b551cb278271b03f52c5a2cb70e65e00c26a\"").expect("write line fail");
    writeln!(file, "}}").expect("write line fail");

    //prep the file path again.. because borrowing
    let file_path_buf = t.path().join("valid_lockfile.json");
    let file_path = file_path_buf.to_str().expect("file path fail").to_string();

    let result = Lockfile::new(file_path, false);

    let lockfile = match result {
      Ok(lockfile) => lockfile,
      Err(error) => panic!("Lockfile was not created: {:?}", error),
    };
    let keys: Vec<String> = lockfile.map.keys().cloned().collect();
    assert_eq!(keys.len(), 3);
  }
}