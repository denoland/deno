// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_cache_dir::npm::mixed_case_package_name_decode;
use deno_npm::NpmPackageCacheFolderId;
use deno_semver::package::PackageNv;
use deno_semver::StackString;

#[inline]
pub fn get_package_folder_id_folder_name(
  folder_id: &NpmPackageCacheFolderId,
) -> String {
  get_package_folder_id_folder_name_from_parts(
    &folder_id.nv,
    folder_id.copy_index,
  )
}

pub fn get_package_folder_id_folder_name_from_parts(
  nv: &PackageNv,
  copy_index: u8,
) -> String {
  let copy_str = if copy_index == 0 {
    Cow::Borrowed("")
  } else {
    Cow::Owned(format!("_{}", copy_index))
  };
  let name = normalize_pkg_name_for_node_modules_deno_folder(&nv.name);
  format!("{}@{}{}", name, nv.version, copy_str)
}

pub fn get_package_folder_id_from_folder_name(
  folder_name: &str,
) -> Option<NpmPackageCacheFolderId> {
  let folder_name = folder_name.replace('+', "/");
  let (name, ending) = folder_name.rsplit_once('@')?;
  let name: StackString = if let Some(encoded_name) = name.strip_prefix('_') {
    StackString::from_string(mixed_case_package_name_decode(encoded_name)?)
  } else {
    name.into()
  };
  let (raw_version, copy_index) = match ending.split_once('_') {
    Some((raw_version, copy_index)) => {
      let copy_index = copy_index.parse::<u8>().ok()?;
      (raw_version, copy_index)
    }
    None => (ending, 0),
  };
  let version = deno_semver::Version::parse_from_npm(raw_version).ok()?;
  Some(NpmPackageCacheFolderId {
    nv: PackageNv { name, version },
    copy_index,
  })
}

/// Normalizes a package name for use at `node_modules/.deno/<pkg-name>@<version>[_<copy_index>]`
pub fn normalize_pkg_name_for_node_modules_deno_folder(name: &str) -> Cow<str> {
  let name = if name.to_lowercase() == name {
    Cow::Borrowed(name)
  } else {
    Cow::Owned(format!("_{}", mixed_case_package_name_encode(name)))
  };
  if name.starts_with('@') {
    name.replace('/', "+").into()
  } else {
    name
  }
}

fn mixed_case_package_name_encode(name: &str) -> String {
  // use base32 encoding because it's reversible and the character set
  // only includes the characters within 0-9 and A-Z so it can be lower cased
  base32::encode(
    base32::Alphabet::Rfc4648Lower { padding: false },
    name.as_bytes(),
  )
  .to_lowercase()
}

#[cfg(test)]
mod test {
  use deno_npm::NpmPackageCacheFolderId;
  use deno_semver::package::PackageNv;

  use super::*;

  #[test]
  fn test_get_package_folder_id_folder_name() {
    let cases = vec![
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("@types/foo@1.2.3").unwrap(),
          copy_index: 1,
        },
        "@types+foo@1.2.3_1".to_string(),
      ),
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("JSON@3.2.1").unwrap(),
          copy_index: 0,
        },
        "_jjju6tq@3.2.1".to_string(),
      ),
    ];
    for (input, output) in cases {
      assert_eq!(get_package_folder_id_folder_name(&input), output);
      let folder_id = get_package_folder_id_from_folder_name(&output).unwrap();
      assert_eq!(folder_id, input);
    }
  }
}
