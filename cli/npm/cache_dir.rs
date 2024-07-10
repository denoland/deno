// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::NpmPackageCacheFolderId;
use deno_semver::package::PackageNv;
use deno_semver::Version;

use crate::util::fs::canonicalize_path;
use crate::util::path::root_url_to_safe_local_dirname;

/// The global cache directory of npm packages.
#[derive(Clone, Debug)]
pub struct NpmCacheDir {
  root_dir: PathBuf,
  // cached url representation of the root directory
  root_dir_url: Url,
  // A list of all registry that were discovered via `.npmrc` files
  // turned into a safe directory names.
  known_registries_dirnames: Vec<String>,
}

impl NpmCacheDir {
  pub fn new(root_dir: PathBuf, known_registries_urls: Vec<Url>) -> Self {
    fn try_get_canonicalized_root_dir(
      root_dir: &Path,
    ) -> Result<PathBuf, AnyError> {
      if !root_dir.exists() {
        std::fs::create_dir_all(root_dir)
          .with_context(|| format!("Error creating {}", root_dir.display()))?;
      }
      Ok(canonicalize_path(root_dir)?)
    }

    // this may fail on readonly file systems, so just ignore if so
    let root_dir =
      try_get_canonicalized_root_dir(&root_dir).unwrap_or(root_dir);
    let root_dir_url = Url::from_directory_path(&root_dir).unwrap();

    let known_registries_dirnames: Vec<_> = known_registries_urls
      .into_iter()
      .map(|url| {
        root_url_to_safe_local_dirname(&url)
          .to_string_lossy()
          .replace('\\', "/")
      })
      .collect();

    Self {
      root_dir,
      root_dir_url,
      known_registries_dirnames,
    }
  }

  pub fn root_dir(&self) -> &Path {
    &self.root_dir
  }

  pub fn root_dir_url(&self) -> &Url {
    &self.root_dir_url
  }

  pub fn package_folder_for_id(
    &self,
    folder_id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> PathBuf {
    if folder_id.copy_index == 0 {
      self.package_folder_for_nv(&folder_id.nv, registry_url)
    } else {
      self
        .package_name_folder(&folder_id.nv.name, registry_url)
        .join(format!("{}_{}", folder_id.nv.version, folder_id.copy_index))
    }
  }

  pub fn package_folder_for_nv(
    &self,
    package: &PackageNv,
    registry_url: &Url,
  ) -> PathBuf {
    self
      .package_name_folder(&package.name, registry_url)
      .join(package.version.to_string())
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    let mut dir = self.registry_folder(registry_url);
    if name.to_lowercase() != name {
      let encoded_name = mixed_case_package_name_encode(name);
      // Using the encoded directory may have a collision with an actual package name
      // so prefix it with an underscore since npm packages can't start with that
      dir.join(format!("_{encoded_name}"))
    } else {
      // ensure backslashes are used on windows
      for part in name.split('/') {
        dir = dir.join(part);
      }
      dir
    }
  }

  fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self
      .root_dir
      .join(root_url_to_safe_local_dirname(registry_url))
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<NpmPackageCacheFolderId> {
    let mut maybe_relative_url = None;

    // Iterate through known registries and try to get a match.
    for registry_dirname in &self.known_registries_dirnames {
      let registry_root_dir = self
        .root_dir_url
        .join(&format!("{}/", registry_dirname))
        // this not succeeding indicates a fatal issue, so unwrap
        .unwrap();

      let Some(relative_url) = registry_root_dir.make_relative(specifier)
      else {
        continue;
      };

      if relative_url.starts_with("../") {
        continue;
      }

      maybe_relative_url = Some(relative_url);
      break;
    }

    let mut relative_url = maybe_relative_url?;

    // base32 decode the url if it starts with an underscore
    // * Ex. _{base32(package_name)}/
    if let Some(end_url) = relative_url.strip_prefix('_') {
      let mut parts = end_url
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
      match mixed_case_package_name_decode(&parts[0]) {
        Some(part) => {
          parts[0] = part;
        }
        None => return None,
      }
      relative_url = parts.join("/");
    }

    // examples:
    // * chalk/5.0.1/
    // * @types/chalk/5.0.1/
    // * some-package/5.0.1_1/ -- where the `_1` (/_\d+/) is a copy of the folder for peer deps
    let is_scoped_package = relative_url.starts_with('@');
    let mut parts = relative_url
      .split('/')
      .enumerate()
      .take(if is_scoped_package { 3 } else { 2 })
      .map(|(_, part)| part)
      .collect::<Vec<_>>();
    if parts.len() < 2 {
      return None;
    }
    let version_part = parts.pop().unwrap();
    let name = parts.join("/");
    let (version, copy_index) =
      if let Some((version, copy_count)) = version_part.split_once('_') {
        (version, copy_count.parse::<u8>().ok()?)
      } else {
        (version_part, 0)
      };
    Some(NpmPackageCacheFolderId {
      nv: PackageNv {
        name,
        version: Version::parse_from_npm(version).ok()?,
      },
      copy_index,
    })
  }

  pub fn get_cache_location(&self) -> PathBuf {
    self.root_dir.clone()
  }
}

pub fn mixed_case_package_name_encode(name: &str) -> String {
  // use base32 encoding because it's reversible and the character set
  // only includes the characters within 0-9 and A-Z so it can be lower cased
  base32::encode(
    base32::Alphabet::RFC4648 { padding: false },
    name.as_bytes(),
  )
  .to_lowercase()
}

pub fn mixed_case_package_name_decode(name: &str) -> Option<String> {
  base32::decode(base32::Alphabet::RFC4648 { padding: false }, name)
    .and_then(|b| String::from_utf8(b).ok())
}

#[cfg(test)]
mod test {
  use deno_core::url::Url;
  use deno_semver::package::PackageNv;
  use deno_semver::Version;

  use super::NpmCacheDir;
  use crate::npm::cache_dir::NpmPackageCacheFolderId;

  #[test]
  fn should_get_package_folder() {
    let deno_dir = crate::cache::DenoDir::new(None).unwrap();
    let root_dir = deno_dir.npm_folder_path();
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();
    let cache = NpmCacheDir::new(root_dir.clone(), vec![registry_url.clone()]);

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          nv: PackageNv {
            name: "json".to_string(),
            version: Version::parse_from_npm("1.2.5").unwrap(),
          },
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          nv: PackageNv {
            name: "json".to_string(),
            version: Version::parse_from_npm("1.2.5").unwrap(),
          },
          copy_index: 1,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5_1"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          nv: PackageNv {
            name: "JSON".to_string(),
            version: Version::parse_from_npm("2.1.5").unwrap(),
          },
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_jjju6tq")
        .join("2.1.5"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          nv: PackageNv {
            name: "@types/JSON".to_string(),
            version: Version::parse_from_npm("2.1.5").unwrap(),
          },
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_ib2hs4dfomxuuu2pjy")
        .join("2.1.5"),
    );
  }
}
