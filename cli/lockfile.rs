// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use deno_core::serde_json::json;
use log::debug;
use std::collections::BTreeMap;
use std::io::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Lockfile {
  write: bool,
  map: BTreeMap<String, String>,
  pub filename: PathBuf,
}

impl Lockfile {
  pub fn new(filename: PathBuf, write: bool) -> Result<Lockfile> {
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
    let j = json!(&self.map);
    let s = serde_json::to_string_pretty(&j).unwrap();
    let mut f = std::fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .open(&self.filename)?;
    use std::io::Write;
    f.write_all(s.as_bytes())?;
    debug!("lockfile write {}", self.filename.display());
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
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use std::fs::File;
  use std::io::prelude::*;
  use std::io::Write;
  use tempfile::TempDir;

  fn setup() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("could not create temp dir");

    let file_path = temp_dir.path().join("valid_lockfile.json");
    let mut file = File::create(file_path).expect("write file fail");

    let value: serde_json::Value = json!({
      "https://deno.land/std@0.71.0/textproto/mod.ts": "3118d7a42c03c242c5a49c2ad91c8396110e14acca1324e7aaefd31a999b71a4",
      "https://deno.land/std@0.71.0/async/delay.ts": "35957d585a6e3dd87706858fb1d6b551cb278271b03f52c5a2cb70e65e00c26a"
    });

    file.write_all(value.to_string().as_bytes()).unwrap();

    let file_path = temp_dir.path().join("valid_lockfile.json");

    (temp_dir, file_path)
  }

  fn teardown(temp_dir: TempDir) {
    temp_dir.close().expect("file close error");
  }

  #[test]
  fn new_nonexistent_lockfile() {
    let file_path = PathBuf::from("nonexistent_lock_file.json");
    assert!(Lockfile::new(file_path, false).is_err());
  }

  #[test]
  fn new_valid_lockfile() {
    let (temp_dir, file_path) = setup();

    let result = Lockfile::new(file_path, false).unwrap();

    let keys: Vec<String> = result.map.keys().cloned().collect();
    let expected_keys = vec![
      String::from("https://deno.land/std@0.71.0/async/delay.ts"),
      String::from("https://deno.land/std@0.71.0/textproto/mod.ts"),
    ];

    assert_eq!(keys.len(), 2);
    assert_eq!(keys, expected_keys);

    teardown(temp_dir);
  }

  #[test]
  fn new_lockfile_from_file_and_insert() {
    let (temp_dir, file_path) = setup();

    let mut lockfile = Lockfile::new(file_path, false).unwrap();

    lockfile.insert(
      "https://deno.land/std@0.71.0/io/util.ts",
      "Here is some source code",
    );

    let keys: Vec<String> = lockfile.map.keys().cloned().collect();
    let expected_keys = vec![
      String::from("https://deno.land/std@0.71.0/async/delay.ts"),
      String::from("https://deno.land/std@0.71.0/io/util.ts"),
      String::from("https://deno.land/std@0.71.0/textproto/mod.ts"),
    ];
    assert_eq!(keys.len(), 3);
    assert_eq!(keys, expected_keys);

    teardown(temp_dir);
  }

  #[test]
  fn new_lockfile_and_write() {
    let (temp_dir, file_path) = setup();

    let mut lockfile = Lockfile::new(file_path, true).unwrap();

    lockfile.insert(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "Here is some source code",
    );
    lockfile.insert(
      "https://deno.land/std@0.71.0/io/util.ts",
      "more source code here",
    );
    lockfile.insert(
      "https://deno.land/std@0.71.0/async/delay.ts",
      "this source is really exciting",
    );

    lockfile.write().expect("unable to write");

    let file_path_buf = temp_dir.path().join("valid_lockfile.json");
    let file_path = file_path_buf.to_str().expect("file path fail").to_string();

    // read the file contents back into a string and check
    let mut checkfile = File::open(file_path).expect("Unable to open the file");
    let mut contents = String::new();
    checkfile
      .read_to_string(&mut contents)
      .expect("Unable to read the file");

    let contents_json =
      serde_json::from_str::<serde_json::Value>(&contents).unwrap();
    let object = contents_json.as_object().unwrap();

    assert_eq!(
      object
        .get("https://deno.land/std@0.71.0/textproto/mod.ts")
        .and_then(|v| v.as_str()),
      // sha-256 hash of the source 'Here is some source code'
      Some("fedebba9bb82cce293196f54b21875b649e457f0eaf55556f1e318204947a28f")
    );

    // confirm that keys are sorted alphabetically
    let mut keys = object.keys().map(|k| k.as_str());
    assert_eq!(
      keys.next(),
      Some("https://deno.land/std@0.71.0/async/delay.ts")
    );
    assert_eq!(keys.next(), Some("https://deno.land/std@0.71.0/io/util.ts"));
    assert_eq!(
      keys.next(),
      Some("https://deno.land/std@0.71.0/textproto/mod.ts")
    );
    assert!(keys.next().is_none());

    teardown(temp_dir);
  }

  #[test]
  fn check_or_insert_lockfile_false() {
    let (temp_dir, file_path) = setup();

    let mut lockfile = Lockfile::new(file_path, false).unwrap();

    lockfile.insert(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "Here is some source code",
    );

    let check_true = lockfile.check_or_insert(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "Here is some source code",
    );
    assert!(check_true);

    let check_false = lockfile.check_or_insert(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "This is new Source code",
    );
    assert!(!check_false);

    teardown(temp_dir);
  }
}
