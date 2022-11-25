// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod integration;

use test_util as util;

mod flags {
  use super::*;

  #[test]
  fn help_flag() {
    let status = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("--help")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }

  #[test]
  fn version_short_flag() {
    let status = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("-V")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }

  #[test]
  fn version_long_flag() {
    let status = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("--version")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }

  itest!(types {
    args: "types",
    output: "types/types.out",
  });
}
