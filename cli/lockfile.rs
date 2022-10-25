// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use log::debug;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::npm::NpmPackageReq;
use crate::npm::NpmResolutionPackage;
use crate::tools::fmt::format_json;

#[derive(Debug)]
pub struct LockfileError(String);

impl std::fmt::Display for LockfileError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for LockfileError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmPackageInfo {
  pub integrity: String,
  pub dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NpmContent {
  /// Mapping between requests for npm packages and resolved specifiers, eg.
  /// {
  ///   "chalk": "chalk@5.0.0"
  ///   "react@17": "react@17.0.1"
  ///   "foo@latest": "foo@1.0.0"
  /// }
  pub specifiers: BTreeMap<String, String>,
  /// Mapping between resolved npm specifiers and their associated info, eg.
  /// {
  ///   "chalk@5.0.0": {
  ///     "integrity": "sha512-...",
  ///     "dependencies": {
  ///       "ansi-styles": "ansi-styles@4.1.0",
  ///     }
  ///   }
  /// }
  pub packages: BTreeMap<String, NpmPackageInfo>,
}

impl NpmContent {
  fn is_empty(&self) -> bool {
    self.specifiers.is_empty() && self.packages.is_empty()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockfileContent {
  version: String,
  // Mapping between URLs and their checksums for "http:" and "https:" deps
  remote: BTreeMap<String, String>,
  #[serde(skip_serializing_if = "NpmContent::is_empty")]
  #[serde(default)]
  pub npm: NpmContent,
}

#[derive(Debug, Clone)]
pub struct Lockfile {
  pub write: bool,
  pub content: LockfileContent,
  pub filename: PathBuf,
}

impl Lockfile {
  pub fn new(filename: PathBuf, write: bool) -> Result<Lockfile, AnyError> {
    // Writing a lock file always uses the new format.
    let content = if write {
      LockfileContent {
        version: "2".to_string(),
        remote: BTreeMap::new(),
        npm: NpmContent::default(),
      }
    } else {
      let s = std::fs::read_to_string(&filename).with_context(|| {
        format!("Unable to read lockfile: {}", filename.display())
      })?;
      let value: serde_json::Value = serde_json::from_str(&s)
        .context("Unable to parse contents of the lockfile")?;
      let version = value.get("version").and_then(|v| v.as_str());
      if version == Some("2") {
        serde_json::from_value::<LockfileContent>(value)
          .context("Unable to parse lockfile")?
      } else {
        // If there's no version field, we assume that user is using the old
        // version of the lockfile. We'll migrate it in-place into v2 and it
        // will be writte in v2 if user uses `--lock-write` flag.
        let remote: BTreeMap<String, String> =
          serde_json::from_value(value).context("Unable to parse lockfile")?;
        LockfileContent {
          version: "2".to_string(),
          remote,
          npm: NpmContent::default(),
        }
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

    let json_string = serde_json::to_string(&self.content).unwrap();
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

  // TODO(bartlomieju): this function should return an error instead of a bool,
  // but it requires changes to `deno_graph`'s `Locker`.
  pub fn check_or_insert_remote(
    &mut self,
    specifier: &str,
    code: &str,
  ) -> bool {
    if self.write {
      // In case --lock-write is specified check always passes
      self.insert(specifier, code);
      true
    } else {
      self.check(specifier, code)
    }
  }

  pub fn check_or_insert_npm_package(
    &mut self,
    package: &NpmResolutionPackage,
  ) -> Result<(), LockfileError> {
    if self.write {
      // In case --lock-write is specified check always passes
      self.insert_npm_package(package);
      Ok(())
    } else {
      self.check_npm_package(package)
    }
  }

  /// Checks the given module is included.
  /// Returns Ok(true) if check passed.
  fn check(&mut self, specifier: &str, code: &str) -> bool {
    if specifier.starts_with("file:") {
      return true;
    }
    if let Some(lockfile_checksum) = self.content.remote.get(specifier) {
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
    self.content.remote.insert(specifier.to_string(), checksum);
  }

  fn check_npm_package(
    &mut self,
    package: &NpmResolutionPackage,
  ) -> Result<(), LockfileError> {
    let specifier = package.id.serialize_for_lock_file();
    if let Some(package_info) = self.content.npm.packages.get(&specifier) {
      let integrity = package
        .dist
        .integrity
        .as_ref()
        .unwrap_or(&package.dist.shasum);
      if &package_info.integrity != integrity {
        return Err(LockfileError(format!(
          "Integrity check failed for npm package: \"{}\".
  Cache has \"{}\" and lockfile has \"{}\".
  Use \"--lock-write\" flag to update the lockfile.",
          package.id, integrity, package_info.integrity
        )));
      }
    }

    Ok(())
  }

  fn insert_npm_package(&mut self, package: &NpmResolutionPackage) {
    let dependencies = package
      .dependencies
      .iter()
      .map(|(name, id)| (name.to_string(), id.serialize_for_lock_file()))
      .collect::<BTreeMap<String, String>>();

    let integrity = package
      .dist
      .integrity
      .as_ref()
      .unwrap_or(&package.dist.shasum);
    self.content.npm.packages.insert(
      package.id.serialize_for_lock_file(),
      NpmPackageInfo {
        integrity: integrity.to_string(),
        dependencies,
      },
    );
  }

  pub fn insert_npm_specifier(
    &mut self,
    package_req: &NpmPackageReq,
    version: String,
  ) {
    if self.write {
      self.content.npm.specifiers.insert(
        package_req.to_string(),
        format!("{}@{}", package_req.name, version),
      );
    }
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
      lock_file.check_or_insert_remote(specifier.as_str(), source)
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
) -> Option<Rc<RefCell<dyn deno_graph::source::Locker>>> {
  lockfile.as_ref().map(|lf| {
    Rc::new(RefCell::new(Locker(Some(lf.clone()))))
      as Rc<RefCell<dyn deno_graph::source::Locker>>
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
      },
      "npm": {
        "specifiers": {},
        "packages": {}
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

    let remote = result.content.remote;
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

    let remote = lockfile.content.remote;
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

    let check_true = lockfile.check_or_insert_remote(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "Here is some source code",
    );
    assert!(check_true);

    let check_false = lockfile.check_or_insert_remote(
      "https://deno.land/std@0.71.0/textproto/mod.ts",
      "This is new Source code",
    );
    assert!(!check_false);
  }
}
