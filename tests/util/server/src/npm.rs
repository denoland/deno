// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use flate2::Compression;
use flate2::write::GzEncoder;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tar::Builder;

use crate::PathRef;
use crate::root_path;
use crate::tests_path;

pub const DENOTEST_SCOPE_NAME: &str = "@denotest";
pub const DENOTEST2_SCOPE_NAME: &str = "@denotest2";
pub const DENOTEST3_SCOPE_NAME: &str = "@denotest3";
pub const ESBUILD_VERSION: &str = "0.25.5";

pub static PUBLIC_TEST_NPM_REGISTRY: Lazy<TestNpmRegistry> = Lazy::new(|| {
  TestNpmRegistry::new(
    NpmRegistryKind::Public,
    &format!(
      "http://localhost:{}",
      crate::servers::PUBLIC_NPM_REGISTRY_PORT
    ),
    "npm",
  )
});

pub static PRIVATE_TEST_NPM_REGISTRY_1: Lazy<TestNpmRegistry> =
  Lazy::new(|| {
    TestNpmRegistry::new(
      NpmRegistryKind::Private,
      &format!(
        "http://localhost:{}",
        crate::servers::PRIVATE_NPM_REGISTRY_1_PORT
      ),
      "npm-private",
    )
  });

pub static PRIVATE_TEST_NPM_REGISTRY_2: Lazy<TestNpmRegistry> =
  Lazy::new(|| {
    TestNpmRegistry::new(
      NpmRegistryKind::Private,
      &format!(
        "http://localhost:{}",
        crate::servers::PRIVATE_NPM_REGISTRY_2_PORT
      ),
      "npm-private2",
    )
  });

pub static PRIVATE_TEST_NPM_REGISTRY_3: Lazy<TestNpmRegistry> =
  Lazy::new(|| {
    TestNpmRegistry::new(
      NpmRegistryKind::Private,
      &format!(
        "http://localhost:{}",
        crate::servers::PRIVATE_NPM_REGISTRY_3_PORT
      ),
      "npm-private3",
    )
  });

pub enum NpmRegistryKind {
  Public,
  Private,
}

struct CustomNpmPackage {
  pub registry_file: String,
  pub tarballs: HashMap<String, Vec<u8>>,
}

/// Creates tarballs and a registry json file for npm packages
/// in the `tests/registry/npm/@denotest` directory.
pub struct TestNpmRegistry {
  #[allow(unused)]
  kind: NpmRegistryKind,
  // Eg. http://localhost:4544/
  hostname: String,
  /// Path in the tests/registry folder (Eg. npm)
  local_path: String,

  cache: Mutex<HashMap<String, CustomNpmPackage>>,
}

impl TestNpmRegistry {
  pub fn new(kind: NpmRegistryKind, hostname: &str, local_path: &str) -> Self {
    let hostname = hostname.strip_suffix('/').unwrap_or(hostname).to_string();

    Self {
      hostname,
      local_path: local_path.to_string(),
      kind,
      cache: Default::default(),
    }
  }

  pub fn root_dir(&self) -> PathRef {
    tests_path().join("registry").join(&self.local_path)
  }

  pub fn tarball_bytes(
    &self,
    name: &str,
    version: &str,
  ) -> Result<Option<Vec<u8>>> {
    Ok(
      self
        .get_package_property(name, |p| p.tarballs.get(version).cloned())?
        .flatten(),
    )
  }

  pub fn registry_file(&self, name: &str) -> Result<Option<Vec<u8>>> {
    self.get_package_property(name, |p| p.registry_file.as_bytes().to_vec())
  }

  pub fn package_url(&self, package_name: &str) -> String {
    let scheme = if self.hostname.starts_with("http://") {
      ""
    } else {
      "http://"
    };
    format!("{}{}/{}/", scheme, self.hostname, package_name)
  }

