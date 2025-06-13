// Copyright 2018-2025 the Deno authors. MIT license.

pub fn is_skippable_io_error(e: &std::io::Error) -> bool {
  use std::io::ErrorKind::*;

  // skip over invalid filenames on windows
  const ERROR_INVALID_NAME: i32 = 123;
  if cfg!(windows) && e.raw_os_error() == Some(ERROR_INVALID_NAME) {
    return true;
  }

  match e.kind() {
    InvalidInput | PermissionDenied | NotFound => {
      // ok keep going
      true
    }
    _ => {
      const NOT_A_DIRECTORY: i32 = 20;
      cfg!(unix) && e.raw_os_error() == Some(NOT_A_DIRECTORY)
    }
  }
}

#[cfg(test)]
mod tests {
  #[cfg(windows)]
  #[test]
  fn is_skippable_io_error_win_invalid_filename() {
    let error = std::io::Error::from_raw_os_error(123);
    assert!(super::is_skippable_io_error(&error));
  }
}
