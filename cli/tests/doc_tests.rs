// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod integration;

use test_util as util;
use test_util::TempDir;
use util::assert_contains;

mod doc {
  use super::*;

  itest!(deno_doc_builtin {
    args: "doc",
    output: "doc/deno_doc_builtin.out",
  });

  #[test]
  fn deno_doc() {
    let dir = TempDir::new();
    // try this twice to ensure it works with the cache
    for _ in 0..2 {
      let output = util::deno_cmd_with_deno_dir(&dir)
        .current_dir(util::testdata_path())
        .arg("doc")
        .arg("doc/deno_doc.ts")
        .env("NO_COLOR", "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();
      assert!(output.status.success());
      assert_contains!(
        std::str::from_utf8(&output.stdout).unwrap(),
        "function foo"
      );
    }
  }

  itest!(deno_doc_import_map {
    args:
      "doc --unstable --import-map=doc/import_map.json doc/use_import_map.js",
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
}
