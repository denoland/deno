// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;

use super::char_at_index;
use super::constants;
use super::is_path_separator;
use super::is_windows_device_root;

#[op]
pub fn op_node_path_win32_dirname(path: &str) -> Result<String, AnyError> {
  let char_count = path.chars().count();
  if char_count == 0 {
    return Ok(String::from("."));
  }

  let mut root_end: Option<usize> = None;
  let mut end: Option<usize> = None;
  let mut matched_slash = true;
  let mut offset = 0;

  let code = char_at_index(path, 0).unwrap() as u32;

  if char_count > 1 {
    if is_path_separator(code) {
      // Possible UNC root

      root_end = Some(1);
      offset = 1;

      if is_path_separator(char_at_index(path, 1).unwrap() as u32) {
        // Matched double path separator at beginning
        let mut j = 2;
        let mut last = j;
        // Match 1 or more non-path separators
        for ch in path.chars().skip(2) {
          if is_path_separator(ch as u32) {
            break;
          }
          j += 1;
        }
        if j < char_count && j != last {
          // Matched!
          last = j;
          // Match 1 or more path separators
          for ch in path.chars().skip(j) {
            if !is_path_separator(ch as u32) {
              break;
            }
            j += 1;
          }

          if j < char_count && j != last {
            // Matched!
            last = j;
            // Match 1 or more non-path separators
            for ch in path.chars().skip(j) {
              if is_path_separator(ch as u32) {
                break;
              }
              j += 1;
            }
            if j == char_count {
              // We matched UNC root only
              return Ok(path.to_string());
            }
            if j != last {
              // We matched a UNC root with leftovers

              // Offset by 1 to include the separator after the UNC root to
              // treat it as a "normal root" on top of a (UNC) root
              offset = j + 1;
              root_end = Some(offset);
            }
          }
        }
      }
    } else if is_windows_device_root(code) {
      // Possible device root

      if char_at_index(path, 1).unwrap() as u32 == constants::CHAR_COLON {
        root_end = Some(2);
        offset = 2;

        if char_count > 2
          && is_path_separator(char_at_index(path, 2).unwrap() as u32)
        {
          root_end = Some(3);
          offset = 3;
        }
      }
    }
  } else if is_path_separator(code) {
    // `path` contains just a path separator, exit early to avoid
    // unnecessary work
    return Ok(path.to_string());
  }

  for (i, ch) in path.char_indices().rev() {
    if i <= offset {
      break;
    }
    if is_path_separator(ch as u32) {
      if !matched_slash {
        end = Some(i);
        break;
      }
    } else {
      // We saw the first non-path separator
      matched_slash = false;
    }
  }

  if end.is_none() {
    if root_end.is_none() {
      return Ok(String::from("."));
    } else {
      end = root_end;
    }
  }

  let dirname = match (end, root_end) {
    (None, None) => String::from("."),
    (None, Some(e)) => path.chars().take(e).collect(),
    (Some(e), _) => path.chars().take(e).collect(),
  };

  Ok(dirname)
}