  fn get_package_property<TResult>(
    &self,
    package_name: &str,
    func: impl FnOnce(&CustomNpmPackage) -> TResult,
  ) -> Result<Option<TResult>> {
    // it's ok if multiple threads race here as they will do the same work twice
    if !self.cache.lock().contains_key(package_name) {
      match get_npm_package(&self.hostname, &self.local_path, package_name)? {
        Some(package) => {
          self.cache.lock().insert(package_name.to_string(), package);
        }
        None => return Ok(None),
      }
    }
    Ok(self.cache.lock().get(package_name).map(func))
  }

  pub fn get_test_scope_and_package_name_with_path_from_uri_path<'s>(
    &self,
    uri_path: &'s str,
  ) -> Option<(&'s str, &'s str)> {
    let prefix1 = format!("/{}/", DENOTEST_SCOPE_NAME);
    let prefix2 = format!("/{}%2f", DENOTEST_SCOPE_NAME);

    let maybe_package_name_with_path = uri_path
      .strip_prefix(&prefix1)
      .or_else(|| uri_path.strip_prefix(&prefix2));

    if let Some(package_name_with_path) = maybe_package_name_with_path {
      return Some((DENOTEST_SCOPE_NAME, package_name_with_path));
    }

    let prefix1 = format!("/{}/", DENOTEST2_SCOPE_NAME);
    let prefix2 = format!("/{}%2f", DENOTEST2_SCOPE_NAME);

    let maybe_package_name_with_path = uri_path
      .strip_prefix(&prefix1)
      .or_else(|| uri_path.strip_prefix(&prefix2));

    if let Some(package_name_with_path) = maybe_package_name_with_path {
      return Some((DENOTEST2_SCOPE_NAME, package_name_with_path));
    }

    let prefix1 = format!("/{}/", DENOTEST3_SCOPE_NAME);
    let prefix2 = format!("/{}%2f", DENOTEST3_SCOPE_NAME);

    let maybe_package_name_with_path = uri_path
      .strip_prefix(&prefix1)
      .or_else(|| uri_path.strip_prefix(&prefix2));

    if let Some(package_name_with_path) = maybe_package_name_with_path {
      return Some((DENOTEST3_SCOPE_NAME, package_name_with_path));
    }

    let prefix1 = format!("/{}/", "@types");
    let prefix2 = format!("/{}%2f", "@types");
    let maybe_package_name_with_path = uri_path
      .strip_prefix(&prefix1)
      .or_else(|| uri_path.strip_prefix(&prefix2));
    if let Some(package_name_with_path) = maybe_package_name_with_path
      && package_name_with_path.starts_with("denotest")
    {
      return Some(("@types", package_name_with_path));
    }

    let prefix1 = format!("/{}/", "@esbuild");
    let prefix2 = format!("/{}%2f", "@esbuild");
    let maybe_package_name_with_path = uri_path
      .strip_prefix(&prefix1)
      .or_else(|| uri_path.strip_prefix(&prefix2));
    if let Some(package_name_with_path) = maybe_package_name_with_path {
      return Some(("@esbuild", package_name_with_path));
    }

    None
  }
}

