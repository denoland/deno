// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;

use super::char_at_index;
use super::constants;
use super::is_path_separator;
use super::is_windows_device_root;

#[op]
pub fn op_node_path_win32_isAbsolute(path: &str) -> bool {
  let len = path.chars().count();

  if len == 0 {
    return false;
  }

  let code = char_at_index(path,0).unwrap() as u32;

  if is_path_separator(code) {
    return true;
  }else if is_windows_device_root(code) {
    if len > 2 && char_at_index(path,1).unwrap() as u32 == constants::CHAR_COLON {
      if is_path_separator(char_at_index(path,2).unwrap() as u32){
        return true;
      }
    }
  }

  return false;
}
