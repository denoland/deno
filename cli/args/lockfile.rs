// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use log::debug;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use crate::args::config_file::LockConfig;
use crate::args::ConfigFile;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmResolutionPackage;
use crate::tools::fmt::format_json;
use crate::util;
use crate::Flags;

use super::DenoSubcommand;

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
  pub fn discover(
    flags: &Flags,
    maybe_config_file: Option<&ConfigFile>,
  ) -> Result<Option<Lockfile>, AnyError> {
    if flags.no_lock
      || matches!(
        flags.subcommand,
        DenoSubcommand::Install(_) | DenoSubcommand::Uninstall(_)
      )
    {
      return Ok(None);
    }

    let filename = match flags.lock {
      Some(ref lock) => PathBuf::from(lock),
      None => match maybe_config_file {
        Some(config_file) => {
          if config_file.specifier.scheme() == "file" {
            match config_file.clone().to_lock_config()? {
              Some(LockConfig::Bool(lock)) if !lock => {
                return Ok(None);
              }
              Some(LockConfig::PathBuf(lock)) => config_file
                .specifier
                .to_file_path()
                .unwrap()
                .parent()
                .unwrap()
                .join(lock),
              _ => {
                let mut path = config_file.specifier.to_file_path().unwrap();
                path.set_file_name("deno.lock");
                path
              }
            }
          } else {
            return Ok(None);
          }
        }
        None => return Ok(None),
      },
    };

    let lockfile = Self::new(filename, flags.lock_write)?;
    Ok(Some(lockfile))
  }

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
    package: &NpmResolutionPackage,
  ) -> Result<(), LockfileError> {
    if self.overwrite {
      // In case --lock-write is specified check always passes
      self.insert_npm(package);
      Ok(())
    } else {
      self.check_or_insert_npm(package)
    }
  }

  /// Checks the given module is included, if so verify the checksum. If module
  /// is not included, insert it.
  fn check_or_insert(&mut self, specifier: &str, code: &str) -> bool {
    if let Some(lockfile_checksum) = self.content.remote.get(specifier) {
      let compiled_checksum = util::checksum::gen(&[code.as_bytes()]);
      lockfile_checksum == &compiled_checksum
    } else {
      self.insert(specifier, code);
      true
    }
  }

  fn insert(&mut self, specifier: &str, code: &str) {
    let checksum = util::checksum::gen(&[code.as_bytes()]);
    self.content.remote.insert(specifier.to_string(), checksum);
    self.has_content_changed = true;
  }

  fn check_or_insert_npm(
    &mut self,
    package: &NpmResolutionPackage,
  ) -> Result<(), LockfileError> {
    let specifier = package.id.as_serialized();
    if let Some(package_info) = self.content.npm.packages.get(&specifier) {
      let integrity = package
        .dist
        .integrity
        .as_ref()
        .unwrap_or(&package.dist.shasum);
      if &package_info.integrity != integrity {
        return Err(LockfileError(format!(
          "Integrity check failed for npm package: \"{}\". Unable to verify that the package
is the same as when the lockfile was generated.

This could be caused by:
  * the lock file may be corrupt
  * the source itself may be corrupt

Use \"--lock-write\" flag to regenerate the lockfile at \"{}\".",
          package.id.display(), self.filename.display()
        )));
      }
    } else {
      self.insert_npm(package);
    }

    Ok(())
  }

  fn insert_npm(&mut self, package: &NpmResolutionPackage) {
    let dependencies = package
      .dependencies
      .iter()
      .map(|(name, id)| (name.to_string(), id.as_serialized()))
      .collect::<BTreeMap<String, String>>();

    let integrity = package
      .dist
      .integrity
      .as_ref()
      .unwrap_or(&package.dist.shasum);
    self.content.npm.packages.insert(
      package.id.as_serialized(),
      NpmPackageInfo {
        integrity: integrity.to_string(),
        dependencies,
      },
    );
    self.has_content_changed = true;
  }

  pub fn insert_npm_specifier(
    &mut self,
    package_req: &NpmPackageReq,
    package_id: &NpmPackageId,
  ) {
    self
      .content
      .npm
      .specifiers
      .insert(package_req.to_string(), package_id.as_serialized());
    self.has_content_changed = true;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::npm::NpmPackageId;
  use crate::npm::NpmPackageVersionDistInfo;
  use crate::npm::NpmVersion;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use std::collections::HashMap;
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

    let npm_package = NpmResolutionPackage {
      id: NpmPackageId {
        name: "nanoid".to_string(),
        version: NpmVersion::parse("3.3.4").unwrap(),
        peer_dependencies: Vec::new(),
      },
      copy_index: 0,
      dist: NpmPackageVersionDistInfo {
        tarball: "foo".to_string(),
        shasum: "foo".to_string(),
        integrity: Some("sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==".to_string())
      },
      dependencies: HashMap::new(),
    };
    let check_ok = lockfile.check_or_insert_npm_package(&npm_package);
    assert!(check_ok.is_ok());

    let npm_package = NpmResolutionPackage {
      id: NpmPackageId {
        name: "picocolors".to_string(),
        version: NpmVersion::parse("1.0.0").unwrap(),
        peer_dependencies: Vec::new(),
      },
      copy_index: 0,
      dist: NpmPackageVersionDistInfo {
        tarball: "foo".to_string(),
        shasum: "foo".to_string(),
        integrity: Some("sha512-1fygroTLlHu66zi26VoTDv8yRgm0Fccecssto+MhsZ0D/DGW2sm8E8AjW7NU5VVTRt5GxbeZ5qBuJr+HyLYkjQ==".to_string())
      },
      dependencies: HashMap::new(),
    };
    // Integrity is borked in the loaded lockfile
    let check_err = lockfile.check_or_insert_npm_package(&npm_package);
    assert!(check_err.is_err());

    let npm_package = NpmResolutionPackage {
      id: NpmPackageId {
        name: "source-map-js".to_string(),
        version: NpmVersion::parse("1.0.2").unwrap(),
        peer_dependencies: Vec::new(),
      },
      copy_index: 0,
      dist: NpmPackageVersionDistInfo {
        tarball: "foo".to_string(),
        shasum: "foo".to_string(),
        integrity: Some("sha512-R0XvVJ9WusLiqTCEiGCmICCMplcCkIwwR11mOSD9CR5u+IXYdiseeEuXCVAjS54zqwkLcPNnmU4OeJ6tUrWhDw==".to_string())
      },
      dependencies: HashMap::new(),
    };
    // Not present in lockfile yet, should be inserted and check passed.
    let check_ok = lockfile.check_or_insert_npm_package(&npm_package);
    assert!(check_ok.is_ok());

    let npm_package = NpmResolutionPackage {
      id: NpmPackageId {
        name: "source-map-js".to_string(),
        version: NpmVersion::parse("1.0.2").unwrap(),
        peer_dependencies: Vec::new(),
      },
      copy_index: 0,
      dist: NpmPackageVersionDistInfo {
        tarball: "foo".to_string(),
        shasum: "foo".to_string(),
        integrity: Some("sha512-foobar".to_string()),
      },
      dependencies: HashMap::new(),
    };
    // Now present in lockfile, should file due to borked integrity
    let check_err = lockfile.check_or_insert_npm_package(&npm_package);
    assert!(check_err.is_err());
  }
}
