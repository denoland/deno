// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use typenum::True;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::v8::Boolean;

use super::char_at_index;
use super::constants;


#[op]
pub fn op_node_path_posix_isAbsolute(path: &str) -> bool {
  return path.chars().count() > 0 && char_at_index(path,0).unwrap() as u32 == constants::CHAR_FORWARD_SLASH;
}
