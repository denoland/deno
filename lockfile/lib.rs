// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::io::Write;

use anyhow::Context;
use anyhow::Error as AnyError;
use ring::digest;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

pub struct NpmPackageLockfileInfo {
  pub display_id: String,
  pub serialized_id: String,
  pub integrity: String,
  pub dependencies: Vec<NpmPackageDependencyLockfileInfo>,
}

pub struct NpmPackageDependencyLockfileInfo {
  pub name: String,
  pub id: String,
}

fn gen_checksum(v: &[impl AsRef<[u8]>]) -> String {
  let mut ctx = digest::Context::new(&digest::SHA256);
  for src in v {
    ctx.update(src.as_ref());
  }
  let digest = ctx.finish();
  let out: Vec<String> = digest
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
  out.join("")
}

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
  /// Mapping between requests for npm packages and resolved packages, eg.
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

impl LockfileContent {
  fn empty() -> Self {
    Self {
      version: "2".to_string(),
      remote: BTreeMap::new(),
      npm: NpmContent::default(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Lockfile {
  pub overwrite: bool,
  pub has_content_changed: bool,
  pub content: LockfileContent,
  pub filename: PathBuf,
}

impl Lockfile {
  pub fn new(filename: PathBuf, overwrite: bool) -> Result<Lockfile, AnyError> {
    // Writing a lock file always uses the new format.
    if overwrite {
      return Ok(Lockfile {
        overwrite,
        has_content_changed: false,
        content: LockfileContent::empty(),
        filename,
      });
    }

    let result = match std::fs::read_to_string(&filename) {
      Ok(content) => Ok(content),
      Err(e) => {
        if e.kind() == std::io::ErrorKind::NotFound {
          return Ok(Lockfile {
            overwrite,
            has_content_changed: false,
            content: LockfileContent::empty(),
            filename,
          });
        } else {
          Err(e)
        }
      }
    };

    let s = result.with_context(|| {
      format!("Unable to read lockfile: \"{}\"", filename.display())
    })?;
    let value: serde_json::Value =
      serde_json::from_str(&s).with_context(|| {
        format!(
          "Unable to parse contents of the lockfile \"{}\"",
          filename.display()
        )
      })?;
    let version = value.get("version").and_then(|v| v.as_str());
    let content = if version == Some("2") {
      serde_json::from_value::<LockfileContent>(value).with_context(|| {
        format!(
          "Unable to parse contents of the lockfile \"{}\"",
          filename.display()
        )
      })?
    } else {
      // If there's no version field, we assume that user is using the old
      // version of the lockfile. We'll migrate it in-place into v2 and it
      // will be writte in v2 if user uses `--lock-write` flag.
      let remote: BTreeMap<String, String> = serde_json::from_value(value)
        .with_context(|| {
          format!(
            "Unable to parse contents of the lockfile \"{}\"",
            filename.display()
          )
        })?;
      LockfileContent {
        version: "2".to_string(),
        remote,
        npm: NpmContent::default(),
      }
    };

    Ok(Lockfile {
      overwrite,
      has_content_changed: false,
      content,
      filename,
    })
  }

  // Synchronize lock file to disk - noop if --lock-write file is not specified.
  pub fn write(&self) -> Result<(), AnyError> {
    if !self.has_content_changed && !self.overwrite {
      return Ok(());
    }

    let mut json_string = serde_json::to_string_pretty(&self.content).unwrap();
    json_string.push('\n'); // trailing newline in file
    let mut f = std::fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .open(&self.filename)?;
    f.write_all(json_string.as_bytes())?;
    Ok(())
  }

  // TODO(bartlomieju): this function should return an error instead of a bool,
  // but it requires changes to `deno_graph`'s `Locker`.
  pub fn check_or_insert_remote(
    &mut self,
    specifier: &str,
    code: &str,
  ) -> bool {
    if !(specifier.starts_with("http:") || specifier.starts_with("https:")) {
      return true;
    }
    if self.overwrite {
      // In case --lock-write is specified check always passes
      self.insert(specifier, code);
      true
    } else {
      self.check_or_insert(specifier, code)
    }
  }

  pub fn check_or_insert_npm_package(
    &mut self,
    package_info: NpmPackageLockfileInfo,
  ) -> Result<(), LockfileError> {
    if self.overwrite {
      // In case --lock-write is specified check always passes
      self.insert_npm(package_info);
      Ok(())
    } else {
      self.check_or_insert_npm(package_info)
    }
  }

  /// Checks the given module is included, if so verify the checksum. If module
  /// is not included, insert it.
  fn check_or_insert(&mut self, specifier: &str, code: &str) -> bool {
    if let Some(lockfile_checksum) = self.content.remote.get(specifier) {
      let compiled_checksum = gen_checksum(&[code.as_bytes()]);
      lockfile_checksum == &compiled_checksum
    } else {
      self.insert(specifier, code);
      true
    }
  }

  fn insert(&mut self, specifier: &str, code: &str) {
    let checksum = gen_checksum(&[code.as_bytes()]);
    self.content.remote.insert(specifier.to_string(), checksum);
    self.has_content_changed = true;
  }

  fn check_or_insert_npm(
    &mut self,
    package: NpmPackageLockfileInfo,
  ) -> Result<(), LockfileError> {
    if let Some(package_info) =
      self.content.npm.packages.get(&package.serialized_id)
    {
      if package_info.integrity.as_str() != package.integrity {
        return Err(LockfileError(format!(
            "Integrity check failed for npm package: \"{}\". Unable to verify that the package
is the same as when the lockfile was generated.

This could be caused by:
  * the lock file may be corrupt
  * the source itself may be corrupt

Use \"--lock-write\" flag to regenerate the lockfile at \"{}\".",
            package.display_id, self.filename.display()
          )));
      }
    } else {
      self.insert_npm(package);
    }

    Ok(())
  }

  fn insert_npm(&mut self, package_info: NpmPackageLockfileInfo) {
    let dependencies = package_info
      .dependencies
      .iter()
      .map(|dep| (dep.name.to_string(), dep.id.to_string()))
      .collect::<BTreeMap<String, String>>();

    self.content.npm.packages.insert(
      package_info.serialized_id.to_string(),
      NpmPackageInfo {
        integrity: package_info.integrity,
        dependencies,
      },
    );
    self.has_content_changed = true;
  }

  pub fn insert_npm_specifier(
    &mut self,
    serialized_package_req: String,
    serialized_package_id: String,
  ) {
    self
      .content
      .npm
      .specifiers
      .insert(serialized_package_req, serialized_package_id);
    self.has_content_changed = true;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
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
        "packages": {
          "nanoid@3.3.4": {
            "integrity": "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==",
            "dependencies": {}
          },
          "picocolors@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          },
        }
      }
    });

    file.write_all(value.to_string().as_bytes()).unwrap();

    temp_dir.path().join("valid_lockfile.json")
  }

  #[test]
  fn create_lockfile_for_nonexistent_path() {
    let file_path = PathBuf::from("nonexistent_lock_file.json");
    assert!(Lockfile::new(file_path, false).is_ok());
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
  fn check_or_insert_lockfile() {
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
      "Here is some NEW source code",
    );
    assert!(!check_false);

    // Not present in lockfile yet, should be inserted and check passed.
    let check_true = lockfile.check_or_insert_remote(
      "https://deno.land/std@0.71.0/http/file_server.ts",
      "This is new Source code",
    );
    assert!(check_true);
  }

  #[test]
  fn check_or_insert_lockfile_npm() {
    let temp_dir = TempDir::new();
    let file_path = setup(&temp_dir);

    let mut lockfile = Lockfile::new(file_path, false).unwrap();

    let npm_package = NpmPackageLockfileInfo {
      display_id: "nanoid@3.3.4".to_string(),
      serialized_id: "nanoid@3.3.4".to_string(),
      integrity: "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==".to_string(),
      dependencies: vec![],
    };
    let check_ok = lockfile.check_or_insert_npm_package(npm_package);
    assert!(check_ok.is_ok());

    let npm_package = NpmPackageLockfileInfo {
      display_id: "picocolors@1.0.0".to_string(),
      serialized_id: "picocolors@1.0.0".to_string(),
      integrity: "sha512-1fygroTLlHu66zi26VoTDv8yRgm0Fccecssto+MhsZ0D/DGW2sm8E8AjW7NU5VVTRt5GxbeZ5qBuJr+HyLYkjQ==".to_string(),
      dependencies: vec![],
    };
    // Integrity is borked in the loaded lockfile
    let check_err = lockfile.check_or_insert_npm_package(npm_package);
    assert!(check_err.is_err());

    let npm_package = NpmPackageLockfileInfo {
      display_id: "source-map-js@1.0.2".to_string(),
      serialized_id: "source-map-js@1.0.2".to_string(),
      integrity: "sha512-R0XvVJ9WusLiqTCEiGCmICCMplcCkIwwR11mOSD9CR5u+IXYdiseeEuXCVAjS54zqwkLcPNnmU4OeJ6tUrWhDw==".to_string(),
      dependencies: vec![],
    };
    // Not present in lockfile yet, should be inserted and check passed.
    let check_ok = lockfile.check_or_insert_npm_package(npm_package);
    assert!(check_ok.is_ok());

    let npm_package = NpmPackageLockfileInfo {
      display_id: "source-map-js@1.0.2".to_string(),
      serialized_id: "source-map-js@1.0.2".to_string(),
      integrity: "sha512-foobar".to_string(),
      dependencies: vec![],
    };
    // Now present in lockfile, should file due to borked integrity
    let check_err = lockfile.check_or_insert_npm_package(npm_package);
    assert!(check_err.is_err());
  }
}
