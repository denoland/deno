// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

#![allow(dead_code)]

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use log::debug;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::tools::fmt::format_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockfileV2Content {
  version: String,
  // Mapping between URLs and their checksums
  remote: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LockfileV1Content(BTreeMap<String, String>);

#[derive(Debug, Clone)]
enum LockfileContent {
  V1(LockfileV1Content),
  V2(LockfileV2Content),
}

#[derive(Debug, Clone)]
pub struct Lockfile {
  write: bool,
  content: LockfileContent,
  pub filename: PathBuf,
}

impl Lockfile {
  pub fn new(filename: PathBuf, write: bool) -> Result<Lockfile, AnyError> {
    // Writing a lock file always uses the new format.
    let content = if write {
      LockfileContent::V2(LockfileV2Content {
        version: "2".to_string(),
        remote: BTreeMap::new(),
      })
    } else {
      let s = std::fs::read_to_string(&filename).with_context(|| {
        format!("Unable to read lockfile: {}", filename.display())
      })?;
      let value: serde_json::Value = serde_json::from_str(&s)
        .context("Unable to parse contents of the lockfile")?;
      let version = value.get("version").and_then(|v| v.as_str());
      if version == Some("2") {
        let content: LockfileV2Content =
          serde_json::from_value(value).context("Unable to parse lockfile")?;
        LockfileContent::V2(content)
      } else {
        let content: LockfileV1Content =
          serde_json::from_value(value).context("Unable to parse lockfile")?;
        LockfileContent::V1(content)
      }
    };

    Ok(Lockfile {
      write,
      content,
      filename,
    })
  }

  // Synchronize lock file to disk - noop if --lock-write file is not specified.
  pub fn write(&self) -> Result<(), AnyError> {
    if !self.write {
      return Ok(());
    }

    let json_string = match &self.content {
      LockfileContent::V1(c) => {
        let j = json!(&c.0);
        serde_json::to_string(&j).unwrap()
      }
      LockfileContent::V2(c) => serde_json::to_string(&c).unwrap(),
    };

    let format_s = format_json(&json_string, &Default::default())
      .ok()
      .flatten()
      .unwrap_or(json_string);
    let mut f = std::fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .open(&self.filename)?;
    f.write_all(format_s.as_bytes())?;
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
    match &self.content {
      LockfileContent::V1(c) => {
        if let Some(lockfile_checksum) = c.0.get(specifier) {
          let compiled_checksum = crate::checksum::gen(&[code.as_bytes()]);
          lockfile_checksum == &compiled_checksum
        } else {
          false
        }
      }
      LockfileContent::V2(c) => {
        if let Some(lockfile_checksum) = c.remote.get(specifier) {
          let compiled_checksum = crate::checksum::gen(&[code.as_bytes()]);
          lockfile_checksum == &compiled_checksum
        } else {
          false
        }
      }
    }
  }

  fn insert(&mut self, specifier: &str, code: &str) {
    if specifier.starts_with("file:") {
      return;
    }
    let checksum = crate::checksum::gen(&[code.as_bytes()]);
    match &mut self.content {
      LockfileContent::V1(c) => {
        c.0.insert(specifier.to_string(), checksum);
      }
      LockfileContent::V2(c) => {
        c.remote.insert(specifier.to_string(), checksum);
      }
    };
  }
}

#[derive(Debug)]
pub struct Locker(Option<Arc<Mutex<Lockfile>>>);

impl deno_graph::source::Locker for Locker {
  fn check_or_insert(
    &mut self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> bool {
    if let Some(lock_file) = &self.0 {
      let mut lock_file = lock_file.lock();
      lock_file.check_or_insert(specifier.as_str(), source)
    } else {
      true
    }
  }

  fn get_checksum(&self, content: &str) -> String {
    crate::checksum::gen(&[content.as_bytes()])
  }

  fn get_filename(&self) -> Option<String> {
    let lock_file = self.0.as_ref()?.lock();
    lock_file.filename.to_str().map(|s| s.to_string())
  }
}

pub fn as_maybe_locker(
  lockfile: Option<Arc<Mutex<Lockfile>>>,
) -> Option<Rc<RefCell<Box<dyn deno_graph::source::Locker>>>> {
  lockfile.as_ref().map(|lf| {
    Rc::new(RefCell::new(
      Box::new(Locker(Some(lf.clone()))) as Box<dyn deno_graph::source::Locker>
    ))
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use std::fs::File;
  use std::io::prelude::*;
  use std::io::Write;
  use test_util::TempDir;

  fn setup(temp_dir: &TempDir) -> PathBuf {
    let file_path = temp_dir.path().join("valid_lockfile.json");
    let mut file = File::create(file_path).expect("write file fail");

    let value: serde_json::Value = json!({
      "version": "2",
      "remote": {
        "https://deno.land/std@0.71.0/textproto/mod.ts": "3118d7a42c03c242c5a49c2ad91c8396110e14acca1324e7aaefd31a999b71a4",
        "https://deno.land/std@0.71.0/async/delay.ts": "35957d585a6e3dd87706858fb1d6b551cb278271b03f52c5a2cb70e65e00c26a"
      }
    });

    file.write_all(value.to_string().as_bytes()).unwrap();

    temp_dir.path().join("valid_lockfile.json")
  }

  #[test]
  fn new_nonexistent_lockfile() {
    let file_path = PathBuf::from("nonexistent_lock_file.json");
    assert!(Lockfile::new(file_path, false).is_err());
  }

  #[test]
  fn new_valid_lockfile() {
    let temp_dir = TempDir::new();
    let file_path = setup(&temp_dir);

    let result = Lockfile::new(file_path, false).unwrap();

    let remote = match result.content {
      LockfileContent::V2(c) => c.remote,
      _ => unreachable!(),
    };
    let keys: Vec<String> = remote.keys().cloned().collect();
    let expected_keys = vec![
      String::from("https://deno.land/std@0.71.0/async/delay.ts"),
      String::from("https://deno.land/std@0.71.0/textproto/mod.ts"),
    ];

    assert_eq!(keys.len(), 2);
    assert_eq!(keys, expected_keys);
  }

  #[test]
  fn new_lockfile_from_file_and_insert() {
    let temp_dir = TempDir::new();
    let file_path = setup(&temp_dir);

    let mut lockfile = Lockfile::new(file_path, false).unwrap();

    lockfile.insert(
      "https://deno.land/std@0.71.0/io/util.ts",
      "Here is some source code",
    );

    let remote = match lockfile.content {
      LockfileContent::V2(c) => c.remote,
      _ => unreachable!(),
    };
    let keys: Vec<String> = remote.keys().cloned().collect();
    let expected_keys = vec![
      String::from("https://deno.land/std@0.71.0/async/delay.ts"),
      String::from("https://deno.land/std@0.71.0/io/util.ts"),
      String::from("https://deno.land/std@0.71.0/textproto/mod.ts"),
    ];
    assert_eq!(keys.len(), 3);
    assert_eq!(keys, expected_keys);
  }

  #[test]
  fn new_lockfile_and_write() {
    let temp_dir = TempDir::new();
    let file_path = setup(&temp_dir);

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
    let object = contents_json["remote"].as_object().unwrap();

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
  }

  #[test]
  fn check_or_insert_lockfile_false() {
    let temp_dir = TempDir::new();
    let file_path = setup(&temp_dir);

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
  }
}