// NOTE: extracted out partially from the `tar` crate, all credits to the original authors
fn append_dir_all<W: std::io::Write>(
  builder: &mut tar::Builder<W>,
  path: &Path,
  src_path: &Path,
) -> Result<()> {
  builder.follow_symlinks(true);
  let mode = tar::HeaderMode::Deterministic;
  builder.mode(mode);
  let mut stack = vec![(src_path.to_path_buf(), true, false)];
  let mut entries = Vec::new();
  while let Some((src, is_dir, is_symlink)) = stack.pop() {
    let dest = path.join(src.strip_prefix(src_path).unwrap());
    // In case of a symlink pointing to a directory, is_dir is false, but src.is_dir() will return true
    if is_dir || (is_symlink && src.is_dir()) {
      for entry in fs::read_dir(&src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        stack.push((entry.path(), file_type.is_dir(), file_type.is_symlink()));
      }
      if dest != Path::new("") {
        entries.push((src, dest));
      }
    } else {
      entries.push((src, dest));
    }
  }
  entries.sort_by(|(_, a), (_, b)| a.cmp(b));
  for (src, dest) in entries {
    let mut header = tar::Header::new_gnu();
    let metadata = src.metadata().with_context(|| {
      format!("trying to get metadata for {}", src.display())
    })?;
    header.set_metadata_in_mode(&metadata, mode);
    // this is what `tar` sets the mtime to on unix in deterministic mode, on windows it uses a different
    // value, which causes the tarball to have a different hash on windows. force it to be the same
    // to ensure the same output on all platforms
    header.set_mtime(1153704088);

    let data = if src.is_file() {
      Box::new(
        fs::File::open(&src)
          .with_context(|| format!("trying to open file {}", src.display()))?,
      ) as Box<dyn std::io::Read>
    } else {
      Box::new(std::io::empty()) as Box<dyn std::io::Read>
    };
    builder
      .append_data(&mut header, dest, data)
      .with_context(|| "appending data")?;
  }
  Ok(())
}

fn create_package_version_info(
  version_folder: &PathRef,
  version: &str,
  package_name: &str,
  registry_hostname: &str,
) -> Result<(Vec<u8>, serde_json::Map<String, serde_json::Value>)> {
  let tarball_bytes = create_tarball_from_dir(version_folder.as_path())?;

  let mut dist = serde_json::Map::new();
  if package_name != "@denotest/no-shasums" {
    let tarball_checksum = get_tarball_checksum(&tarball_bytes);
    dist.insert(
      "integrity".to_string(),
      format!("sha512-{tarball_checksum}").into(),
    );
    dist.insert("shasum".to_string(), "dummy-value".into());
  }
  dist.insert(
    "tarball".to_string(),
    format!("{registry_hostname}/{package_name}/{version}.tgz").into(),
  );

  let package_json_path = version_folder.join("package.json");
  let package_json_bytes = fs::read(&package_json_path).with_context(|| {
    format!("Error reading package.json at {}", package_json_path)
  })?;
  let package_json_text = String::from_utf8_lossy(&package_json_bytes);
  let mut version_info: serde_json::Map<String, serde_json::Value> =
    serde_json::from_str(&package_json_text)?;
  version_info.insert("dist".to_string(), dist.into());

  // add a bin entry for a directories.bin package.json entry as this
  // is what the npm registry does as well
  if let Some(directories) = version_info.get("directories")
    && !version_info.contains_key("bin")
    && let Some(bin) = directories
      .as_object()
      .and_then(|o| o.get("bin"))
      .and_then(|v| v.as_str())
  {
    let mut bins = serde_json::Map::new();
    for entry in std::fs::read_dir(version_folder.join(bin))? {
      let entry = entry?;
      let file_name = entry.file_name().to_string_lossy().to_string();
      bins.insert(
        file_name.to_string(),
        format!("{}/{}", bin, file_name).into(),
      );
    }
    version_info.insert("bin".into(), bins.into());
  }

  Ok((tarball_bytes, version_info))
}

fn get_esbuild_platform_info(
  platform_name: &str,
) -> Option<(&'static str, &'static str, bool)> {
  match platform_name {
    "linux-x64" => Some(("esbuild-x64", "linux64", false)),
    "linux-arm64" => Some(("esbuild-aarch64", "linux64", false)),
    "darwin-x64" => Some(("esbuild-x64", "mac", false)),
    "darwin-arm64" => Some(("esbuild-aarch64", "mac", false)),
    "win32-x64" => Some(("esbuild-x64.exe", "win", true)),
    "win32-arm64" => Some(("esbuild-arm64.exe", "win", true)),
    _ => None,
  }
}

