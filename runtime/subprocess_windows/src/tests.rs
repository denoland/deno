// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::OsStr;
use std::io;

use crate::Command;

#[test]
fn spawn_rejects_nuls_in_arguments() {
  for verbatim_arguments in [false, true] {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
      .arg(OsStr::new("invalid\0argument"))
      .arg("must-not-be-dropped")
      .verbatim_arguments(verbatim_arguments);

    let error = command.spawn().err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "nul byte found in provided data");
  }
}
