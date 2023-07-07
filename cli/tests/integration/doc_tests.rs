// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;
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
      .args("doc doc/deno_doc.ts")
      .split_output()
      .run();

    output.assert_exit_code(0);
    assert_contains!(output.stdout(), "function foo");
  }
}

itest!(deno_doc_import_map {
  args: "doc --unstable --import-map=doc/import_map.json doc/use_import_map.js",
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

itest!(_060_deno_doc_displays_all_overloads_in_details_view {
  args:
    "doc doc/060_deno_doc_displays_all_overloads_in_details_view.ts NS.test",
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

itest!(doc_lock {
  args: "doc main.ts",
  http_server: true,
  cwd: Some("lockfile/basic"),
  exit_code: 10,
  output: "lockfile/basic/fail.out",
});

itest!(doc_no_lock {
  args: "doc --no-lock main.ts",
  http_server: true,
  cwd: Some("lockfile/basic"),
  output: "lockfile/basic/doc.nolock.out",
});
