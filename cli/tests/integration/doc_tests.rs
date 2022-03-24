// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(deno_doc_builtin {
  args: "doc",
  output: "deno_doc_builtin.out",
});

itest!(deno_doc {
  args: "doc deno_doc.ts",
  output: "deno_doc.out",
});

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
  args: "doc 060_deno_doc_displays_all_overloads_in_details_view.ts NS.test",
  output: "060_deno_doc_displays_all_overloads_in_details_view.ts.out",
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
