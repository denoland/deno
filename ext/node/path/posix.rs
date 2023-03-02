// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;

use super::char_at_index;
use super::constants;

#[op]
pub fn op_node_path_posix_dirname(path: &str) -> Result<String, AnyError> {
  if path.chars().count() == 0 {
    return Ok(String::from("."));
  }

  let has_root =
    char_at_index(path, 0).unwrap() as u32 == constants::CHAR_FORWARD_SLASH;
  let mut end: Option<usize> = None;
  let mut matched_slash = true;

  for (i, c) in path.char_indices().rev() {
    if i < 1 {
      break;
    }
    match c as u32 {
      constants::CHAR_FORWARD_SLASH if !matched_slash => {
        end = Some(i);
        break;
      }
      constants::CHAR_FORWARD_SLASH => {
        // noop
      }
      _ => matched_slash = false,
    }
  }

  let dirname = match (end, has_root) {
    (None, true) => String::from("/"),
    (None, false) => String::from("."),
    (Some(1), true) => String::from("//"),
    (Some(ending), _) => path.chars().take(ending).collect(),
  };

  Ok(dirname)
}
