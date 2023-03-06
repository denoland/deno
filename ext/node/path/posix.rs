// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;

use super::char_at_index;
use super::constants;

#[op]
pub fn op_node_path_posix_dirname(path: &str) -> Result<String, AnyError> {
  let has_root = match char_at_index(path, 0) {
    Some(char) => char as u32 == constants::CHAR_FORWARD_SLASH,
    None => return Ok(String::from(".")),
  };

  let end = match path.rfind('/') {
    Some(0) => None,
    Some(n) => Some(n),
    _ => None,
  };

  let mut dirname = String::with_capacity(path.len());

  match (end, has_root) {
    (None, true) => dirname.push('/'),
    (None, false) => dirname.push('.'),
    (Some(1), true) => dirname.push_str("//"),
    (Some(ending), _) => dirname.extend(path.chars().take(ending)),
  };

  Ok(dirname)
}
