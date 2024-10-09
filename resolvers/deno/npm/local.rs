// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;

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
