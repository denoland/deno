// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::assert_contains;
use util::assert_not_contains;
use util::wildcard_match;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn junit_path() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("test.js", "Deno.test('does test', () => {});");
  let output = context
    .new_command()
    .args("test --junit-path=sub_dir/output.xml test.js")
    .run();
  output.skip_output_check();
  output.assert_exit_code(0);
  temp_dir
    .path()
    .join("sub_dir/output.xml")
    .assert_matches_text("<?xml [WILDCARD]");
}

#[test]
// todo(#18480): re-enable
#[ignore]
fn sigint_with_hanging_test() {
  util::with_pty(
    &[
      "test",
      "--quiet",
      "--no-check",
      "test/sigint_with_hanging_test.ts",
    ],
    |mut console| {
      std::thread::sleep(std::time::Duration::from_secs(1));
      console.write_line("\x03");
      let text = console.read_until("hanging_test.ts:10:15");
      wildcard_match(
        include_str!("../testdata/test/sigint_with_hanging_test.out"),
        &text,
      );
    },
  );
}

#[test]
fn test_with_glob_config() {
  let context = TestContextBuilder::new().cwd("test").build();

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/nested/fizz/fizz.ts");
  assert_contains!(output, "glob/pages/[id].ts");
  assert_contains!(output, "glob/nested/fizz/bar.ts");
  assert_contains!(output, "glob/nested/foo/foo.ts");
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/nested/foo/bar.ts");
  assert_contains!(output, "glob/nested/foo/fizz.ts");
  assert_contains!(output, "glob/nested/fizz/foo.ts");
  assert_contains!(output, "glob/data/test1.ts");
}

#[test]
fn test_with_glob_config_and_flags() {
  let context = TestContextBuilder::new().cwd("test").build();

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json --ignore=glob/nested/**/bar.ts")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/nested/fizz/fizz.ts");
  assert_contains!(output, "glob/pages/[id].ts");
  assert_contains!(output, "glob/nested/fizz/bazz.ts");
  assert_contains!(output, "glob/nested/foo/foo.ts");
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/nested/foo/bazz.ts");
  assert_contains!(output, "glob/nested/foo/fizz.ts");
  assert_contains!(output, "glob/nested/fizz/foo.ts");
  assert_contains!(output, "glob/data/test1.ts");

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json glob/data/test1.?s")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/data/test1.ts");
}

#[test]
fn conditionally_loads_type_graph() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("test --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch_no_follow_with_options - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("test --reload -L debug --no-check run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}
