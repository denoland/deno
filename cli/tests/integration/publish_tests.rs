// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use test_util as util;
use test_util::TempDir;

itest!(no_token {
  args: "do-not-use-publish publish/missing_deno_json",
  output: "publish/no_token.out",
  exit_code: 1,
});

itest!(missing_deno_json {
  args:
    "do-not-use-publish --token 'sadfasdf' $TESTDATA/publish/missing_deno_json",
  output: "publish/missing_deno_json/missing_deno_json.out",
  exit_code: 1,
  temp_cwd: true,
});