fn setup_esbuild_binary(
  package_dir: &Path,
  esbuild_prebuilt: &Path,
  is_windows: bool,
) -> Result<&'static str> {
  let binary_name = if is_windows { "esbuild.exe" } else { "esbuild" };

  if is_windows {
    std::fs::copy(esbuild_prebuilt, package_dir.join(binary_name))?;
    Ok(binary_name)
  } else {
    let bin_dir = package_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    let binary_path = bin_dir.join(binary_name);
    std::fs::copy(esbuild_prebuilt, &binary_path)?;

    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(&binary_path)?.permissions();
      perms.set_mode(0o755); // rwxr-xr-x
      std::fs::set_permissions(&binary_path, perms)?;
    }

    Ok("bin/esbuild")
  }
}

fn create_tarball_from_dir(package_dir: &Path) -> Result<Vec<u8>> {
  let mut tarball_bytes = Vec::new();
  {
    let mut encoder =
      GzEncoder::new(&mut tarball_bytes, Compression::default());
    {
      let mut builder = Builder::new(&mut encoder);
      append_dir_all(&mut builder, Path::new("package"), package_dir)?;
      builder.finish()?;
    }
    encoder.finish()?;
  }
  Ok(tarball_bytes)
}

fn create_npm_registry_response(
  package_name: &str,
  version: &str,
  description: &str,
  bin_path: &str,
  tarball_bytes: Vec<u8>,
  registry_hostname: &str,
) -> Result<CustomNpmPackage> {
  let tarball_checksum = get_tarball_checksum(&tarball_bytes);
  let mut dist = serde_json::Map::new();
  dist.insert(
    "integrity".to_string(),
    format!("sha512-{tarball_checksum}").into(),
  );
  dist.insert("shasum".to_string(), "dummy-value".into());
  dist.insert(
    "tarball".to_string(),
    format!("{registry_hostname}/{package_name}/{version}.tgz").into(),
  );

  let mut version_info = serde_json::Map::new();
  version_info.insert("name".to_string(), package_name.into());
  version_info.insert("version".to_string(), version.into());
  version_info.insert("description".to_string(), description.into());
  version_info.insert("bin".to_string(), bin_path.into());
  version_info.insert("dist".to_string(), dist.into());

  let mut versions = serde_json::Map::new();
  versions.insert(version.to_string(), version_info.into());

  let mut dist_tags = serde_json::Map::new();
  dist_tags.insert("latest".to_string(), version.into());

  let mut registry_file = serde_json::Map::new();
  registry_file.insert("name".to_string(), package_name.into());
  registry_file.insert("versions".to_string(), versions.into());
  registry_file.insert("dist-tags".to_string(), dist_tags.into());

  let mut tarballs = HashMap::new();
  tarballs.insert(version.to_string(), tarball_bytes);

  Ok(CustomNpmPackage {
    registry_file: serde_json::to_string(&registry_file)?,
    tarballs,
  })
}

fn create_esbuild_package(
  registry_hostname: &str,
  package_name: &str,
) -> Result<Option<CustomNpmPackage>> {
  let platform_name = package_name.strip_prefix("@esbuild/").unwrap();

  let (bin_name, folder, is_windows) =
    match get_esbuild_platform_info(platform_name) {
      Some(info) => info,
      None => return Ok(None),
    };

  let esbuild_prebuilt = root_path()
    .join("third_party/prebuilt")
    .join(folder)
    .join(bin_name);

  if !esbuild_prebuilt.exists() {
    return Ok(None);
  }

  let temp_dir = tempfile::tempdir()?;
  let package_dir = temp_dir.path().join("package");
  std::fs::create_dir_all(&package_dir)?;

  let bin_path =
    setup_esbuild_binary(&package_dir, esbuild_prebuilt.as_path(), is_windows)?;

  let package_json = serde_json::json!({
    "name": package_name,
    "version": ESBUILD_VERSION,
    "description": format!("The {} binary for esbuild", platform_name),
    "bin": bin_path
  });

  std::fs::write(
    package_dir.join("package.json"),
    serde_json::to_string_pretty(&package_json)?,
  )?;

  let tarball_bytes = create_tarball_from_dir(&package_dir)?;
  let package = create_npm_registry_response(
    package_name,
    ESBUILD_VERSION,
    &format!("The {} binary for esbuild", platform_name),
    bin_path,
    tarball_bytes,
    registry_hostname,
  )?;

  Ok(Some(package))
}

