// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::itest;
use util::assert_contains;
use util::TestContext;

itest!(deno_doc_builtin {
  args: "doc",
  output: "doc/deno_doc_builtin.out",
});

#[test]
fn deno_doc() {
  let context = TestContext::default();
  // try this twice to ensure it works with the cache
  for _ in 0..2 {
    let output = context
      .new_command()
      .env("NO_COLOR", "1")
      .args("doc doc/deno_doc.ts doc/deno_doc2.ts")
      .split_output()
      .run();

    output.assert_exit_code(0);
    assert_contains!(output.stdout(), "function foo");
    assert_contains!(output.stdout(), "function bar");
  }
}

itest!(deno_doc_import_map {
  args: "doc --import-map=doc/import_map.json doc/use_import_map.js",
  output: "doc/use_import_map.out",
});

itest!(deno_doc_types_hint {
  args: "doc doc/types_hint.ts",
  output: "doc/types_hint.out",
});

itest!(deno_doc_types_ref {
  args: "doc doc/types_ref.js",
  output: "doc/types_ref.out",
});

itest!(deno_doc_types_header {
  args: "doc --reload doc/types_header.ts",
  output: "doc/types_header.out",
  http_server: true,
});

itest!(deno_doc_referenced_private_types {
  args: "doc doc/referenced_private_types.ts",
  output: "doc/referenced_private_types.out",
});

itest!(deno_doc_lint_referenced_private_types_error {
  args: "doc --lint doc/referenced_private_types.ts",
  exit_code: 1,
  output: "doc/referenced_private_types_lint.out",
});

itest!(deno_doc_lint_referenced_private_types_fixed {
  args: "doc --lint doc/referenced_private_types_fixed.ts",
  output: "doc/referenced_private_types_fixed.out",
});

itest!(deno_doc_html_lint_referenced_private_types_fixed {
  args: "doc --lint --html --name=Library doc/referenced_private_types.ts",
  exit_code: 1,
  output: "doc/referenced_private_types_lint.out",
});

itest!(deno_doc_lint_success {
  args: "doc --lint doc/lint_success.ts",
  output: "doc/lint_success.out",
});

itest!(deno_doc_lint_json_success {
  args: "doc --lint --json doc/lint_success.ts",
  output: "doc/lint_success_json.out",
});

itest!(deno_doc_lint_html_success {
  args: "doc --lint --html --name=Library lint_success.ts",
  copy_temp_dir: Some("doc"),
  cwd: Some("doc"),
  output: "doc/lint_success_html.out",
});

itest!(_060_deno_doc_displays_all_overloads_in_details_view {
  args:
    "doc --filter NS.test doc/060_deno_doc_displays_all_overloads_in_details_view.ts",
  output: "doc/060_deno_doc_displays_all_overloads_in_details_view.ts.out",
});

itest!(deno_doc_types_header_direct {
  args: "doc --reload http://127.0.0.1:4545/xTypeScriptTypes.js",
  output: "doc/types_header.out",
  http_server: true,
});

itest!(deno_doc_invalid_url {
  args: "doc https://raw.githubusercontent.com%2Fdyedgreen%2Fdeno-sqlite%2Frework_api%2Fmod.ts",
  output: "doc/invalid_url.out",
  exit_code: 1,
});

#[test]
fn deno_doc_html() {
  let context = TestContext::default();
  let temp_dir = context.temp_dir();
  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args_vec(vec![
      "doc",
      "--html",
      "--name=MyLib",
      &format!("--output={}", temp_dir.path().to_string_lossy()),
      "doc/referenced_private_types_fixed.ts",
    ])
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Written 14 files to");
  assert!(temp_dir.path().join("all_symbols.html").exists());
  assert!(temp_dir.path().join("index.html").exists());
  assert!(temp_dir.path().join("fuse.js").exists());
  assert!(temp_dir.path().join("page.css").exists());
  assert!(temp_dir.path().join("reset.css").exists());
  assert!(temp_dir.path().join("script.js").exists());
  assert!(temp_dir.path().join("search.js").exists());
  assert!(temp_dir.path().join("search_index.js").exists());
  assert!(temp_dir.path().join("styles.css").exists());
  assert!(temp_dir.path().join("~/MyInterface.html").exists());
  assert!(temp_dir.path().join("~/MyInterface.prop.html").exists());
  assert!(temp_dir.path().join("~/MyClass.html").exists());
  assert!(temp_dir.path().join("~/MyClass.prototype.html").exists());
  assert!(temp_dir
    .path()
    .join("~/MyClass.prototype.prop.html")
    .exists());
}