fn get_npm_package(
  registry_hostname: &str,
  local_path: &str,
  package_name: &str,
) -> Result<Option<CustomNpmPackage>> {
  if package_name.starts_with("@esbuild/")
    && let Some(esbuild_package) =
      create_esbuild_package(registry_hostname, package_name)?
  {
    return Ok(Some(esbuild_package));
  }

  let registry_hostname = if package_name == "@denotest/tarballs-privateserver2"
  {
    "http://localhost:4262"
  } else {
    registry_hostname
  };
  let package_folder = tests_path()
    .join("registry")
    .join(local_path)
    .join(package_name);
  if !package_folder.exists() {
    return Ok(None);
  }

  // read all the package's versions
  let mut tarballs = HashMap::new();
  let mut versions = serde_json::Map::new();
  let mut latest_version = semver::Version::parse("0.0.0").unwrap();
  let mut dist_tags = serde_json::Map::new();
  let mut time = serde_json::Map::new();
  for entry in fs::read_dir(&package_folder)? {
    let entry = entry?;
    let file_type = entry.file_type()?;
    if !file_type.is_dir() {
      continue;
    }
    let version = entry.file_name().to_string_lossy().into_owned();
    let version_folder = package_folder.join(&version);

    let (tarball_bytes, mut version_info) = create_package_version_info(
      &version_folder,
      &version,
      package_name,
      registry_hostname,
    )?;

    tarballs.insert(version.clone(), tarball_bytes);

    if let Some(maybe_optional_deps) = version_info.get("optionalDependencies")
      && let Some(optional_deps) = maybe_optional_deps.as_object()
    {
      if let Some(maybe_deps) = version_info.get("dependencies") {
        if let Some(deps) = maybe_deps.as_object() {
          let mut cloned_deps = deps.to_owned();
          for (key, value) in optional_deps {
            cloned_deps.insert(key.to_string(), value.to_owned());
          }
          version_info.insert(
            "dependencies".to_string(),
            serde_json::to_value(cloned_deps).unwrap(),
          );
        }
      } else {
        version_info.insert(
          "dependencies".to_string(),
          serde_json::to_value(optional_deps).unwrap(),
        );
      }
    }

    if let Some(publish_config) = version_info.get("publishConfig")
      && let Some(tag) = publish_config.get("tag")
      && let Some(tag) = tag.as_str()
    {
      dist_tags.insert(tag.to_string(), version.clone().into());
    }

    if let Some(date) = version_info.get("publishDate") {
      time.insert(version.clone(), date.clone());
    }

    versions.insert(version.clone(), version_info.into());
    let version = semver::Version::parse(&version)?;
    if version.cmp(&latest_version).is_gt() {
      latest_version = version;
    }
  }

  if !dist_tags.contains_key("latest") {
    dist_tags.insert("latest".to_string(), latest_version.to_string().into());
  }

  // create the registry file for this package
  let mut registry_file = serde_json::Map::new();
  registry_file.insert("name".to_string(), package_name.to_string().into());
  registry_file.insert("versions".to_string(), versions.into());
  registry_file.insert("dist-tags".to_string(), dist_tags.into());
  registry_file.insert("time".to_string(), time.into());
  Ok(Some(CustomNpmPackage {
    registry_file: serde_json::to_string(&registry_file).unwrap(),
    tarballs,
  }))
}

fn get_tarball_checksum(bytes: &[u8]) -> String {
  use sha2::Digest;
  let mut hasher = sha2::Sha512::new();
  hasher.update(bytes);
  BASE64_STANDARD.encode(hasher.finalize())
}
