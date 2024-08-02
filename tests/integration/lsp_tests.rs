// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use pretty_assertions::assert_eq;
use std::fs;
use test_util::assert_starts_with;
use test_util::assertions::assert_json_subset;
use test_util::deno_cmd_with_deno_dir;
use test_util::env_vars_for_npm_tests;
use test_util::lsp::range_of;
use test_util::lsp::source_file;
use test_util::lsp::LspClient;
use test_util::testdata_path;
use test_util::TestContextBuilder;
use tower_lsp::lsp_types as lsp;

#[test]
fn lsp_startup_shutdown() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.shutdown();
}

#[test]
fn lsp_init_tsconfig() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "lib.tsconfig.json",
    r#"{
  "compilerOptions": {
    "lib": ["deno.ns", "deno.unstable", "dom"]
  }
}"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("lib.tsconfig.json");
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "location.pathname;\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 0);

  client.shutdown();
}

#[test]
fn lsp_tsconfig_types() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "types.tsconfig.json",
    r#"{
  "compilerOptions": {
    "types": ["./a.d.ts"]
  },
  "lint": {
    "rules": {
      "tags": []
    }
  }
}"#,
  );
  let a_dts = "// deno-lint-ignore-file no-var\ndeclare var a: string;";
  temp_dir.write("a.d.ts", a_dts);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder
      .set_config("types.tsconfig.json")
      // avoid finding the declaration file via the document preload
      .set_preload_limit(0);
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("test.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(a);\n"
    }
  }));

  assert_eq!(json!(diagnostics.all()), json!([]));

  client.shutdown();
}

#[test]
fn lsp_tsconfig_types_config_sub_dir() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  let sub_dir = temp_dir.path().join("sub_dir");
  sub_dir.create_dir_all();
  sub_dir.join("types.tsconfig.json").write(
    r#"{
  "compilerOptions": {
    "types": ["./a.d.ts"]
  },
  "lint": {
    "rules": {
      "tags": []
    }
  }
}"#,
  );
  let a_dts = "// deno-lint-ignore-file no-var\ndeclare var a: string;";
  sub_dir.join("a.d.ts").write(a_dts);
  temp_dir.write("deno.json", "{}");

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder
      .set_config("sub_dir/types.tsconfig.json")
      // avoid finding the declaration file via the document preload
      .set_preload_limit(0);
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("test.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(a);\n"
    }
  }));

  assert_eq!(json!(diagnostics.all()), json!([]));

  client.shutdown();
}

#[test]
fn lsp_triple_slash_types() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let a_dts = "// deno-lint-ignore-file no-var\ndeclare var a: string;";
  temp_dir.write("a.d.ts", a_dts);
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("test.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "/// <reference types=\"./a.d.ts\" />\n\nconsole.log(a);\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 0);

  client.shutdown();
}

#[test]
fn unadded_dependency_message_with_import_map() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "import_map.json",
    json!({
      "imports": {

      }
    })
    .to_string(),
  );
  temp_dir.write(
    "deno.json",
    json!({
      "importMap": "import_map.json".to_string(),
    })
    .to_string(),
  );
  temp_dir.write(
    "file.ts",
    r#"
        import * as x from "@std/fs";
      "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    }
  }));
  // expected lsp_messages don't include the file path
  let mut expected_lsp_messages = Vec::from(["`x` is never used\nIf this is intentional, prefix it with an underscore like `_x`",
  "'x' is declared but its value is never read.",
  "Relative import path \"@std/fs\" not prefixed with / or ./ or ../ and not in import map from \" Hint: Use [deno add @std/fs] to add the dependency."]);
  expected_lsp_messages.sort();
  let all_diagnostics = diagnostics.all();
  let mut correct_lsp_messages = all_diagnostics
    .iter()
    .map(|d| d.message.as_str())
    .collect::<Vec<&str>>();
  correct_lsp_messages.sort();
  let part1 = correct_lsp_messages[1].split("file").collect::<Vec<_>>()[0];
  let part2 = correct_lsp_messages[1].split('\n').collect::<Vec<_>>()[1];
  let file_path_removed_from_message = format!("{} {}", part1, part2);
  correct_lsp_messages[1] = file_path_removed_from_message.as_str();
  assert_eq!(correct_lsp_messages, expected_lsp_messages);
  client.shutdown();
}

#[test]
fn unadded_dependency_message() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
        "imports": {

    }
      })
    .to_string(),
  );
  temp_dir.write(
    "file.ts",
    r#"
        import * as x from "@std/fs";
      "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    }
  }));
  // expected lsp_messages don't include the file path
  let mut expected_lsp_messages = Vec::from(["`x` is never used\nIf this is intentional, prefix it with an underscore like `_x`",
  "'x' is declared but its value is never read.",
  "Relative import path \"@std/fs\" not prefixed with / or ./ or ../ and not in import map from \" Hint: Use [deno add @std/fs] to add the dependency."]);
  expected_lsp_messages.sort();
  let all_diagnostics = diagnostics.all();
  let mut correct_lsp_messages = all_diagnostics
    .iter()
    .map(|d| d.message.as_str())
    .collect::<Vec<&str>>();
  correct_lsp_messages.sort();
  let part1 = correct_lsp_messages[1].split("file").collect::<Vec<_>>()[0];
  let part2 = correct_lsp_messages[1].split('\n').collect::<Vec<_>>()[1];
  let file_path_removed_from_message = format!("{} {}", part1, part2);
  correct_lsp_messages[1] = file_path_removed_from_message.as_str();
  assert_eq!(correct_lsp_messages, expected_lsp_messages);
  client.shutdown();
}

#[test]
fn lsp_import_map() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let import_map = r#"{
  "imports": {
    "/~/": "./lib/"
  }
}"#;
  temp_dir.write("import-map.json", import_map);
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("import-map.json");
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(json!(diagnostics.all()), json!([]));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_import_map_remote() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "importMap": "http://localhost:4545/import_maps/import_map_remote.json",
    })
    .to_string(),
  );
  temp_dir.write(
    "file.ts",
    r#"
      import { printHello } from "print_hello";
      printHello();
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map(
      "http://localhost:4545/import_maps/import_map_remote.json",
    );
  });
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    }
  }));
  assert_eq!(diagnostics.all(), vec![]);
  client.shutdown();
}

#[test]
fn lsp_import_map_data_url() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("data:application/json;utf8,{\"imports\": { \"example\": \"https://deno.land/x/example/mod.ts\" }}");
  });
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import example from \"example\";\n"
    }
  }));

  // This indicates that the import map is applied correctly.
  assert!(diagnostics.all().iter().any(|diagnostic| diagnostic.code
    == Some(lsp::NumberOrString::String("no-cache".to_string()))
    && diagnostic
      .message
      .contains("https://deno.land/x/example/mod.ts")));
  client.shutdown();
}

#[test]
fn lsp_import_map_config_file() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.import_map.jsonc",
    r#"{
  "importMap": "import-map.json"
}"#,
  );
  temp_dir.write(
    "import-map.json",
    r#"{
  "imports": {
    "/~/": "./lib/"
  }
}"#,
  );
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.import_map.jsonc");
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 0);

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_import_map_embedded_in_config_file() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.embedded_import_map.jsonc",
    r#"{
  // some comment
  "imports": {
    "/~/": "./lib/"
  }
}"#,
  );
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.embedded_import_map.jsonc");
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 0);

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_import_map_embedded_in_config_file_after_initialize() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.embedded_import_map.jsonc", "{}");
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.embedded_import_map.jsonc");
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 1);

  // update the import map
  temp_dir.write(
    "deno.embedded_import_map.jsonc",
    r#"{
  "imports": {
    "/~/": "./lib/"
  }
}"#,
  );

  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.embedded_import_map.jsonc").unwrap(),
      "type": 2
    }]
  }));

  assert_eq!(json!(client.read_diagnostics().all()), json!([]));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_import_map_config_file_auto_discovered() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().capture_stderr().build();
  client.initialize_default();

  // add the deno.json
  temp_dir.write("deno.jsonc", r#"{ "imports": { "/~/": "./lib/" } }"#);
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.jsonc").unwrap(),
      "type": 2
    }]
  }));
  client.wait_until_stderr_line(|line| {
    line.contains("  Resolved Deno configuration file:")
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(diagnostics.all().len(), 0);

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );

  // now cause a syntax error
  temp_dir.write("deno.jsonc", r#",,#,#,,"#);
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.jsonc").unwrap(),
      "type": 2
    }]
  }));
  assert_eq!(client.read_diagnostics().all().len(), 1);

  // now fix it, and things should work again
  temp_dir.write("deno.jsonc", r#"{ "imports": { "/~/": "./lib/" } }"#);
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.jsonc").unwrap(),
      "type": 2
    }]
  }));
  client.wait_until_stderr_line(|line| {
    line.contains("  Resolved Deno configuration file:")
  });
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 2, "character": 12 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": { "line": 2, "character": 12 },
        "end": { "line": 2, "character": 13 }
      }
    })
  );
  assert_eq!(client.read_diagnostics().all().len(), 0);

  client.shutdown();
}

#[test]
fn lsp_import_map_config_file_auto_discovered_symlink() {
  let context = TestContextBuilder::new()
    // DO NOT COPY THIS CODE. Very rare case where we want to force the temp
    // directory on the CI to not be a symlinked directory because we are
    // testing a scenario with a symlink to a non-symlink in the same directory
    // tree. Generally you want to ensure your code works in symlinked directories
    // so don't use this unless you have a similar scenario.
    .temp_dir_path(std::env::temp_dir().canonicalize().unwrap())
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().capture_stderr().build();
  client.initialize_default();

  // now create a symlink in the current directory to a subdir/deno.json
  // and ensure the watched files notification still works
  temp_dir.create_dir_all("subdir");
  temp_dir.write("subdir/deno.json", r#"{ }"#);
  temp_dir.symlink_file(
    temp_dir.path().join("subdir").join("deno.json"),
    temp_dir.path().join("deno.json"),
  );
  client.did_change_watched_files(json!({
    "changes": [{
      // the client will give a watched file changed event for the symlink's target
      "uri": temp_dir.path().join("subdir/deno.json").canonicalize().uri_file(),
      "type": 2
    }]
  }));

  // this will discover the deno.json in the root
  let search_line = format!(
    "  Resolved Deno configuration file: \"{}\"",
    temp_dir.uri().join("deno.json").unwrap().as_str()
  );
  client.wait_until_stderr_line(|line| line.contains(&search_line));

  // now open a file which will cause a diagnostic because the import map is empty
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("a.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));
  assert_eq!(diagnostics.all().len(), 1);

  // update the import map to have the imports now
  temp_dir.write("subdir/deno.json", r#"{ "imports": { "/~/": "./lib/" } }"#);
  client.did_change_watched_files(json!({
    "changes": [{
      // now still say that the target path has changed
      "uri": temp_dir.path().join("subdir/deno.json").canonicalize().uri_file(),
      "type": 2
    }]
  }));
  assert_eq!(client.read_diagnostics().all().len(), 0);

  client.shutdown();
}

#[test]
fn lsp_deno_json_imports_comments_cache() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.jsonc",
    r#"{
      // comment
      "imports": {
        "print_hello": "http://localhost:4545/import_maps/print_hello.ts",
      },
    }"#,
  );
  temp_dir.write(
    "file.ts",
    r#"
      import { printHello } from "print_hello";
      printHello();
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    }
  }));
  assert_eq!(diagnostics.all(), vec![]);
  client.shutdown();
}

#[test]
fn lsp_import_map_node_specifiers() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write("deno.json", r#"{ "imports": { "fs": "node:fs" } }"#);

  // cache @types/node
  context
    .new_command()
    .args("cache npm:@types/node")
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.json");
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("a.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import fs from \"fs\";\nconsole.log(fs);"
    }
  }));
  assert_eq!(diagnostics.all(), vec![]);

  client.shutdown();
}

#[test]
fn lsp_format_vendor_path() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();

  // put this dependency in the global cache
  context
    .new_command()
    .args("cache http://localhost:4545/run/002_hello.ts")
    .run()
    .skip_output_check();

  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({ "vendor": true }).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"import "http://localhost:4545/run/002_hello.ts";"#,
    },
  }));
  // copying from the global cache to the local cache requires explicitly
  // running the cache command so that the checksums can be verified
  assert_eq!(
    diagnostics
      .all()
      .iter()
      .map(|d| d.message.as_str())
      .collect::<Vec<_>>(),
    vec![
      "Uncached or missing remote URL: http://localhost:4545/run/002_hello.ts"
    ]
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );
  assert!(temp_dir
    .path()
    .join("vendor/http_localhost_4545/run/002_hello.ts")
    .exists());
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("vendor/http_localhost_4545/run/002_hello.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"console.log("Hello World");"#,
    },
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("vendor/http_localhost_4545/run/002_hello.ts").unwrap(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true,
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": {
          "line": 0,
          "character": 27,
        },
        "end": {
          "line": 0,
          "character": 27,
        },
      },
      "newText": "\n",
    }]),
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/19802.
// Disable the `workspace/configuration` capability. Ensure the LSP falls back
// to using `enablePaths` from the `InitializationOptions`.
#[test]
fn lsp_workspace_enable_paths_no_workspace_configuration() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("main_disabled.ts", "Date.now()");
  temp_dir.write("main_enabled.ts", "Date.now()");

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.with_capabilities(|capabilities| {
      capabilities.workspace.as_mut().unwrap().configuration = Some(false);
    });
    builder.set_workspace_folders(vec![lsp::WorkspaceFolder {
      uri: temp_dir.uri(),
      name: "project".to_string(),
    }]);
    builder.set_root_uri(temp_dir.uri());
    builder.set_enable_paths(vec!["./main_enabled.ts".to_string()]);
  });

  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("main_disabled.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("main_disabled.ts"),
    }
  }));

  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("main_enabled.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("main_enabled.ts"),
    }
  }));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("main_disabled.ts").unwrap(),
      },
      "position": { "line": 0, "character": 5 }
    }),
  );
  assert_eq!(res, json!(null));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("main_enabled.ts").unwrap(),
      },
      "position": { "line": 0, "character": 5 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(method) DateConstructor.now(): number",
        },
        "Returns the number of milliseconds elapsed since midnight, January 1, 1970 Universal Coordinated Time (UTC)."
      ],
      "range": {
        "start": { "line": 0, "character": 5, },
        "end": { "line": 0, "character": 8, }
      }
    })
  );

  client.shutdown();
}

#[test]
fn lsp_did_change_deno_configuration_notification() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  temp_dir.write("package.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [
        {
          "scopeUri": temp_dir.uri(),
          "fileUri": temp_dir.uri().join("deno.json").unwrap(),
          "type": "added",
          "configurationType": "denoJson"
        },
        {
          "scopeUri": temp_dir.uri(),
          "fileUri": temp_dir.uri().join("package.json").unwrap(),
          "type": "added",
          "configurationType": "packageJson"
        },
      ],
    }))
  );

  temp_dir.write(
    "deno.json",
    json!({ "fmt": { "semiColons": false } }).to_string(),
  );
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.json").unwrap(),
      "type": 2,
    }],
  }));
  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [{
        "scopeUri": temp_dir.uri(),
        "fileUri": temp_dir.uri().join("deno.json").unwrap(),
        "type": "changed",
        "configurationType": "denoJson"
      }],
    }))
  );

  temp_dir.remove_file("deno.json");
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.json").unwrap(),
      "type": 3,
    }],
  }));
  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [{
        "scopeUri": temp_dir.uri(),
        "fileUri": temp_dir.uri().join("deno.json").unwrap(),
        "type": "removed",
        "configurationType": "denoJson"
      }],
    }))
  );

  temp_dir.write("package.json", json!({ "type": "module" }).to_string());
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("package.json").unwrap(),
      "type": 2,
    }],
  }));
  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [{
        "scopeUri": temp_dir.uri(),
        "fileUri": temp_dir.uri().join("package.json").unwrap(),
        "type": "changed",
        "configurationType": "packageJson"
      }],
    }))
  );

  temp_dir.remove_file("package.json");
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("package.json").unwrap(),
      "type": 3,
    }],
  }));
  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [{
        "scopeUri": temp_dir.uri(),
        "fileUri": temp_dir.uri().join("package.json").unwrap(),
        "type": "removed",
        "configurationType": "packageJson"
      }],
    }))
  );
  client.shutdown();
}

#[test]
fn lsp_deno_task() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.jsonc",
    r#"{
    "tasks": {
      "build": "deno test",
      "some:test": "deno bundle mod.ts"
    }
  }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.jsonc");
  });

  let res = client.write_request("deno/task", json!(null));

  assert_eq!(
    res,
    json!([
      {
        "name": "build",
        "detail": "deno test",
        "sourceUri": temp_dir.uri().join("deno.jsonc").unwrap(),
      }, {
        "name": "some:test",
        "detail": "deno bundle mod.ts",
        "sourceUri": temp_dir.uri().join("deno.jsonc").unwrap(),
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_reload_import_registries_command() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "workspace/executeCommand",
    json!({ "command": "deno.reloadImportRegistries" }),
  );
  assert_eq!(res, json!(true));
  client.shutdown();
}

#[test]
fn lsp_import_attributes() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("data:application/json;utf8,{\"imports\": { \"example\": \"https://deno.land/x/example/mod.ts\" }}");
  });
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "test": true,
      },
    },
  }));

  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/test.json",
      "languageId": "json",
      "version": 1,
      "text": "{\"a\":1}",
    },
  }));

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/a.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import a from \"./test.json\";\n\nconsole.log(a);\n"
    }
  }));

  assert_eq!(
    json!(
      diagnostics
        .messages_with_file_and_source("file:///a/a.ts", "deno")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 14 },
          "end": { "line": 0, "character": 27 }
        },
        "severity": 1,
        "code": "no-attribute-type",
        "source": "deno",
        "message": "The module is a JSON module and not being imported with an import attribute. Consider adding `with { type: \"json\" }` to the import statement."
      }
    ])
  );

  let res = client
    .write_request(
      "textDocument/codeAction",
      json!({
        "textDocument": {
          "uri": "file:///a/a.ts"
        },
        "range": {
          "start": { "line": 0, "character": 14 },
          "end": { "line": 0, "character": 27 }
        },
        "context": {
          "diagnostics": [{
            "range": {
              "start": { "line": 0, "character": 14 },
              "end": { "line": 0, "character": 27 }
            },
            "severity": 1,
            "code": "no-attribute-type",
            "source": "deno",
            "message": "The module is a JSON module and not being imported with an import attribute. Consider adding `with { type: \"json\" }` to the import statement."
          }],
          "only": ["quickfix"]
        }
      }),
    );
  assert_eq!(
    res,
    json!([{
      "title": "Insert import attribute.",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 14 },
            "end": { "line": 0, "character": 27 }
          },
          "severity": 1,
          "code": "no-attribute-type",
          "source": "deno",
          "message": "The module is a JSON module and not being imported with an import attribute. Consider adding `with { type: \"json\" }` to the import statement."
        }
      ],
      "edit": {
        "changes": {
          "file:///a/a.ts": [
            {
              "range": {
                "start": { "line": 0, "character": 27 },
                "end": { "line": 0, "character": 27 }
              },
              "newText": " with { type: \"json\" }"
            }
          ]
        }
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_import_map_import_completions() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "import-map.json",
    r#"{
  "imports": {
    "/~/": "./lib/",
    "/#/": "./src/",
    "fs": "https://example.com/fs/index.js",
    "std/": "https://example.com/std@0.123.0/"
  }
}"#,
  );
  temp_dir.create_dir_all("lib");
  temp_dir.write("lib/b.ts", r#"export const b = "b";"#);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("import-map.json");
  });

  let uri = temp_dir.uri().join("a.ts").unwrap();

  client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"/~/b.ts\";\nimport * as b from \"\""
    }
  }));

  let res = client.get_completion(
    &uri,
    (1, 20),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "\""
    }),
  );
  assert_eq!(
    json!(res),
    json!({
      "isIncomplete": false,
      "items": [
        {
          "label": ".",
          "kind": 19,
          "detail": "(local)",
          "sortText": "1",
          "insertText": ".",
          "commitCharacters": ["\"", "'"],
        }, {
          "label": "..",
          "kind": 19,
          "detail": "(local)",
          "sortText": "1",
          "insertText": "..",
          "commitCharacters": ["\"", "'"],
        }, {
          "label": "std",
          "kind": 19,
          "detail": "(import map)",
          "sortText": "std",
          "insertText": "std",
          "commitCharacters": ["\"", "'"],
        }, {
          "label": "fs",
          "kind": 17,
          "detail": "(import map)",
          "sortText": "fs",
          "insertText": "fs",
          "commitCharacters": ["\"", "'"],
        }, {
          "label": "/~",
          "kind": 19,
          "detail": "(import map)",
          "sortText": "/~",
          "insertText": "/~",
          "commitCharacters": ["\"", "'"],
        }, {
          "label": "/#",
          "kind": 19,
          "detail": "(import map)",
          "sortText": "/#",
          "insertText": "/#",
          "commitCharacters": ["\"", "'"],
        },
      ]
    })
  );

  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": uri,
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 1, "character": 20 },
            "end": { "line": 1, "character": 20 }
          },
          "text": "/~/"
        }
      ]
    }),
  );

  let res = client.get_completion(
    uri,
    (1, 23),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "/"
    }),
  );
  assert_eq!(
    json!(res),
    json!({
      "isIncomplete": false,
      "items": [
        {
          "label": "b.ts",
          "kind": 17,
          "detail": "(local)",
          "sortText": "1",
          "filterText": "/~/b.ts",
          "textEdit": {
            "range": {
              "start": { "line": 1, "character": 20 },
              "end": { "line": 1, "character": 23 }
            },
            "newText": "/~/b.ts"
          },
          "commitCharacters": ["\"", "'"],
        }
      ]
    })
  );

  client.shutdown();
}

#[test]
fn lsp_hover() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Deno.args);\n"
    }
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 19 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const Deno.args: string[]"
        },
        "Returns the script arguments to the program.\n\nGive the following command line invocation of Deno:\n\n```sh\ndeno run --allow-read https://examples.deno.land/command-line-arguments.ts Sushi\n```\n\nThen `Deno.args` will contain:\n\n```ts\n[ \"Sushi\" ]\n```\n\nIf you are looking for a structured way to parse arguments, there is\n[`parseArgs()`](https://jsr.io/@std/cli/doc/parse-args/~/parseArgs) from\nthe Deno Standard Library.",
        "\n\n*@category* - Runtime",
      ],
      "range": {
        "start": { "line": 0, "character": 17 },
        "end": { "line": 0, "character": 21 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_asset() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n"
    }
  }));
  client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap()
      },
      "position": { "line": 0, "character": 14 }
    }),
  );
  client.write_request(
    "deno/virtualTextDocument",
    json!({
      "textDocument": {
        "uri": "deno:/asset/lib.deno.shared_globals.d.ts"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "deno:/asset/lib.es2015.symbol.wellknown.d.ts"
      },
      "position": { "line": 111, "character": 13 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "interface Date",
        },
        "Enables basic storage and retrieval of dates and times.",
        "\n\n*@category* - Temporal  \n\n*@experimental*"
      ],
      "range": {
        "start": { "line": 111, "character": 10, },
        "end": { "line": 111, "character": 14, }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_disabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_deno_enable(false);
  });
  client.change_configuration(json!({ "deno": { "enable": false } }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n",
    },
  }));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 19 }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_inlay_hints() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.enable_inlay_hints();
  });
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"function a(b: string) {
          return b;
        }

        a("foo");

        enum C {
          A,
        }

        parseInt("123", 8);

        const d = Date.now();

        class E {
          f = Date.now();
        }

        ["a"].map((v) => v + v);
        "#
    }
  }));
  let res = client.write_request(
    "textDocument/inlayHint",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 19, "character": 0, }
      }
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "position": { "line": 0, "character": 21 },
        "label": ": string",
        "kind": 1,
        "paddingLeft": true
      }, {
        "position": { "line": 4, "character": 10 },
        "label": "b:",
        "kind": 2,
        "paddingRight": true
      }, {
        "position": { "line": 7, "character": 11 },
        "label": "= 0",
        "paddingLeft": true
      }, {
        "position": { "line": 10, "character": 17 },
        "label": "string:",
        "kind": 2,
        "paddingRight": true
      }, {
        "position": { "line": 10, "character": 24 },
        "label": "radix:",
        "kind": 2,
        "paddingRight": true
      }, {
        "position": { "line": 12, "character": 15 },
        "label": ": number",
        "kind": 1,
        "paddingLeft": true
      }, {
        "position": { "line": 15, "character": 11 },
        "label": ": number",
        "kind": 1,
        "paddingLeft": true
      }, {
        "position": { "line": 18, "character": 18 },
        "label": "callbackfn:",
        "kind": 2,
        "paddingRight": true
      }, {
        "position": { "line": 18, "character": 20 },
        "label": ": string",
        "kind": 1,
        "paddingLeft": true
      }, {
        "position": { "line": 18, "character": 21 },
        "label": ": string",
        "kind": 1,
        "paddingLeft": true
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_inlay_hints_not_enabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"function a(b: string) {
          return b;
        }

        a("foo");

        enum C {
          A,
        }

        parseInt("123", 8);

        const d = Date.now();

        class E {
          f = Date.now();
        }

        ["a"].map((v) => v + v);
        "#
    }
  }));
  let res = client.write_request(
    "textDocument/inlayHint",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 19, "character": 0, }
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_workspace_disable_enable_paths() {
  fn run_test(use_trailing_slash: bool) {
    let context = TestContextBuilder::new().use_temp_cwd().build();
    let temp_dir = context.temp_dir();
    temp_dir.create_dir_all("worker");
    temp_dir.write("worker/shared.ts", "export const a = 1");
    temp_dir.write("worker/other.ts", "import { a } from './shared.ts';\na;");
    temp_dir.write("worker/node.ts", "Buffer.alloc(1);");

    let root_specifier = temp_dir.uri();

    let mut client = context.new_lsp_command().build();
    client.initialize_with_config(
      |builder| {
        builder
          .set_disable_paths(vec!["./worker/node.ts".to_string()])
          .set_enable_paths(vec!["./worker".to_string()])
          .set_root_uri(root_specifier.clone())
          .set_workspace_folders(vec![lsp::WorkspaceFolder {
            uri: if use_trailing_slash {
              root_specifier.clone()
            } else {
              ModuleSpecifier::parse(
                root_specifier.as_str().strip_suffix('/').unwrap(),
              )
              .unwrap()
            },
            name: "project".to_string(),
          }]);
      },
      json!({ "deno": {
        "disablePaths": ["./worker/node.ts"],
        "enablePaths": ["./worker"],
      } }),
    );

    client.did_open(json!({
      "textDocument": {
        "uri": root_specifier.join("./file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }));

    client.did_open(json!({
      "textDocument": {
        "uri": root_specifier.join("./other/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }));

    client.did_open(json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": concat!(
          "console.log(Date.now());\n",
          "import { a } from './shared.ts';\n",
          "a;\n",
        ),
      }
    }));

    client.did_open(json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/subdir/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }));

    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 19 }
      }),
    );
    assert_eq!(res, json!(null));

    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./other/file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 19 }
      }),
    );
    assert_eq!(res, json!(null));

    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/node.ts").unwrap(),
        },
        "position": { "line": 0, "character": 0 }
      }),
    );
    assert_eq!(res, json!(null));

    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 19 }
      }),
    );
    assert_eq!(
      res,
      json!({
        "contents": [
          {
            "language": "typescript",
            "value": "(method) DateConstructor.now(): number",
          },
          "Returns the number of milliseconds elapsed since midnight, January 1, 1970 Universal Coordinated Time (UTC)."
        ],
        "range": {
          "start": { "line": 0, "character": 17, },
          "end": { "line": 0, "character": 20, }
        }
      })
    );

    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/subdir/file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 19 }
      }),
    );
    assert_eq!(
      res,
      json!({
        "contents": [
          {
            "language": "typescript",
            "value": "(method) DateConstructor.now(): number",
          },
          "Returns the number of milliseconds elapsed since midnight, January 1, 1970 Universal Coordinated Time (UTC)."
        ],
        "range": {
          "start": { "line": 0, "character": 17, },
          "end": { "line": 0, "character": 20, }
        }
      })
    );

    // check that the file system documents were auto-discovered
    // via the enabled paths
    let res = client.write_request(
      "textDocument/references",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/file.ts").unwrap(),
        },
        "position": { "line": 2, "character": 0 },
        "context": {
          "includeDeclaration": true
        }
      }),
    );

    assert_eq!(
      res,
      json!([{
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
        "range": {
          "start": { "line": 1, "character": 9 },
          "end": { "line": 1, "character": 10 }
        }
      }, {
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
        "range": {
          "start": { "line": 2, "character": 0 },
          "end": { "line": 2, "character": 1 }
        }
      }, {
        "uri": root_specifier.join("./worker/shared.ts").unwrap(),
        "range": {
          "start": { "line": 0, "character": 13 },
          "end": { "line": 0, "character": 14 }
        }
      }, {
        "uri": root_specifier.join("./worker/other.ts").unwrap(),
        "range": {
          "start": { "line": 0, "character": 9 },
          "end": { "line": 0, "character": 10 }
        }
      }, {
        "uri": root_specifier.join("./worker/other.ts").unwrap(),
        "range": {
          "start": { "line": 1, "character": 0 },
          "end": { "line": 1, "character": 1 }
        }
      }])
    );

    client.shutdown();
  }

  run_test(true);
  run_test(false);
}

#[test]
fn lsp_exclude_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("other");
  temp_dir.write(
    "other/shared.ts",
    // this should not be found in the "find references" since this file is excluded
    "import { a } from '../worker/shared.ts'; console.log(a);",
  );
  temp_dir.create_dir_all("worker");
  temp_dir.write("worker/shared.ts", "export const a = 1");
  temp_dir.write(
    "deno.json",
    r#"{
  "exclude": ["other"],
}"#,
  );
  let root_specifier = temp_dir.uri();

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  client.did_open(json!({
    "textDocument": {
      "uri": root_specifier.join("./other/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n"
    }
  }));

  client.did_open(json!({
    "textDocument": {
      "uri": root_specifier.join("./worker/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": concat!(
        "console.log(Date.now());\n",
        "import { a } from './shared.ts';\n",
        "a;\n",
      ),
    }
  }));

  client.did_open(json!({
    "textDocument": {
      "uri": root_specifier.join("./worker/subdir/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n"
    }
  }));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": root_specifier.join("./other/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 19 }
    }),
  );
  assert_eq!(res, json!(null));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 19 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(method) DateConstructor.now(): number",
        },
        "Returns the number of milliseconds elapsed since midnight, January 1, 1970 Universal Coordinated Time (UTC)."
      ],
      "range": {
        "start": { "line": 0, "character": 17, },
        "end": { "line": 0, "character": 20, }
      }
    })
  );

  // check that the file system documents were auto-discovered
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
      },
      "position": { "line": 2, "character": 0 },
      "context": {
        "includeDeclaration": true
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
      "uri": root_specifier.join("./worker/file.ts").unwrap(),
      "range": {
        "start": { "line": 1, "character": 9 },
        "end": { "line": 1, "character": 10 }
      }
    }, {
      "uri": root_specifier.join("./worker/file.ts").unwrap(),
      "range": {
        "start": { "line": 2, "character": 0 },
        "end": { "line": 2, "character": 1 }
      }
    }, {
      "uri": root_specifier.join("./worker/shared.ts").unwrap(),
      "range": {
        "start": { "line": 0, "character": 13 },
        "end": { "line": 0, "character": 14 }
      }
    }])
  );

  client.shutdown();
}

#[test]
fn lsp_hover_unstable_always_enabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      // IMPORTANT: If you change this API due to stabilization, also change it
      // in the enabled test below.
      "text": "type _ = Deno.ForeignLibraryInterface;\n"
    }
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 14 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents":[
        {
          "language":"typescript",
          "value":"interface Deno.ForeignLibraryInterface"
        },
        "**UNSTABLE**: New API, yet to be vetted.\n\nA foreign library interface descriptor.",
        "\n\n*@category* - FFI  \n\n*@experimental*",
      ],
      "range":{
        "start":{ "line":0, "character":14 },
        "end":{ "line":0, "character":37 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_unstable_enabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    // NOTE(bartlomieju): this is effectively not used anymore.
    builder.set_unstable(true);
  });
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "type _ = Deno.ForeignLibraryInterface;\n"
    }
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 14 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents":[
        {
          "language":"typescript",
          "value":"interface Deno.ForeignLibraryInterface"
        },
        "**UNSTABLE**: New API, yet to be vetted.\n\nA foreign library interface descriptor.",
        "\n\n*@category* - FFI  \n\n*@experimental*",
      ],
      "range":{
        "start":{ "line":0, "character":14 },
        "end":{ "line":0, "character":37 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_change_mbc() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "const a = `编写软件很难`;\nconst b = `👍🦕😃`;\nconsole.log(a, b);\n"
      }
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 1, "character": 11 },
            "end": {
              "line": 1,
              // the LSP uses utf16 encoded characters indexes, so
              // after the deno emoji is character index 15
              "character": 15
            }
          },
          "text": ""
        }
      ]
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 2, "character": 15 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const b: \"😃\"",
        },
        "",
      ],
      "range": {
        "start": { "line": 2, "character": 15, },
        "end": { "line": 2, "character": 16, },
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_closed_document() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("a.ts", r#"export const a = "a";"#);
  temp_dir.write("b.ts", r#"export * from "./a.ts";"#);
  temp_dir.write("c.ts", "import { a } from \"./b.ts\";\nconsole.log(a);\n");

  let b_specifier = temp_dir.uri().join("b.ts").unwrap();
  let c_specifier = temp_dir.uri().join("c.ts").unwrap();

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": b_specifier,
      "languageId": "typescript",
      "version": 1,
      "text": r#"export * from "./a.ts";"#
    }
  }));

  client.did_open(json!({
    "textDocument": {
      "uri": c_specifier,
      "languageId": "typescript",
      "version": 1,
      "text": "import { a } from \"./b.ts\";\nconsole.log(a);\n",
    }
  }));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": c_specifier,
      },
      "position": { "line": 0, "character": 10 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(alias) const a: \"a\"\nimport a"
        },
        ""
      ],
      "range": {
        "start": { "line": 0, "character": 9 },
        "end": { "line": 0, "character": 10 }
      }
    })
  );
  client.write_notification(
    "textDocument/didClose",
    json!({
      "textDocument": {
        "uri": b_specifier,
      }
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": c_specifier,
      },
      "position": { "line": 0, "character": 10 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(alias) const a: \"a\"\nimport a"
        },
        ""
      ],
      "range": {
        "start": { "line": 0, "character": 9 },
        "end": { "line": 0, "character": 10 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_dependency() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    }
  }));
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/xTypeScriptTypes.js\";\n// @deno-types=\"http://127.0.0.1:4545/type_definitions/foo.d.ts\"\nimport * as b from \"http://127.0.0.1:4545/type_definitions/foo.js\";\nimport * as c from \"http://127.0.0.1:4545/subdir/type_reference.js\";\nimport * as d from \"http://127.0.0.1:4545/subdir/mod1.ts\";\nimport * as e from \"data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=\";\nimport * as f from \"./file_01.ts\";\nimport * as g from \"http://localhost:4545/x/a/mod.ts\";\nimport * as h from \"./mod🦕.ts\";\n\nconsole.log(a, b, c, d, e, f, g, h);\n"
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], "file:///a/file.ts"],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 0, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n\n**Types**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.d.ts\n"
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end":{ "line": 0, "character": 62 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 3, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/subdir/type_reference.js\n\n**Types**: http&#8203;://127.0.0.1:4545/subdir/type_reference.d.ts\n"
      },
      "range": {
        "start": { "line": 3, "character": 19 },
        "end":{ "line": 3, "character": 67 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 4, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/subdir/mod1.ts\n"
      },
      "range": {
        "start": { "line": 4, "character": 19 },
        "end":{ "line": 4, "character": 57 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 5, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: _(a data url)_\n"
      },
      "range": {
        "start": { "line": 5, "character": 19 },
        "end":{ "line": 5, "character": 132 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 6, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: file&#8203;:///a/file_01.ts\n"
      },
      "range": {
        "start": { "line": 6, "character": 19 },
        "end":{ "line": 6, "character": 33 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 8, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: file&#8203;:///a/mod🦕.ts\n"
      },
      "range": {
        "start": { "line": 8, "character": 19 },
        "end":{ "line": 8, "character": 30 }
      }
    })
  );
  client.shutdown();
}

// This tests for a regression covered by denoland/deno#12753 where the lsp was
// unable to resolve dependencies when there was an invalid syntax in the module
#[test]
fn lsp_hover_deps_preserved_when_invalid_parse() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file1.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export type Foo = { bar(): string };\n"
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file2.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import { Foo } from './file1.ts'; declare const f: Foo; f\n"
    }
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file2.ts"
      },
      "position": { "line": 0, "character": 56 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const f: Foo",
        },
        ""
      ],
      "range": {
        "start": { "line": 0, "character": 56, },
        "end": { "line": 0, "character": 57, }
      }
    })
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file2.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 0, "character": 57 },
            "end": { "line": 0, "character": 58 }
          },
          "text": "."
        }
      ]
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file2.ts"
      },
      "position": { "line": 0, "character": 56 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const f: Foo",
        },
        ""
      ],
      "range": {
        "start": { "line": 0, "character": 56, },
        "end": { "line": 0, "character": 57, }
      }
    })
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/vscode_deno/issues/1068.
#[test]
fn lsp_rename_synbol_file_scheme_edits_only() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        import { SEPARATOR } from "http://localhost:4545/subdir/exports.ts";
        console.log(SEPARATOR);
      "#,
    },
  }));
  let res = client.write_request(
    "textDocument/rename",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
      "position": { "line": 1, "character": 17 },
      "newName": "PATH_SEPARATOR",
    }),
  );
  assert_eq!(
    res,
    json!({
      "documentChanges": [
        {
          "textDocument": {
            "uri": temp_dir.uri().join("file.ts").unwrap(),
            "version": 1,
          },
          "edits": [
            {
              "range": {
                "start": { "line": 1, "character": 17 },
                "end": { "line": 1, "character": 26 },
              },
              "newText": "PATH_SEPARATOR",
            },
            {
              "range": {
                "start": { "line": 2, "character": 20 },
                "end": { "line": 2, "character": 29 },
              },
              "newText": "PATH_SEPARATOR",
            },
          ],
        }
      ],
    })
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/23121.
#[test]
fn lsp_document_preload_limit_zero_deno_json_detection() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_preload_limit(0);
  });
  let res = client
    .read_notification_with_method::<Value>("deno/didChangeDenoConfiguration");
  assert_eq!(
    res,
    Some(json!({
      "changes": [{
        "scopeUri": temp_dir.uri(),
        "fileUri": temp_dir.uri().join("deno.json").unwrap(),
        "type": "added",
        "configurationType": "denoJson",
      }],
    }))
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/23141.
#[test]
fn lsp_import_map_setting_with_deno_json() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  temp_dir.write(
    "import_map.json",
    json!({
      "imports": {
        "file2": "./file2.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("file2.ts", "");
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("import_map.json");
  });
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"file2\";\n",
    },
  }));
  assert_eq!(json!(diagnostics.all()), json!([]));
  client.shutdown();
}

#[test]
fn lsp_hover_typescript_types() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/xTypeScriptTypes.js\";\n\nconsole.log(a.foo);\n",
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["http://127.0.0.1:4545/xTypeScriptTypes.js"],
        "file:///a/file.ts",
      ],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 24 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n\n**Types**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.d.ts\n"
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end": { "line": 0, "character": 62 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_hover_jsr() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"jsr:@denotest/add@1.0.0\";\n",
    }
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: jsr&#8203;:&#8203;@denotest/add&#8203;@1.0.0 (<http://127.0.0.1:4250/@denotest/add/1.0.0/mod.ts>)\n",
      },
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 32 },
      },
    }),
  );
  client.shutdown();
}

#[test]
fn lsp_hover_jsdoc_symbol_link() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/b.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export function hello() {}\n"
    }
  }));
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import { hello } from \"./b.ts\";\n\nhello();\n\nconst b = \"b\";\n\n/** JSDoc {@link hello} and {@linkcode b} */\nfunction a() {}\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 7, "character": 10 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "function a(): void"
        },
        "JSDoc [hello](file:///a/b.ts#L1,1) and [`b`](file:///a/file.ts#L5,7)"
      ],
      "range": {
        "start": { "line": 7, "character": 9 },
        "end": { "line": 7, "character": 10 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_goto_type_definition() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  a: string;\n}\n\nexport class B implements A {\n  a = \"a\";\n  log() {\n    console.log(this.a);\n  }\n}\n\nconst b = new B();\nb;\n",
      }
    }),
  );
  let res = client.write_request(
    "textDocument/typeDefinition",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 12, "character": 1 }
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "targetUri": "file:///a/file.ts",
        "targetRange": {
          "start": { "line": 4, "character": 0 },
          "end": { "line": 9, "character": 1 }
        },
        "targetSelectionRange": {
          "start": { "line": 4, "character": 13 },
          "end": { "line": 4, "character": 14 }
        }
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_call_hierarchy() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "function foo() {\n  return false;\n}\n\nclass Bar {\n  baz() {\n    return foo();\n  }\n}\n\nfunction main() {\n  const bar = new Bar();\n  bar.baz();\n}\n\nmain();"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/prepareCallHierarchy",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 5, "character": 3 }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "name": "baz",
      "kind": 6,
      "detail": "Bar",
      "uri": "file:///a/file.ts",
      "range": {
        "start": { "line": 5, "character": 2 },
        "end": { "line": 7, "character": 3 }
      },
      "selectionRange": {
        "start": { "line": 5, "character": 2 },
        "end": { "line": 5, "character": 5 }
      }
    }])
  );
  let res = client.write_request(
    "callHierarchy/incomingCalls",
    json!({
      "item": {
        "name": "baz",
        "kind": 6,
        "detail": "Bar",
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 7, "character": 3 }
        },
        "selectionRange": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 5, "character": 5 }
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "from": {
        "name": "main",
        "kind": 12,
        "detail": "",
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 10, "character": 0 },
          "end": { "line": 13, "character": 1 }
        },
        "selectionRange": {
          "start": { "line": 10, "character": 9 },
          "end": { "line": 10, "character": 13 }
        }
      },
      "fromRanges": [
        {
          "start": { "line": 12, "character": 6 },
          "end": { "line": 12, "character": 9 }
        }
      ]
    }])
  );
  let res = client.write_request(
    "callHierarchy/outgoingCalls",
    json!({
      "item": {
        "name": "baz",
        "kind": 6,
        "detail": "Bar",
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 7, "character": 3 }
        },
        "selectionRange": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 5, "character": 5 }
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "to": {
        "name": "foo",
        "kind": 12,
        "detail": "",
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 2, "character": 1 }
        },
        "selectionRange": {
          "start": { "line": 0, "character": 9 },
          "end": { "line": 0, "character": 12 }
        }
      },
      "fromRanges": [{
        "start": { "line": 6, "character": 11 },
        "end": { "line": 6, "character": 14 }
      }]
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_large_doc_changes() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let large_file_text =
    fs::read_to_string(testdata_path().join("lsp").join("large_file.txt"))
      .unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "javascript",
      "version": 1,
      "text": large_file_text,
    }
  }));
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 444, "character": 11 },
            "end": { "line": 444, "character": 14 }
          },
          "text": "+++"
        }
      ]
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 445, "character": 4 },
            "end": { "line": 445, "character": 4 }
          },
          "text": "// "
        }
      ]
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 477, "character": 4 },
            "end": { "line": 477, "character": 9 }
          },
          "text": "error"
        }
      ]
    }),
  );
  client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 421, "character": 30 }
    }),
  );
  client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 444, "character": 6 }
    }),
  );
  client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 461, "character": 34 }
    }),
  );
  client.shutdown();

  assert!(client.duration().as_millis() <= 15000);
}

#[test]
fn lsp_document_symbol() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface IFoo {\n  foo(): boolean;\n}\n\nclass Bar implements IFoo {\n  constructor(public x: number) { }\n  foo() { return true; }\n  /** @deprecated */\n  baz() { return false; }\n  get value(): number { return 0; }\n  set value(_newValue: number) { return; }\n  static staticBar = new Bar(0);\n  private static getStaticBar() { return Bar.staticBar; }\n}\n\nenum Values { value1, value2 }\n\nvar bar: IFoo = new Bar(3);"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/documentSymbol",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "name": "bar",
      "kind": 13,
      "range": {
        "start": { "line": 17, "character": 4 },
        "end": { "line": 17, "character": 26 }
      },
      "selectionRange": {
        "start": { "line": 17, "character": 4 },
        "end": { "line": 17, "character": 7 }
      }
    }, {
      "name": "Bar",
      "kind": 5,
      "range": {
        "start": { "line": 4, "character": 0 },
        "end": { "line": 13, "character": 1 }
      },
      "selectionRange": {
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 9 }
      },
      "children": [{
        "name": "constructor",
        "kind": 9,
        "range": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 5, "character": 35 }
        },
        "selectionRange": {
          "start": { "line": 5, "character": 2 },
          "end": { "line": 5, "character": 35 }
        }
      }, {
        "name": "baz",
        "kind": 6,
        "tags": [1],
        "range": {
          "start": { "line": 8, "character": 2 },
          "end": { "line": 8, "character": 25 }
        },
        "selectionRange": {
          "start": { "line": 8, "character": 2 },
          "end": { "line": 8, "character": 5 }
        }
      }, {
        "name": "foo",
        "kind": 6,
        "range": {
          "start": { "line": 6, "character": 2 },
          "end": { "line": 6, "character": 24 }
        },
        "selectionRange": {
          "start": { "line": 6, "character": 2 },
          "end": { "line": 6, "character": 5 }
        }
      }, {
        "name": "getStaticBar",
        "kind": 6,
        "range": {
          "start": { "line": 12, "character": 2 },
          "end": { "line": 12, "character": 57 }
        },
        "selectionRange": {
          "start": { "line": 12, "character": 17 },
          "end": { "line": 12, "character": 29 }
        }
      }, {
        "name": "staticBar",
        "kind": 8,
        "range": {
          "start": { "line": 11, "character": 2 },
          "end": { "line": 11, "character": 32 }
        },
        "selectionRange": {
          "start": { "line": 11, "character": 9 },
          "end": { "line": 11, "character": 18 }
        }
      }, {
        "name": "(get) value",
        "kind": 8,
        "range": {
          "start": { "line": 9, "character": 2 },
          "end": { "line": 9, "character": 35 }
        },
        "selectionRange": {
          "start": { "line": 9, "character": 6 },
          "end": { "line": 9, "character": 11 }
        }
      }, {
        "name": "(set) value",
        "kind": 8,
        "range": {
          "start": { "line": 10, "character": 2 },
          "end": { "line": 10, "character": 42 }
        },
        "selectionRange": {
          "start": { "line": 10, "character": 6 },
          "end": { "line": 10, "character": 11 }
        }
      }, {
        "name": "x",
        "kind": 8,
        "range": {
          "start": { "line": 5, "character": 14 },
          "end": { "line": 5, "character": 30 }
        },
        "selectionRange": {
          "start": { "line": 5, "character": 21 },
          "end": { "line": 5, "character": 22 }
        }
      }]
    }, {
      "name": "IFoo",
      "kind": 11,
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 2, "character": 1 }
      },
      "selectionRange": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 14 }
      },
      "children": [{
        "name": "foo",
        "kind": 6,
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 17 }
        },
        "selectionRange": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 5 }
        }
      }]
    }, {
      "name": "Values",
      "kind": 10,
      "range": {
        "start": { "line": 15, "character": 0 },
        "end": { "line": 15, "character": 30 }
      },
      "selectionRange": {
        "start": { "line": 15, "character": 5 },
        "end": { "line": 15, "character": 11 }
      },
      "children": [{
        "name": "value1",
        "kind": 22,
        "range": {
          "start": { "line": 15, "character": 14 },
          "end": { "line": 15, "character": 20 }
        },
        "selectionRange": {
          "start": { "line": 15, "character": 14 },
          "end": { "line": 15, "character": 20 }
        }
      }, {
        "name": "value2",
        "kind": 22,
        "range": {
          "start": { "line": 15, "character": 22 },
          "end": { "line": 15, "character": 28 }
        },
        "selectionRange": {
          "start": { "line": 15, "character": 22 },
          "end": { "line": 15, "character": 28 }
        }
      }]
    }]
    )
  );
  client.shutdown();
}

#[test]
fn lsp_folding_range() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "// #region 1\n/*\n * Some comment\n */\nclass Foo {\n  bar(a, b) {\n    if (a === b) {\n      return true;\n    }\n    return false;\n  }\n}\n// #endregion"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/foldingRange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "startLine": 0,
      "endLine": 12,
      "kind": "region"
    }, {
      "startLine": 1,
      "endLine": 3,
      "kind": "comment"
    }, {
      "startLine": 4,
      "endLine": 10
    }, {
      "startLine": 5,
      "endLine": 9
    }, {
      "startLine": 6,
      "endLine": 7
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_rename() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        // this should not rename in comments and strings
        "text": "let variable = 'a'; // variable\nconsole.log(variable);\n\"variable\";\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/rename",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 4 },
      "newName": "variable_modified"
    }),
  );
  assert_eq!(
    res,
    json!({
      "documentChanges": [{
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 1
        },
        "edits": [{
          "range": {
            "start": { "line": 0, "character": 4 },
            "end": { "line": 0, "character": 12 }
          },
          "newText": "variable_modified"
        }, {
          "range": {
            "start": { "line": 1, "character": 12 },
            "end": { "line": 1, "character": 20 }
          },
          "newText": "variable_modified"
        }]
      }]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_selection_range() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "class Foo {\n  bar(a, b) {\n    if (a === b) {\n      return true;\n    }\n    return false;\n  }\n}"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/selectionRange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "positions": [{ "line": 2, "character": 8 }]
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 2, "character": 8 },
        "end": { "line": 2, "character": 9 }
      },
      "parent": {
        "range": {
          "start": { "line": 2, "character": 8 },
          "end": { "line": 2, "character": 15 }
        },
        "parent": {
          "range": {
            "start": { "line": 2, "character": 4 },
            "end": { "line": 4, "character": 5 }
          },
          "parent": {
            "range": {
              "start": { "line": 1, "character": 13 },
              "end": { "line": 6, "character": 2 }
            },
            "parent": {
              "range": {
                "start": { "line": 1, "character": 12 },
                "end": { "line": 6, "character": 3 }
              },
              "parent": {
                "range": {
                  "start": { "line": 1, "character": 2 },
                  "end": { "line": 6, "character": 3 }
                },
                "parent": {
                  "range": {
                    "start": { "line": 0, "character": 11 },
                    "end": { "line": 7, "character": 0 }
                  },
                  "parent": {
                    "range": {
                      "start": { "line": 0, "character": 0 },
                      "end": { "line": 7, "character": 1 }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_semantic_tokens() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "enum Values { value1, value2 }\n\nasync function baz(s: string): Promise<string> {\n  const r = s.slice(0);\n  return r;\n}\n\ninterface IFoo {\n  readonly x: number;\n  foo(): boolean;\n}\n\nclass Bar implements IFoo {\n  constructor(public readonly x: number) { }\n  foo() { return true; }\n  static staticBar = new Bar(0);\n  private static getStaticBar() { return Bar.staticBar; }\n}\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "data": [
        0, 5, 6, 1, 1, 0, 9, 6, 8, 9, 0, 8, 6, 8, 9, 2, 15, 3, 10, 5, 0, 4, 1,
        6, 1, 0, 12, 7, 2, 16, 1, 8, 1, 7, 41, 0, 4, 1, 6, 0, 0, 2, 5, 11, 16,
        1, 9, 1, 7, 40, 3, 10, 4, 2, 1, 1, 11, 1, 9, 9, 1, 2, 3, 11, 1, 3, 6, 3,
        0, 1, 0, 15, 4, 2, 0, 1, 30, 1, 6, 9, 1, 2, 3, 11,1, 1, 9, 9, 9, 3, 0,
        16, 3, 0, 0, 1, 17, 12, 11, 3, 0, 24, 3, 0, 0, 0, 4, 9, 9, 2
      ]
    })
  );
  let res = client.write_request(
    "textDocument/semanticTokens/range",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 6, "character": 0 }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "data": [
        0, 5, 6, 1, 1, 0, 9, 6, 8, 9, 0, 8, 6, 8, 9, 2, 15, 3, 10, 5, 0, 4, 1,
        6, 1, 0, 12, 7, 2, 16, 1, 8, 1, 7, 41, 0, 4, 1, 6, 0, 0, 2, 5, 11, 16,
        1, 9, 1, 7, 40
      ]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_code_lens_references() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "references": true,
      }
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": concat!(
        "class A {\n",
        "  a = \"a\";\n",
        "\n",
        "  b() {\n",
        "    console.log(this.a);\n",
        "  }\n",
        "\n",
        "  c() {\n",
        "    this.a = \"c\";\n",
        "  }\n",
        "}\n",
        "\n",
        "const a = new A();\n",
        "a.b();\n",
        "const b = 2;\n",
        "const c = 3;\n",
        "c; c;",
      ),
    }
  }));
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 6 },
        "end": { "line": 0, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 1, "character": 2 },
        "end": { "line": 1, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 3, "character": 2 },
        "end": { "line": 3, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 7, "character": 2 },
        "end": { "line": 7, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }])
  );
  let res = client.write_request(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 0, "character": 6 },
        "end": { "line": 0, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "range": {
        "start": { "line": 0, "character": 6 },
        "end": { "line": 0, "character": 7 }
      },
      "command": {
        "title": "1 reference",
        "command": "deno.client.showReferences",
        "arguments": [
          "file:///a/file.ts",
          { "line": 0, "character": 6 },
          [{
            "uri": "file:///a/file.ts",
            "range": {
              "start": { "line": 12, "character": 14 },
              "end": { "line": 12, "character": 15 }
            }
          }]
        ]
      }
    })
  );

  // 0 references
  let res = client.write_request(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 14, "character": 6 },
        "end": { "line": 14, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "range": {
        "start": { "line": 14, "character": 6 },
        "end": { "line": 14, "character": 7 }
      },
      "command": {
        "title": "0 references",
        "command": "",
      }
    })
  );

  // 2 references
  let res = client.write_request(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 15, "character": 6 },
        "end": { "line": 15, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "range": {
        "start": { "line": 15, "character": 6 },
        "end": { "line": 15, "character": 7 }
      },
      "command": {
        "title": "2 references",
        "command": "deno.client.showReferences",
        "arguments": [
          "file:///a/file.ts",
          { "line": 15, "character": 6 },
          [{
            "uri": "file:///a/file.ts",
            "range": {
              "start": { "line": 16, "character": 0 },
              "end": { "line": 16, "character": 1 }
            }
          },{
            "uri": "file:///a/file.ts",
            "range": {
              "start": { "line": 16, "character": 3 },
              "end": { "line": 16, "character": 4 }
            }
          }]
        ]
      }
    })
  );

  client.shutdown();
}

#[test]
fn lsp_code_lens_implementations() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "implementations": true,
        "references": true,
      }
    },
  }));
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b(): void;\n}\n\nclass B implements A {\n  b() {\n    console.log(\"b\");\n  }\n}\n\ninterface C {\n  c: string;\n}\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([ {
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }, {
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 1, "character": 2 },
        "end": { "line": 1, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 5, "character": 2 },
        "end": { "line": 5, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }, {
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 11, "character": 2 },
        "end": { "line": 11, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }])
  );
  let res = client.write_request(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "command": {
        "title": "1 implementation",
        "command": "deno.client.showReferences",
        "arguments": [
          "file:///a/file.ts",
          { "line": 0, "character": 10 },
          [{
            "uri": "file:///a/file.ts",
            "range": {
              "start": { "line": 4, "character": 6 },
              "end": { "line": 4, "character": 7 }
            }
          }]
        ]
      }
    })
  );
  let res = client.write_request(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "command": {
        "title": "0 implementations",
        "command": ""
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_code_lens_test() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.disable_testing_api().set_code_lens(None);
  });
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "const { test } = Deno;\nconst { test: test2 } = Deno;\nconst test3 = Deno.test;\n\nDeno.test(\"test a\", () => {});\nDeno.test({\n  name: \"test b\",\n  fn() {},\n});\ntest({\n  name: \"test c\",\n  fn() {},\n});\ntest(\"test d\", () => {});\ntest2({\n  name: \"test e\",\n  fn() {},\n});\ntest2(\"test f\", () => {});\ntest3({\n  name: \"test g\",\n  fn() {},\n});\ntest3(\"test h\", () => {});\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 4, "character": 5 },
        "end": { "line": 4, "character": 9 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test a",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 4, "character": 5 },
        "end": { "line": 4, "character": 9 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test a",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 5, "character": 5 },
        "end": { "line": 5, "character": 9 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test b",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 5, "character": 5 },
        "end": { "line": 5, "character": 9 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test b",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 9, "character": 0 },
        "end": { "line": 9, "character": 4 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test c",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 9, "character": 0 },
        "end": { "line": 9, "character": 4 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test c",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 13, "character": 0 },
        "end": { "line": 13, "character": 4 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test d",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 13, "character": 0 },
        "end": { "line": 13, "character": 4 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test d",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 14, "character": 0 },
        "end": { "line": 14, "character": 5 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test e",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 14, "character": 0 },
        "end": { "line": 14, "character": 5 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test e",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 18, "character": 0 },
        "end": { "line": 18, "character": 5 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test f",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 18, "character": 0 },
        "end": { "line": 18, "character": 5 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test f",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 19, "character": 0 },
        "end": { "line": 19, "character": 5 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test g",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 19, "character": 0 },
        "end": { "line": 19, "character": 5 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test g",
          { "inspect": true }
        ]
      }
    }, {
      "range": {
        "start": { "line": 23, "character": 0 },
        "end": { "line": 23, "character": 5 }
      },
      "command": {
        "title": "▶︎ Run Test",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test h",
          { "inspect": false }
        ]
      }
    }, {
      "range": {
        "start": { "line": 23, "character": 0 },
        "end": { "line": 23, "character": 5 }
      },
      "command": {
        "title": "Debug",
        "command": "deno.client.test",
        "arguments": [
          "file:///a/file.ts",
          "test h",
          { "inspect": true }
        ]
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_lens_test_disabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.disable_testing_api().set_code_lens(Some(json!({
      "implementations": true,
      "references": true,
      "test": false
    })));
  });
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "test": false,
      },
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "const { test } = Deno;\nconst { test: test2 } = Deno;\nconst test3 = Deno.test;\n\nDeno.test(\"test a\", () => {});\nDeno.test({\n  name: \"test b\",\n  fn() {},\n});\ntest({\n  name: \"test c\",\n  fn() {},\n});\ntest(\"test d\", () => {});\ntest2({\n  name: \"test e\",\n  fn() {},\n});\ntest2(\"test f\", () => {});\ntest3({\n  name: \"test g\",\n  fn() {},\n});\ntest3(\"test h\", () => {});\n"
    },
  }));
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_code_lens_non_doc_nav_tree() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "implementations": true,
        "references": true,
      }
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n"
    }
  }));
  client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 3 },
      "context": {
        "includeDeclaration": true
      }
    }),
  );
  client.write_request(
    "deno/virtualTextDocument",
    json!({
      "textDocument": {
        "uri": "deno:/asset/lib.deno.shared_globals.d.ts"
      }
    }),
  );
  let res = client.write_request_with_res_as::<Vec<lsp::CodeLens>>(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "deno:/asset/lib.deno.shared_globals.d.ts"
      }
    }),
  );
  assert!(res.len() > 50);
  client.write_request_with_res_as::<lsp::CodeLens>(
    "codeLens/resolve",
    json!({
      "range": {
        "start": { "line": 416, "character": 12 },
        "end": { "line": 416, "character": 19 }
      },
      "data": {
        "specifier": "asset:///lib.deno.shared_globals.d.ts",
        "source": "references"
      }
    }),
  );
  client.shutdown();
}

#[test]
fn lsp_nav_tree_updates() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "codeLens": {
        "implementations": true,
        "references": true,
      }
    },
  }));
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b(): void;\n}\n\nclass B implements A {\n  b() {\n    console.log(\"b\");\n  }\n}\n\ninterface C {\n  c: string;\n}\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([ {
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }, {
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 1, "character": 2 },
        "end": { "line": 1, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 5, "character": 2 },
        "end": { "line": 5, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }, {
      "range": {
        "start": { "line": 10, "character": 10 },
        "end": { "line": 10, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 11, "character": 2 },
        "end": { "line": 11, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }])
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 10, "character": 0 },
            "end": { "line": 13, "character": 0 }
          },
          "text": ""
        }
      ]
    }),
  );
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "implementations"
      }
    }, {
      "range": {
        "start": { "line": 0, "character": 10 },
        "end": { "line": 0, "character": 11 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 1, "character": 2 },
        "end": { "line": 1, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }, {
      "range": {
        "start": { "line": 5, "character": 2 },
        "end": { "line": 5, "character": 3 }
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "source": "references"
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_find_references() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/mod.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r"export const a = 1;\nconst b = 2;"
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/mod.test.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"import { a } from './mod.ts'; console.log(a);"#
    }
  }));

  // test without including the declaration
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": "file:///a/mod.ts",
      },
      "position": { "line": 0, "character": 13 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
      "uri": "file:///a/mod.test.ts",
      "range": {
        "start": { "line": 0, "character": 9 },
        "end": { "line": 0, "character": 10 }
      }
    }, {
      "uri": "file:///a/mod.test.ts",
      "range": {
        "start": { "line": 0, "character": 42 },
        "end": { "line": 0, "character": 43 }
      }
    }])
  );

  // test with including the declaration
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": "file:///a/mod.ts",
      },
      "position": { "line": 0, "character": 13 },
      "context": {
        "includeDeclaration": true
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
      "uri": "file:///a/mod.ts",
      "range": {
        "start": { "line": 0, "character": 13 },
        "end": { "line": 0, "character": 14 }
      }
    }, {
      "uri": "file:///a/mod.test.ts",
      "range": {
        "start": { "line": 0, "character": 9 },
        "end": { "line": 0, "character": 10 }
      }
    }, {
      "uri": "file:///a/mod.test.ts",
      "range": {
        "start": { "line": 0, "character": 42 },
        "end": { "line": 0, "character": 43 }
      }
    }])
  );

  // test 0 references
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": "file:///a/mod.ts",
      },
      "position": { "line": 1, "character": 6 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  assert_eq!(res, json!(null)); // seems it always returns null for this, which is ok

  client.shutdown();
}

#[test]
fn lsp_signature_help() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "/**\n * Adds two numbers.\n * @param a This is a first number.\n * @param b This is a second number.\n */\nfunction add(a: number, b: number) {\n  return a + b;\n}\n\nadd("
      }
    }),
  );
  let res = client.write_request(
    "textDocument/signatureHelp",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "character": 4, "line": 9 },
      "context": {
        "triggerKind": 2,
        "triggerCharacter": "(",
        "isRetrigger": false
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "signatures": [
        {
          "label": "add(a: number, b: number): number",
          "documentation": {
            "kind": "markdown",
            "value": "Adds two numbers."
          },
          "parameters": [
            {
              "label": "a: number",
              "documentation": {
                "kind": "markdown",
                "value": "This is a first number."
              }
            }, {
              "label": "b: number",
              "documentation": {
                "kind": "markdown",
                "value": "This is a second number."
              }
            }
          ]
        }
      ],
      "activeSignature": 0,
      "activeParameter": 0
    })
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 9, "character": 4 },
            "end": { "line": 9, "character": 4 }
          },
          "text": "123, "
        }
      ]
    }),
  );
  let res = client.write_request(
    "textDocument/signatureHelp",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "character": 8, "line": 9 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "signatures": [
        {
          "label": "add(a: number, b: number): number",
          "documentation": {
            "kind": "markdown",
            "value": "Adds two numbers."
          },
          "parameters": [
            {
              "label": "a: number",
              "documentation": {
                "kind": "markdown",
                "value": "This is a first number."
              }
            }, {
              "label": "b: number",
              "documentation": {
                "kind": "markdown",
                "value": "This is a second number."
              }
            }
          ]
        }
      ],
      "activeSignature": 0,
      "activeParameter": 1
    })
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export function a(): void {\n  await Promise.resolve(\"a\");\n}\n\nexport function b(): void {\n  await Promise.resolve(\"b\");\n}\n"
      }
    }),
  );
  let res = client
    .write_request(      "textDocument/codeAction",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 7 }
        },
        "context": {
          "diagnostics": [{
            "range": {
              "start": { "line": 1, "character": 2 },
              "end": { "line": 1, "character": 7 }
            },
            "severity": 1,
            "code": 1308,
            "source": "deno-ts",
            "message": "'await' expressions are only allowed within async functions and at the top levels of modules.",
            "relatedInformation": []
          }],
          "only": ["quickfix"]
        }
      }),
    )
    ;
  assert_eq!(
    res,
    json!([{
      "title": "Add async modifier to containing function",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 7 }
        },
        "severity": 1,
        "code": 1308,
        "source": "deno-ts",
        "message": "'await' expressions are only allowed within async functions and at the top levels of modules.",
        "relatedInformation": []
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 7 },
              "end": { "line": 0, "character": 7 }
            },
            "newText": "async "
          }, {
            "range": {
              "start": { "line": 0, "character": 21 },
              "end": { "line": 0, "character": 25 }
            },
            "newText": "Promise<void>"
          }]
        }]
      }
    }, {
      "title": "Add all missing 'async' modifiers",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 7 }
        },
        "severity": 1,
        "code": 1308,
        "source": "deno-ts",
        "message": "'await' expressions are only allowed within async functions and at the top levels of modules.",
        "relatedInformation": []
      }],
      "data": {
        "specifier": "file:///a/file.ts",
        "fixId": "fixAwaitInSyncFunction"
      }
    }])
  );
  let res = client
    .write_request(
      "codeAction/resolve",
      json!({
        "title": "Add all missing 'async' modifiers",
        "kind": "quickfix",
        "diagnostics": [{
          "range": {
            "start": { "line": 1, "character": 2 },
            "end": { "line": 1, "character": 7 }
          },
          "severity": 1,
          "code": 1308,
          "source": "deno-ts",
          "message": "'await' expressions are only allowed within async functions and at the top levels of modules.",
          "relatedInformation": []
        }],
        "data": {
          "specifier": "file:///a/file.ts",
          "fixId": "fixAwaitInSyncFunction"
        }
      }),
    );
  assert_eq!(
    res,
    json!({
      "title": "Add all missing 'async' modifiers",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": {
              "line": 1,
              "character": 2
            },
            "end": {
              "line": 1,
              "character": 7
            }
          },
          "severity": 1,
          "code": 1308,
          "source": "deno-ts",
          "message": "'await' expressions are only allowed within async functions and at the top levels of modules.",
          "relatedInformation": []
        }
      ],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 7 },
              "end": { "line": 0, "character": 7 }
            },
            "newText": "async "
          }, {
            "range": {
              "start": { "line": 0, "character": 21 },
              "end": { "line": 0, "character": 25 }
            },
            "newText": "Promise<void>"
          }, {
            "range": {
              "start": { "line": 4, "character": 7 },
              "end": { "line": 4, "character": 7 }
            },
            "newText": "async "
          }, {
            "range": {
              "start": { "line": 4, "character": 21 },
              "end": { "line": 4, "character": 25 }
            },
            "newText": "Promise<void>"
          }]
        }]
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "fixId": "fixAwaitInSyncFunction"
      }
    })
  );
  client.shutdown();
}

#[test]
fn test_lsp_code_actions_ordering() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"
          import "https://deno.land/x/a/mod.ts";
          let a = "a";
          console.log(a);
          export function b(): void {
            await Promise.resolve("b");
          }
        "#
    }
  }));
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 1, "character": 11 },
        "end": { "line": 6, "character": 12 }
      },
      "context": {
        "diagnostics": diagnostics.all(),
        "only": ["quickfix"]
      }
    }),
  );

  // Simplify the serialization to `{ title, source }` for this test.
  let mut actions: Vec<Value> = serde_json::from_value(res).unwrap();
  for action in &mut actions {
    let action = action.as_object_mut().unwrap();
    let title = action.get("title").unwrap().as_str().unwrap().to_string();
    let diagnostics = action.get("diagnostics").unwrap().as_array().unwrap();
    let diagnostic = diagnostics.first().unwrap().as_object().unwrap();
    let source = diagnostic.get("source").unwrap();
    let source = source.as_str().unwrap().to_string();
    action.clear();
    action.insert("title".to_string(), serde_json::to_value(title).unwrap());
    action.insert("source".to_string(), serde_json::to_value(source).unwrap());
  }
  let res = serde_json::to_value(actions).unwrap();

  // Ensure ordering is "deno" -> "deno-ts" -> "deno-lint".
  assert_eq!(
    res,
    json!([
      {
        "title": "Cache \"https://deno.land/x/a/mod.ts\" and its dependencies.",
        "source": "deno",
      },
      {
        "title": "Add async modifier to containing function",
        "source": "deno-ts",
      },
      {
        "title": "Disable prefer-const for this line",
        "source": "deno-lint",
      },
      {
        "title": "Disable prefer-const for the entire file",
        "source": "deno-lint",
      },
      {
        "title": "Ignore lint errors for the entire file",
        "source": "deno-lint",
      },
      {
        "title": "Disable no-await-in-sync-fn for this line",
        "source": "deno-lint",
      },
      {
        "title": "Disable no-await-in-sync-fn for the entire file",
        "source": "deno-lint",
      },
      {
        "title": "Ignore lint errors for the entire file",
        "source": "deno-lint",
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_status_file() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let res = client.write_request(
    "deno/virtualTextDocument",
    json!({
      "textDocument": {
        "uri": "deno:/status.md"
      }
    }),
  );
  let res = res.as_str().unwrap().to_string();
  assert!(res.starts_with("# Deno Language Server Status"));

  let res = client.write_request(
    "deno/virtualTextDocument",
    json!({
      "textDocument": {
        "uri": "deno:/status.md?1"
      }
    }),
  );
  let res = res.as_str().unwrap().to_string();
  assert!(res.starts_with("# Deno Language Server Status"));
  client.shutdown();
}

#[test]
fn lsp_code_actions_deno_cache() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"https://deno.land/x/a/mod.ts\";\n\nconsole.log(a);\n"
      }
    }));
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    serde_json::from_value(json!({
      "uri": "file:///a/file.ts",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 19 },
          "end": { "line": 0, "character": 49 }
        },
        "severity": 1,
        "code": "no-cache",
        "source": "deno",
        "message": "Uncached or missing remote URL: https://deno.land/x/a/mod.ts",
        "data": { "specifier": "https://deno.land/x/a/mod.ts" }
      }],
      "version": 1
    })).unwrap()
  );

  let res =
    client
    .write_request(      "textDocument/codeAction",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "range": {
          "start": { "line": 0, "character": 19 },
          "end": { "line": 0, "character": 49 }
        },
        "context": {
          "diagnostics": [{
            "range": {
              "start": { "line": 0, "character": 19 },
              "end": { "line": 0, "character": 49 }
            },
            "severity": 1,
            "code": "no-cache",
            "source": "deno",
            "message": "Unable to load the remote module: \"https://deno.land/x/a/mod.ts\".",
            "data": {
              "specifier": "https://deno.land/x/a/mod.ts"
            }
          }],
          "only": ["quickfix"]
        }
      }),
    )
    ;
  assert_eq!(
    res,
    json!([{
      "title": "Cache \"https://deno.land/x/a/mod.ts\" and its dependencies.",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 19 },
          "end": { "line": 0, "character": 49 }
        },
        "severity": 1,
        "code": "no-cache",
        "source": "deno",
        "message": "Unable to load the remote module: \"https://deno.land/x/a/mod.ts\".",
        "data": {
          "specifier": "https://deno.land/x/a/mod.ts"
        }
      }],
      "command": {
        "title": "",
        "command": "deno.cache",
        "arguments": [["https://deno.land/x/a/mod.ts"], "file:///a/file.ts"]
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_deno_cache_jsr() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        import { add } from "jsr:@denotest/add@1";
        console.log(add(1, 2));
      "#,
    },
  }));
  assert_eq!(
    json!(diagnostics.messages_with_source("deno")),
    json!({
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 28 },
          "end": { "line": 1, "character": 49 },
        },
        "severity": 1,
        "code": "no-cache-jsr",
        "source": "deno",
        "message": "Uncached or missing jsr package: @denotest/add@1",
        "data": { "specifier": "jsr:@denotest/add@1" },
      }],
      "version": 1,
    })
  );
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": { "uri": temp_dir.uri().join("file.ts").unwrap() },
      "range": {
        "start": { "line": 1, "character": 28 },
        "end": { "line": 1, "character": 49 },
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 1, "character": 28 },
            "end": { "line": 1, "character": 49 },
          },
          "severity": 1,
          "code": "no-cache-jsr",
          "source": "deno",
          "message": "Uncached or missing jsr package: @denotest/add@1",
          "data": { "specifier": "jsr:@denotest/add@1" },
        }],
        "only": ["quickfix"],
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Cache \"jsr:@denotest/add@1\" and its dependencies.",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 28 },
          "end": { "line": 1, "character": 49 },
        },
        "severity": 1,
        "code": "no-cache-jsr",
        "source": "deno",
        "message": "Uncached or missing jsr package: @denotest/add@1",
        "data": { "specifier": "jsr:@denotest/add@1" },
      }],
      "command": {
        "title": "",
        "command": "deno.cache",
        "arguments": [
          ["jsr:@denotest/add@1"],
          temp_dir.uri().join("file.ts").unwrap(),
        ],
      },
    }])
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["jsr:@denotest/add@1"],
        temp_dir.uri().join("file.ts").unwrap(),
      ],
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(json!(diagnostics.all()), json!([]));
  client.shutdown();
}

#[test]
fn lsp_jsr_lockfile() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("./deno.json", json!({}).to_string());
  let lockfile = temp_dir.path().join("deno.lock");
  let integrity = context.get_jsr_package_integrity("@denotest/add/0.2.0");
  lockfile.write_json(&json!({
    "version": "3",
    "packages": {
      "specifiers": {
        // This is an old version of the package which exports `sum()` instead
        // of `add()`.
        "jsr:@denotest/add": "jsr:@denotest/add@0.2.0",
      },
      "jsr": {
        "@denotest/add@0.2.0": {
          "integrity": integrity
        }
      }
    },
    "remote": {},
  }));
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        import { sum } from "jsr:@denotest/add";
        console.log(sum(1, 2));
      "#,
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        [],
        temp_dir.uri().join("file.ts").unwrap(),
      ],
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(json!(diagnostics.all()), json!([]));
  client.shutdown();
}

#[test]
fn lsp_jsr_auto_import_completion() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "main.ts",
    r#"
      import "jsr:@denotest/add@1";
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        [],
        temp_dir.uri().join("main.ts").unwrap(),
      ],
    }),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"add"#,
    }
  }));
  let list = client.get_completion_list(
    temp_dir.uri().join("file.ts").unwrap(),
    (0, 3),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 267);
  let item = list.items.iter().find(|i| i.label == "add").unwrap();
  assert_eq!(&item.label, "add");
  assert_eq!(
    json!(&item.label_details),
    json!({ "description": "jsr:@denotest/add@1" })
  );

  let res = client.write_request("completionItem/resolve", json!(item));
  assert_eq!(
    res,
    json!({
      "label": "add",
      "labelDetails": { "description": "jsr:@denotest/add@1" },
      "kind": 3,
      "detail": "function add(a: number, b: number): number",
      "documentation": { "kind": "markdown", "value": "" },
      "sortText": "\u{ffff}16_1",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 },
          },
          "newText": "import { add } from \"jsr:@denotest/add@1\";\n\n",
        },
      ],
    })
  );
  client.shutdown();
}

#[test]
fn lsp_jsr_auto_import_completion_import_map() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "imports": {
        "add": "jsr:@denotest/add@^1.0",
      },
    })
    .to_string(),
  );
  temp_dir.write(
    "main.ts",
    r#"
      import "jsr:@denotest/add@1";
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        [],
        temp_dir.uri().join("main.ts").unwrap(),
      ],
    }),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"add"#,
    }
  }));
  let list = client.get_completion_list(
    temp_dir.uri().join("file.ts").unwrap(),
    (0, 3),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 267);
  let item = list.items.iter().find(|i| i.label == "add").unwrap();
  assert_eq!(&item.label, "add");
  assert_eq!(json!(&item.label_details), json!({ "description": "add" }));

  let res = client.write_request("completionItem/resolve", json!(item));
  assert_eq!(
    res,
    json!({
      "label": "add",
      "labelDetails": { "description": "add" },
      "kind": 3,
      "detail": "function add(a: number, b: number): number",
      "documentation": { "kind": "markdown", "value": "" },
      "sortText": "\u{ffff}16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 },
          },
          "newText": "import { add } from \"add\";\n\n",
        },
      ],
    })
  );
  client.shutdown();
}

#[test]
fn lsp_jsr_code_action_missing_declaration() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let file = source_file(
    temp_dir.path().join("file.ts"),
    r#"
      import { someFunction } from "jsr:@denotest/types-file";
      assertReturnType(someFunction());
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], file.uri()],
    }),
  );
  client.did_open_file(&file);
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": file.uri(),
      },
      "range": {
        "start": { "line": 2, "character": 6 },
        "end": { "line": 2, "character": 22 },
      },
      "context": {
        "diagnostics": [
          {
            "range": {
              "start": { "line": 2, "character": 6 },
              "end": { "line": 2, "character": 22 },
            },
            "severity": 8,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'assertReturnType'.",
            "relatedInformation": [],
          },
        ],
        "only": ["quickfix"],
      },
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "title": "Add missing function declaration 'assertReturnType'",
        "kind": "quickfix",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 2,
                "character": 6,
              },
              "end": {
                "line": 2,
                "character": 22,
              },
            },
            "severity": 8,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'assertReturnType'.",
            "relatedInformation": [],
          },
        ],
        "edit": {
          "documentChanges": [
            {
              "textDocument": {
                "uri": file.uri(),
                "version": 1,
              },
              "edits": [
                {
                  "range": {
                    "start": {
                      "line": 1,
                      "character": 6,
                    },
                    "end": {
                      "line": 1,
                      "character": 6,
                    },
                  },
                  "newText": "import { ReturnType } from \"jsr:@denotest/types-file/types\";\n",
                },
                {
                  "range": {
                    "start": {
                      "line": 3,
                      "character": 0,
                    },
                    "end": {
                      "line": 3,
                      "character": 0,
                    },
                  },
                  "newText": "\n      function assertReturnType(arg0: ReturnType) {\n        throw new Error(\"Function not implemented.\");\n      }\n",
                },
              ],
            },
          ],
        },
      },
    ])
  );
}

#[test]
fn lsp_code_actions_deno_cache_npm() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import chalk from \"npm:chalk\";\n\nconsole.log(chalk.green);\n"
    }
  }));
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    serde_json::from_value(json!({
      "uri": "file:///a/file.ts",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 18 },
          "end": { "line": 0, "character": 29 }
        },
        "severity": 1,
        "code": "no-cache-npm",
        "source": "deno",
        "message": "Uncached or missing npm package: chalk",
        "data": { "specifier": "npm:chalk" }
      }],
      "version": 1
    }))
    .unwrap()
  );

  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 18 },
        "end": { "line": 0, "character": 29 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 0, "character": 18 },
            "end": { "line": 0, "character": 29 }
          },
          "severity": 1,
          "code": "no-cache-npm",
          "source": "deno",
          "message": "Uncached or missing npm package: chalk",
          "data": { "specifier": "npm:chalk" }
        }],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Cache \"npm:chalk\" and its dependencies.",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 18 },
          "end": { "line": 0, "character": 29 }
        },
        "severity": 1,
        "code": "no-cache-npm",
        "source": "deno",
        "message": "Uncached or missing npm package: chalk",
        "data": { "specifier": "npm:chalk" }
      }],
      "command": {
        "title": "",
        "command": "deno.cache",
        "arguments": [["npm:chalk"], "file:///a/file.ts"]
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_deno_cache_all() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        import * as a from "https://deno.land/x/a/mod.ts";
        import chalk from "npm:chalk";
        console.log(a);
        console.log(chalk);
      "#,
    }
  }));
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    serde_json::from_value(json!({
      "uri": "file:///a/file.ts",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 1, "character": 27 },
            "end": { "line": 1, "character": 57 },
          },
          "severity": 1,
          "code": "no-cache",
          "source": "deno",
          "message": "Uncached or missing remote URL: https://deno.land/x/a/mod.ts",
          "data": { "specifier": "https://deno.land/x/a/mod.ts" },
        },
        {
          "range": {
            "start": { "line": 2, "character": 26 },
            "end": { "line": 2, "character": 37 },
          },
          "severity": 1,
          "code": "no-cache-npm",
          "source": "deno",
          "message": "Uncached or missing npm package: chalk",
          "data": { "specifier": "npm:chalk" },
        },
      ],
      "version": 1,
    })).unwrap()
  );

  let res =
    client
    .write_request(
      "textDocument/codeAction",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "range": {
          "start": { "line": 1, "character": 27 },
          "end": { "line": 1, "character": 57 },
        },
        "context": {
          "diagnostics": [{
            "range": {
              "start": { "line": 1, "character": 27 },
              "end": { "line": 1, "character": 57 },
            },
            "severity": 1,
            "code": "no-cache",
            "source": "deno",
            "message": "Uncached or missing remote URL: https://deno.land/x/a/mod.ts",
            "data": {
              "specifier": "https://deno.land/x/a/mod.ts",
            },
          }],
          "only": ["quickfix"],
        }
      }),
    );
  assert_eq!(
    res,
    json!([
      {
        "title": "Cache \"https://deno.land/x/a/mod.ts\" and its dependencies.",
        "kind": "quickfix",
        "diagnostics": [{
          "range": {
            "start": { "line": 1, "character": 27 },
            "end": { "line": 1, "character": 57 },
          },
          "severity": 1,
          "code": "no-cache",
          "source": "deno",
          "message": "Uncached or missing remote URL: https://deno.land/x/a/mod.ts",
          "data": {
            "specifier": "https://deno.land/x/a/mod.ts",
          },
        }],
        "command": {
          "title": "",
          "command": "deno.cache",
          "arguments": [["https://deno.land/x/a/mod.ts"], "file:///a/file.ts"],
        }
      },
      {
        "title": "Cache all dependencies of this module.",
        "kind": "quickfix",
        "diagnostics": [
          {
            "range": {
              "start": { "line": 1, "character": 27 },
              "end": { "line": 1, "character": 57 },
            },
            "severity": 1,
            "code": "no-cache",
            "source": "deno",
            "message": "Uncached or missing remote URL: https://deno.land/x/a/mod.ts",
            "data": {
              "specifier": "https://deno.land/x/a/mod.ts",
            },
          },
          {
            "range": {
              "start": { "line": 2, "character": 26 },
              "end": { "line": 2, "character": 37 },
            },
            "severity": 1,
            "code": "no-cache-npm",
            "source": "deno",
            "message": "Uncached or missing npm package: chalk",
            "data": { "specifier": "npm:chalk" },
          },
        ],
        "command": {
          "title": "",
          "command": "deno.cache",
          "arguments": [[], "file:///a/file.ts"],
        }
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_cache_on_save() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "file.ts",
    r#"
      import { printHello } from "http://localhost:4545/subdir/print_hello.ts";
      printHello();
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
      "cacheOnSave": true,
    },
  }));

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    }
  }));
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    serde_json::from_value(json!({
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 33 },
          "end": { "line": 1, "character": 78 }
        },
        "severity": 1,
        "code": "no-cache",
        "source": "deno",
        "message": "Uncached or missing remote URL: http://localhost:4545/subdir/print_hello.ts",
        "data": { "specifier": "http://localhost:4545/subdir/print_hello.ts" }
      }],
      "version": 1
    }))
    .unwrap()
  );
  client.did_save(json!({
    "textDocument": { "uri": temp_dir.uri().join("file.ts").unwrap() },
  }));
  assert_eq!(client.read_diagnostics().all(), vec![]);

  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/22122.
#[test]
fn lsp_cache_then_definition() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"import "http://localhost:4545/run/002_hello.ts";"#,
    },
  }));
  // Prior to the fix, this would cause a faulty memoization that maps the
  // URL "http://localhost:4545/run/002_hello.ts" to itself, preventing it from
  // being reverse-mapped to "deno:/http/localhost%3A4545/run/002_hello.ts" on
  // "textDocument/definition" request.
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["http://localhost:4545/run/002_hello.ts"],
        temp_dir.uri().join("file.ts").unwrap(),
      ],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": { "uri": temp_dir.uri().join("file.ts").unwrap() },
      "position": { "line": 0, "character": 8 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": "deno:/http/localhost%3A4545/run/002_hello.ts",
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_imports() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
      "textDocument": {
        "uri": "file:///a/file00.ts",
        "languageId": "typescript",
        "version": 1,
        "text": r#"export interface MallardDuckConfigOptions extends DuckConfigOptions {
  kind: "mallard";
}

export class MallardDuckConfig extends DuckConfig {
  constructor(options: MallardDuckConfigOptions) {
    super(options);
  }
}
"#
      }
    }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"import { DuckConfigOptions } from "./file02.ts";

export class DuckConfig {
  readonly kind;
  constructor(options: DuckConfigOptions) {
    this.kind = options.kind;
  }
}
"#
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file02.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"export interface DuckConfigOptions {
  kind: string;
  quacks: boolean;
}
"#
    }
  }));

  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file00.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 6, "character": 0 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 0, "character": 50 },
            "end": { "line": 0, "character": 67 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'DuckConfigOptions'."
        }, {
          "range": {
            "start": { "line": 4, "character": 39 },
            "end": { "line": 4, "character": 49 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'DuckConfig'."
        }],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"./file02.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 50 },
          "end": { "line": 0, "character": 67 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file00.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "import { DuckConfigOptions } from \"./file02.ts\";\n\n"
          }]
        }]
      }
    }, {
      "title": "Add import from \"./file01.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 4, "character": 39 },
          "end": { "line": 4, "character": 49 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfig'."
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file00.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "import { DuckConfig } from \"./file01.ts\";\n\n"
          }]
        }]
      }
    }, {
      "title": "Add all missing imports",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 50 },
          "end": { "line": 0, "character": 67 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }],
      "data": {
        "specifier": "file:///a/file00.ts",
        "fixId": "fixMissingImport"
      }
    }])
  );
  let res = client.write_request(
    "codeAction/resolve",
    json!({
      "title": "Add all missing imports",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 50 },
          "end": { "line": 0, "character": 67 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }, {
        "range": {
          "start": { "line": 4, "character": 39 },
          "end": { "line": 4, "character": 49 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfig'."
      }],
      "data": {
        "specifier": "file:///a/file00.ts",
        "fixId": "fixMissingImport"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "title": "Add all missing imports",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 50 },
          "end": { "line": 0, "character": 67 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }, {
        "range": {
          "start": { "line": 4, "character": 39 },
          "end": { "line": 4, "character": 49 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfig'."
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file00.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "import { DuckConfig } from \"./file01.ts\";\nimport { DuckConfigOptions } from \"./file02.ts\";\n\n"
          }]
        }]
      },
      "data": {
        "specifier": "file:///a/file00.ts",
        "fixId": "fixMissingImport"
      }
    })
  );

  client.shutdown();
}

#[test]
fn lsp_code_actions_imports_dts() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  source_file(
    temp_dir.path().join("decl.d.ts"),
    "export type SomeType = 1;\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        const a: SomeType = 1;
        console.log(a);
      "#,
    }
  }));
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
      "range": {
        "start": { "line": 1, "character": 17 },
        "end": { "line": 1, "character": 25 },
      },
      "context": {
        "diagnostics": diagnostics.all(),
        "only": ["quickfix"],
      },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"./decl.d.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 17 },
          "end": { "line": 1, "character": 25 },
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'SomeType'.",
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": temp_dir.uri().join("file.ts").unwrap(),
            "version": 1,
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 },
            },
            "newText": "import { SomeType } from \"./decl.d.ts\";\n",
          }],
        }],
      },
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_refactor() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "var x: { a?: number; b?: string } = {};\n"
    }
  }));
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 1, "character": 0 }
      },
      "context": {
        "diagnostics": [],
        "only": ["refactor"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Move to a new file",
      "kind": "refactor.move.newFile",
      "isPreferred": false,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Move to a new file",
        "actionName": "Move to a new file"
      }
    }, {
      "title": "Extract to function in module scope",
      "kind": "refactor.extract.function",
      "isPreferred": false,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Extract Symbol",
        "actionName": "function_scope_0"
      }
    }, {
      "title": "Extract to constant in enclosing scope",
      "kind": "refactor.extract.constant",
      "isPreferred": false,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Extract Symbol",
        "actionName": "constant_scope_0"
      }
    }, {
      "title": "Convert default export to named export",
      "kind": "refactor.rewrite.export.named",
      "isPreferred": false,
      "disabled": {
        "reason": "This file already has a default export"
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Convert export",
        "actionName": "Convert default export to named export"
      }
    }, {
      "title": "Convert named export to default export",
      "kind": "refactor.rewrite.export.default",
      "isPreferred": false,
      "disabled": {
        "reason": "This file already has a default export"
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Convert export",
        "actionName": "Convert named export to default export"
      }
    }, {
      "title": "Convert namespace import to named imports",
      "kind": "refactor.rewrite.import.named",
      "isPreferred": false,
      "disabled": {
        "reason": "Selection is not an import declaration."
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Convert import",
        "actionName": "Convert namespace import to named imports"
      }
    }, {
      "title": "Convert named imports to default import",
      "kind": "refactor.rewrite.import.default",
      "isPreferred": false,
      "disabled": {
        "reason": "Selection is not an import declaration."
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Convert import",
        "actionName": "Convert named imports to default import"
      }
    }, {
      "title": "Convert named imports to namespace import",
      "kind": "refactor.rewrite.import.namespace",
      "isPreferred": false,
      "disabled": {
        "reason": "Selection is not an import declaration."
      },
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "refactorName": "Convert import",
        "actionName": "Convert named imports to namespace import"
      }
    }])
  );
  let res = client.write_request(
    "codeAction/resolve",
    json!({
      "title": "Extract to interface",
      "kind": "refactor.extract.interface",
      "isPreferred": true,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 33 }
        },
        "refactorName": "Extract type",
        "actionName": "Extract to interface"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "title": "Extract to interface",
      "kind": "refactor.extract.interface",
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": "file:///a/file.ts",
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "interface NewType {\n  a?: number;\n  b?: string;\n}\n\n"
          }, {
            "range": {
              "start": { "line": 0, "character": 7 },
              "end": { "line": 0, "character": 33 }
            },
            "newText": "NewType"
          }]
        }]
      },
      "isPreferred": true,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 33 }
        },
        "refactorName": "Extract type",
        "actionName": "Extract to interface"
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_imports_respects_fmt_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "./deno.jsonc",
    json!({
      "fmt": {
        "semiColons": false,
        "singleQuote": true,
      }
    })
    .to_string(),
  );
  temp_dir.write(
    "file00.ts",
    r#"
    export interface MallardDuckConfigOptions extends DuckConfigOptions {
      kind: "mallard";
    }
  "#,
  );
  temp_dir.write(
    "file01.ts",
    r#"
    export interface DuckConfigOptions {
      kind: string;
      quacks: boolean;
    }
  "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file00.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file00.ts"),
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file01.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file01.ts"),
    }
  }));

  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file00.ts").unwrap()
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 4, "character": 0 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 1, "character": 55 },
            "end": { "line": 1, "character": 64 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'DuckConfigOptions'."
        }],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"./file01.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 55 },
          "end": { "line": 1, "character": 64 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": temp_dir.uri().join("file00.ts").unwrap(),
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "import { DuckConfigOptions } from './file01.ts'\n"
          }]
        }]
      }
    }])
  );
  let res = client.write_request(
    "codeAction/resolve",
    json!({
      "title": "Add all missing imports",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 55 },
          "end": { "line": 1, "character": 64 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }],
      "data": {
        "specifier": temp_dir.uri().join("file00.ts").unwrap(),
        "fixId": "fixMissingImport"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "title": "Add all missing imports",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 55 },
          "end": { "line": 1, "character": 64 }
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'."
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": temp_dir.uri().join("file00.ts").unwrap(),
            "version": 1
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "import { DuckConfigOptions } from './file01.ts'\n"
          }]
        }]
      },
      "data": {
        "specifier": temp_dir.uri().join("file00.ts").unwrap(),
        "fixId": "fixMissingImport"
      }
    })
  );

  client.shutdown();
}

#[test]
fn lsp_quote_style_from_workspace_settings() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "file00.ts",
    r#"
      export interface MallardDuckConfigOptions extends DuckConfigOptions {
        kind: "mallard";
      }
    "#,
  );
  temp_dir.write(
    "file01.ts",
    r#"
      export interface DuckConfigOptions {
        kind: string;
        quacks: boolean;
      }
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.change_configuration(json!({
    "deno": {
      "enable": true,
    },
    "typescript": {
      "preferences": {
        "quoteStyle": "single",
      },
    },
  }));

  let code_action_params = json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file00.ts").unwrap(),
    },
    "range": {
      "start": { "line": 0, "character": 0 },
      "end": { "line": 4, "character": 0 },
    },
    "context": {
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 56 },
          "end": { "line": 1, "character": 73 },
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'.",
      }],
      "only": ["quickfix"],
    },
  });

  let res =
    client.write_request("textDocument/codeAction", code_action_params.clone());
  // Expect single quotes in the auto-import.
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"./file01.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 56 },
          "end": { "line": 1, "character": 73 },
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'.",
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": temp_dir.uri().join("file00.ts").unwrap(),
            "version": null,
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 },
            },
            "newText": "import { DuckConfigOptions } from './file01.ts';\n",
          }],
        }],
      },
    }]),
  );

  // It should ignore the workspace setting if a `deno.json` is present.
  temp_dir.write("./deno.json", json!({}).to_string());
  client.did_change_watched_files(json!({
    "changes": [{
      "uri": temp_dir.uri().join("deno.json").unwrap(),
      "type": 1,
    }],
  }));

  let res = client.write_request("textDocument/codeAction", code_action_params);
  // Expect double quotes in the auto-import.
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"./file01.ts\"",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 56 },
          "end": { "line": 1, "character": 73 },
        },
        "severity": 1,
        "code": 2304,
        "source": "deno-ts",
        "message": "Cannot find name 'DuckConfigOptions'.",
      }],
      "edit": {
        "documentChanges": [{
          "textDocument": {
            "uri": temp_dir.uri().join("file00.ts").unwrap(),
            "version": null,
          },
          "edits": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 },
            },
            "newText": "import { DuckConfigOptions } from \"./file01.ts\";\n",
          }],
        }],
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_refactor_no_disabled_support() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.with_capabilities(|c| {
      let doc = c.text_document.as_mut().unwrap();
      let code_action = doc.code_action.as_mut().unwrap();
      code_action.disabled_support = Some(false);
    });
  });
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  a: string;\n}\n\ninterface B {\n  b: string;\n}\n\nclass AB implements A, B {\n  a = \"a\";\n  b = \"b\";\n}\n\nnew AB().a;\n"
      }
    }),
  );
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 14, "character": 0 }
      },
      "context": {
        "diagnostics": [],
        "only": ["refactor"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Move to a new file",
      "kind": "refactor.move.newFile",
      "isPreferred": false,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 14, "character": 0 }
        },
        "refactorName": "Move to a new file",
        "actionName": "Move to a new file"
      }
    }, {
      "title": "Extract to function in module scope",
      "kind": "refactor.extract.function",
      "isPreferred": false,
      "data": {
        "specifier": "file:///a/file.ts",
        "range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 14, "character": 0 }
        },
        "refactorName": "Extract Symbol",
        "actionName": "function_scope_0"
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_deadlock() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let large_file_text =
    fs::read_to_string(testdata_path().join("lsp").join("large_file.txt"))
      .unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "javascript",
      "version": 1,
      "text": large_file_text,
    }
  }));
  client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 444, "character": 11 },
            "end": { "line": 444, "character": 14 }
          },
          "text": "+++"
        }
      ]
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 445, "character": 4 },
            "end": { "line": 445, "character": 4 }
          },
          "text": "// "
        }
      ]
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 477, "character": 4 },
            "end": { "line": 477, "character": 9 }
          },
          "text": "error"
        }
      ]
    }),
  );
  // diagnostics only trigger after changes have elapsed in a separate thread,
  // so we need to delay the next messages a little bit to attempt to create a
  // potential for a deadlock with the codeAction
  std::thread::sleep(std::time::Duration::from_millis(50));
  client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 609, "character": 33, }
    }),
  );
  client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 441, "character": 33 },
        "end": { "line": 441, "character": 42 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 441, "character": 33 },
            "end": { "line": 441, "character": 42 }
          },
          "severity": 1,
          "code": 7031,
          "source": "deno-ts",
          "message": "Binding element 'debugFlag' implicitly has an 'any' type."
        }],
        "only": [ "quickfix" ]
      }
    }),
  );

  client.read_diagnostics();

  client.shutdown();
}

#[test]
fn lsp_completions() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "Deno."
    }
  }));

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (0, 5),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert!(list.items.len() > 90);

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "build",
      "kind": 6,
      "sortText": "1",
      "insertTextFormat": 1,
      "data": {
        "tsc": {
          "specifier": "file:///a/file.ts",
          "position": 5,
          "name": "build",
          "useCodeSnippet": false
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "build",
      "kind": 6,
      "detail": "const Deno.build: {\n    target: string;\n    arch: \"x86_64\" | \"aarch64\";\n    os: \"darwin\" | \"linux\" | \"android\" | \"windows\" | \"freebsd\" | \"netbsd\" | \"aix\" | \"solaris\" | \"illumos\";\n    vendor: string;\n    env?: string;\n}",
      "documentation": {
        "kind": "markdown",
        "value": "Information related to the build of the current Deno runtime.\n\nUsers are discouraged from code branching based on this information, as\nassumptions about what is available in what build environment might change\nover time. Developers should specifically sniff out the features they\nintend to use.\n\nThe intended use for the information is for logging and debugging purposes.\n\n*@category* - Runtime"
      },
      "sortText": "1",
      "insertTextFormat": 1
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_private_fields() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"class Foo { #myProperty = "value"; constructor() { this.# } }"#
    }
  }));
  let list = client.get_completion_list(
    "file:///a/file.ts",
    (0, 57),
    json!({ "triggerKind": 1 }),
  );
  assert_eq!(list.items.len(), 1);
  let item = &list.items[0];
  assert_eq!(item.label, "#myProperty");
  assert!(!list.is_incomplete);
  client.shutdown();
}

#[test]
fn lsp_completions_optional() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b?: string;\n}\n\nconst o: A = {};\n\nfunction c(s: string) {}\n\nc(o.)"
      }
    }),
  );
  let res = client.get_completion(
    "file:///a/file.ts",
    (8, 4),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert_eq!(
    json!(res),
    json!({
      "isIncomplete": false,
      "items": [
        {
          "label": "b?",
          "kind": 5,
          "sortText": "11",
          "filterText": "b",
          "insertText": "b",
          "commitCharacters": [".", ",", ";", "("],
          "data": {
            "tsc": {
              "specifier": "file:///a/file.ts",
              "position": 79,
              "name": "b",
              "useCodeSnippet": false
            }
          }
        }
      ]
    })
  );
  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "b?",
      "kind": 5,
      "sortText": "1",
      "filterText": "b",
      "insertText": "b",
      "data": {
        "tsc": {
          "specifier": "file:///a/file.ts",
          "position": 79,
          "name": "b",
          "useCodeSnippet": false
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "b?",
      "kind": 5,
      "detail": "(property) A.b?: string | undefined",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "1",
      "filterText": "b",
      "insertText": "b"
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_auto_import() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/🦕.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "/**\n *\n * @example\n * ```ts\n * const result = add(1, 2);\n * console.log(result); // 3\n * ```\n *\n * @param {number} a - The first number\n * @param {number} b - The second number\n */\nexport function add(a: number, b: number) {\n  return a + b;\n}",
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export {};\n\n",
    }
  }));
  let list = client.get_completion_list(
    "file:///a/file.ts",
    (2, 0),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list.items.iter().find(|item| item.label == "add");
  let Some(item) = item else {
    panic!("completions items missing 'add' symbol");
  };
  let mut item_value = serde_json::to_value(item).unwrap();
  item_value["data"]["tsc"]["data"]["exportMapKey"] =
    serde_json::Value::String("".to_string());

  let req = json!({
    "label": "add",
    "labelDetails": {
      "description": "./🦕.ts",
    },
    "kind": 3,
    "sortText": "￿16_0",
    "commitCharacters": [
      ".",
      ",",
      ";",
      "("
    ],
    "data": {
      "tsc": {
        "specifier": "file:///a/file.ts",
        "position": 12,
        "name": "add",
        "source": "./%F0%9F%A6%95.ts",
         "specifierRewrite": [
           "./%F0%9F%A6%95.ts",
           "./🦕.ts",
         ],
        "data": {
          "exportName": "add",
          "exportMapKey": "",
          "moduleSpecifier": "./%F0%9F%A6%95.ts",
          "fileName": "file:///a/%F0%9F%A6%95.ts"
        },
        "useCodeSnippet": false
      }
    }
  });
  assert_eq!(item_value, req);

  let res = client.write_request("completionItem/resolve", req);
  assert_eq!(
    res,
    json!({
      "label": "add",
      "labelDetails": {
        "description": "./🦕.ts",
      },
      "kind": 3,
      "detail": "function add(a: number, b: number): number",
      "documentation": {
        "kind": "markdown",
        "value": "\n\n*@example*  \n```ts\nconst result = add(1, 2);\nconsole.log(result); // 3\n```  \n\n*@param* - a - The first number  \n\n*@param* - b - The second number"
      },
      "sortText": "￿16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { add } from \"./🦕.ts\";\n\n"
        }
      ]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_npm_completions_auto_import_and_quick_fix_no_import_map() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import {getClient} from 'npm:@denotest/types-exports-subpaths@1/client';import chalk from 'npm:chalk@5.0';\n\n",
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["npm:@denotest/types-exports-subpaths@1/client", "npm:chalk@5.0"],
        "file:///a/file.ts",
      ],
    }),
  );

  // try auto-import with path
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/a.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "getClie",
    }
  }));
  let list = client.get_completion_list(
    "file:///a/a.ts",
    (0, 7),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "getClient")
    .unwrap();

  let res = client.write_request("completionItem/resolve", item);
  assert_eq!(
    res,
    json!({
      "label": "getClient",
      "labelDetails": {
        "description": "npm:@denotest/types-exports-subpaths@1/client",
      },
      "kind": 3,
      "detail": "function getClient(): 5",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "￿16_1",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { getClient } from \"npm:@denotest/types-exports-subpaths@1/client\";\n\n"
        }
      ]
    })
  );

  // try quick fix with path
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/b.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "getClient",
    }
  }));
  let diagnostics = diagnostics
    .messages_with_file_and_source("file:///a/b.ts", "deno-ts")
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": "file:///a/b.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 9 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"npm:@denotest/types-exports-subpaths@1/client\"",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 9 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'getClient'.",
        }
      ],
      "edit": {
        "documentChanges": [{
            "textDocument": {
              "uri": "file:///a/b.ts",
              "version": 1,
            },
            "edits": [{
              "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
              },
              "newText": "import { getClient } from \"npm:@denotest/types-exports-subpaths@1/client\";\n\n"
            }]
        }]
      }
    }])
  );

  // try auto-import without path
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/c.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "chal",
    }
  }));

  let list = client.get_completion_list(
    "file:///a/c.ts",
    (0, 4),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "chalk")
    .unwrap();

  let mut res = client.write_request("completionItem/resolve", item);
  let obj = res.as_object_mut().unwrap();
  obj.remove("detail"); // not worth testing these
  obj.remove("documentation");
  assert_eq!(
    res,
    json!({
      "label": "chalk",
      "labelDetails": {
        "description": "npm:chalk@5.0",
      },
      "kind": 6,
      "sortText": "￿16_1",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import chalk from \"npm:chalk@5.0\";\n\n"
        }
      ]
    })
  );

  // try quick fix without path
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/d.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "chalk",
    }
  }));
  let diagnostics = diagnostics
    .messages_with_file_and_source("file:///a/d.ts", "deno-ts")
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": "file:///a/d.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 5 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"npm:chalk@5.0\"",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 5 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'chalk'.",
        }
      ],
      "edit": {
        "documentChanges": [{
            "textDocument": {
              "uri": "file:///a/d.ts",
              "version": 1,
            },
            "edits": [{
              "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
              },
              "newText": "import chalk from \"npm:chalk@5.0\";\n\n"
            }]
        }]
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_infer_return_type() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  let types_file = source_file(
    temp_dir.path().join("types.d.ts"),
    r#"
      export interface SomeInterface {
        someField: number;
      }
      declare global {
        export function someFunction(): SomeInterface;
      }
    "#,
  );
  let file = source_file(
    temp_dir.path().join("file.ts"),
    r#"
      function foo() {
        return someFunction();
      }
      foo();
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": { "uri": file.uri() },
      "range": {
        "start": { "line": 1, "character": 15 },
        "end": { "line": 1, "character": 18 },
      },
      "context": {
        "diagnostics": [],
        "only": ["refactor.rewrite.function.returnType"],
      }
    }),
  );
  assert_eq!(
    &res,
    &json!([
      {
        "title": "Infer function return type",
        "kind": "refactor.rewrite.function.returnType",
        "isPreferred": false,
        "data": {
          "specifier": file.uri(),
          "range": {
            "start": { "line": 1, "character": 15 },
            "end": { "line": 1, "character": 18 },
          },
          "refactorName": "Infer function return type",
          "actionName": "Infer function return type",
        },
      }
    ]),
  );
  let code_action = res.as_array().unwrap().first().unwrap();
  let res = client.write_request("codeAction/resolve", code_action);
  assert_eq!(
    &res,
    &json!({
      "title": "Infer function return type",
      "kind": "refactor.rewrite.function.returnType",
      "isPreferred": false,
      "data": {
        "specifier": file.uri(),
        "range": {
          "start": { "line": 1, "character": 15 },
          "end": { "line": 1, "character": 18 },
        },
        "refactorName": "Infer function return type",
        "actionName": "Infer function return type",
      },
      "edit": {
        "documentChanges": [
          {
            "textDocument": { "uri": file.uri(), "version": null },
            "edits": [
              {
                "range": {
                  "start": { "line": 1, "character": 20 },
                  "end": { "line": 1, "character": 20 },
                },
                "newText": format!(": import(\"{}\").SomeInterface", types_file.uri()),
              },
            ],
          },
        ],
      },
    }),
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/23895.
#[test]
fn lsp_npm_types_nested_js_dts() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let file = source_file(
    temp_dir.path().join("file.ts"),
    r#"
      import { someString } from "npm:@denotest/types-nested-js-dts";
      const someNumber: number = someString;
      console.log(someNumber);
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], file.uri()],
    }),
  );
  let diagnostics = client.did_open_file(&file);
  assert_eq!(
    json!(diagnostics.all()),
    json!([
      {
        "range": {
          "start": {
            "line": 2,
            "character": 12,
          },
          "end": {
            "line": 2,
            "character": 22,
          },
        },
        "severity": 1,
        "code": 2322,
        "source": "deno-ts",
        "message": "Type 'string' is not assignable to type 'number'.",
      },
    ])
  );
}

#[test]
fn lsp_completions_using_decl() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": r#"function makeResource() {
  return {
    [Symbol.dispose]() {
    },
  };
}

using resource = makeResource();

res"#
    }
  }));

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (9, 3),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(list.items.iter().any(|i| i.label == "resource"));
  assert!(!list.is_incomplete);

  client.shutdown();
}

#[test]
fn lsp_npm_always_caches() {
  // npm specifiers should always be cached even when not specified
  // because they affect the resolution of each other
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir_path = context.temp_dir().path();

  // this file should be auto-discovered by the lsp
  let not_opened_file = temp_dir_path.join("not_opened.ts");
  not_opened_file.write("import chalk from 'npm:chalk@5.0';\n");

  // create the lsp and cache a different npm specifier
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let opened_file_uri = temp_dir_path.join("file.ts").uri_file();
  client.did_open(
    json!({
      "textDocument": {
        "uri": opened_file_uri,
        "languageId": "typescript",
        "version": 1,
        "text": "import {getClient} from 'npm:@denotest/types-exports-subpaths@1/client';\n",
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["npm:@denotest/types-exports-subpaths@1/client"],
        opened_file_uri,
      ],
    }),
  );

  // now open a new file and chalk should be working
  let new_file_uri = temp_dir_path.join("new_file.ts").uri_file();
  client.did_open(json!({
    "textDocument": {
      "uri": new_file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import chalk from 'npm:chalk@5.0';\nchalk.",
    }
  }));

  let list = client.get_completion_list(
    new_file_uri,
    (1, 6),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert!(list.items.iter().any(|i| i.label == "green"));
  assert!(list.items.iter().any(|i| i.label == "red"));

  client.shutdown();
}

#[test]
fn lsp_semantic_tokens_for_disabled_module() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_with_config(
    |builder| {
      builder.set_deno_enable(false);
    },
    json!({ "deno": {
      "enable": false
    } }),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "const someConst = 1; someConst"
    }
  }));
  let res = client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "data": [0, 6, 9, 7, 9, 0, 15, 9, 7, 8],
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_auto_import_and_quick_fix_with_import_map() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let import_map = r#"{
    "imports": {
      "print_hello": "http://localhost:4545/subdir/print_hello.ts",
      "chalk": "npm:chalk@~5",
      "nested/": "npm:/@denotest/types-exports-subpaths@1/nested/",
      "types-exports-subpaths/": "npm:/@denotest/types-exports-subpaths@1/"
    }
  }"#;
  temp_dir.write("import_map.json", import_map);

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("import_map.json");
  });
  client.did_open(
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": concat!(
          "import {getClient} from 'npm:@denotest/types-exports-subpaths@1/client';\n",
          "import _test1 from 'npm:chalk@^5.0';\n",
          "import chalk from 'npm:chalk@~5';\n",
          "import chalk from 'npm:chalk@~5';\n",
          "import {entryB} from 'npm:@denotest/types-exports-subpaths@1/nested/entry-b';\n",
          "import {printHello} from 'print_hello';\n",
          "\n",
        ),
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        [
          "npm:@denotest/types-exports-subpaths@1/client",
          "npm:@denotest/types-exports-subpaths@1/nested/entry-b",
          "npm:chalk@^5.0",
          "npm:chalk@~5",
          "http://localhost:4545/subdir/print_hello.ts",
        ],
        temp_dir.uri().join("file.ts").unwrap(),
      ],
    }),
  );

  // try auto-import with path
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("a.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "getClie",
    }
  }));
  let list = client.get_completion_list(
    temp_dir.uri().join("a.ts").unwrap(),
    (0, 7),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "getClient")
    .unwrap();

  let res = client.write_request("completionItem/resolve", item);
  assert_eq!(
    res,
    json!({
      "label": "getClient",
      "labelDetails": {
        "description": "types-exports-subpaths/client",
      },
      "kind": 3,
      "detail": "function getClient(): 5",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "￿16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { getClient } from \"types-exports-subpaths/client\";\n\n"
        }
      ]
    })
  );

  // try quick fix with path
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("b.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "getClient",
    }
  }));
  let diagnostics = diagnostics
    .messages_with_file_and_source(
      temp_dir.uri().join("b.ts").unwrap().as_str(),
      "deno-ts",
    )
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("b.ts").unwrap()
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 9 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"types-exports-subpaths/client\"",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 9 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'getClient'.",
        }
      ],
      "edit": {
        "documentChanges": [{
            "textDocument": {
              "uri": temp_dir.uri().join("b.ts").unwrap(),
              "version": 1,
            },
            "edits": [{
              "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
              },
              "newText": "import { getClient } from \"types-exports-subpaths/client\";\n\n"
            }]
        }]
      }
    }])
  );

  // try auto-import without path
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("c.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "chal",
    }
  }));

  let list = client.get_completion_list(
    temp_dir.uri().join("c.ts").unwrap(),
    (0, 4),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "chalk")
    .unwrap();

  let mut res = client.write_request("completionItem/resolve", item);
  let obj = res.as_object_mut().unwrap();
  obj.remove("detail"); // not worth testing these
  obj.remove("documentation");
  assert_eq!(
    res,
    json!({
      "label": "chalk",
      "labelDetails": {
        "description": "chalk",
      },
      "kind": 6,
      "sortText": "￿16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import chalk from \"chalk\";\n\n"
        }
      ]
    })
  );

  // try quick fix without path
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("d.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "chalk",
    }
  }));
  let diagnostics = diagnostics
    .messages_with_file_and_source(
      temp_dir.uri().join("d.ts").unwrap().as_str(),
      "deno-ts",
    )
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("d.ts").unwrap()
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 5 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"chalk\"",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 5 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'chalk'.",
        }
      ],
      "edit": {
        "documentChanges": [{
            "textDocument": {
              "uri": temp_dir.uri().join("d.ts").unwrap(),
              "version": 1,
            },
            "edits": [{
              "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
              },
              "newText": "import chalk from \"chalk\";\n\n"
            }]
        }]
      }
    }])
  );

  // try auto-import with http import map
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("e.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "printH",
    }
  }));

  let list = client.get_completion_list(
    temp_dir.uri().join("e.ts").unwrap(),
    (0, 6),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "printHello")
    .unwrap();

  let mut res = client.write_request("completionItem/resolve", item);
  let obj = res.as_object_mut().unwrap();
  obj.remove("detail"); // not worth testing these
  obj.remove("documentation");
  assert_eq!(
    res,
    json!({
      "label": "printHello",
      "labelDetails": {
        "description": "print_hello",
      },
      "kind": 3,
      "sortText": "￿16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { printHello } from \"print_hello\";\n\n"
        }
      ]
    })
  );

  // try quick fix with http import
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("f.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "printHello",
    }
  }));
  let diagnostics = diagnostics
    .messages_with_file_and_source(
      temp_dir.uri().join("f.ts").unwrap().as_str(),
      "deno-ts",
    )
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("f.ts").unwrap()
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 10 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add import from \"print_hello\"",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 10 }
          },
          "severity": 1,
          "code": 2304,
          "source": "deno-ts",
          "message": "Cannot find name 'printHello'.",
        }
      ],
      "edit": {
        "documentChanges": [{
            "textDocument": {
              "uri": temp_dir.uri().join("f.ts").unwrap(),
              "version": 1,
            },
            "edits": [{
              "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
              },
              "newText": "import { printHello } from \"print_hello\";\n\n"
            }]
        }]
      }
    }])
  );

  // try auto-import with npm package with sub-path on value side of import map
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("nested_path.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "entry",
    }
  }));
  let list = client.get_completion_list(
    temp_dir.uri().join("nested_path.ts").unwrap(),
    (0, 5),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  let item = list
    .items
    .iter()
    .find(|item| item.label == "entryB")
    .unwrap();

  let res = client.write_request("completionItem/resolve", item);
  assert_eq!(
    res,
    json!({
      "label": "entryB",
      "labelDetails": {
        "description": "nested/entry-b",
      },
      "kind": 3,
      "detail": "function entryB(): \"b\"",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "￿16_0",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { entryB } from \"nested/entry-b\";\n\n"
        }
      ]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_snippet() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/a.tsx",
        "languageId": "typescriptreact",
        "version": 1,
        "text": "function A({ type }: { type: string }) {\n  return type;\n}\n\nfunction B() {\n  return <A t\n}",
      }
    }),
  );
  let list = client.get_completion_list(
    "file:///a/a.tsx",
    (5, 13),
    json!({ "triggerKind": 1 }),
  );
  assert_eq!(
    json!(list),
    json!({
      "isIncomplete": false,
      "items": [
        {
          "label": "type",
          "kind": 5,
          "sortText": "11",
          "filterText": "type=\"$1\"",
          "insertText": "type=\"$1\"",
          "insertTextFormat": 2,
          "commitCharacters": [
            ".",
            ",",
            ";",
            "("
          ],
          "data": {
            "tsc": {
              "specifier": "file:///a/a.tsx",
              "position": 87,
              "name": "type",
              "useCodeSnippet": false
            }
          }
        }
      ]
    })
  );

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "type",
      "kind": 5,
      "sortText": "11",
      "filterText": "type=\"$1\"",
      "insertText": "type=\"$1\"",
      "insertTextFormat": 2,
      "commitCharacters": [
        ".",
        ",",
        ";",
        "("
      ],
      "data": {
        "tsc": {
          "specifier": "file:///a/a.tsx",
          "position": 87,
          "name": "type",
          "useCodeSnippet": false
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "type",
      "kind": 5,
      "detail": "(property) type: string",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "11",
      "filterText": "type=\"$1\"",
      "insertText": "type=\"$1\"",
      "insertTextFormat": 2
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_no_snippet() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.with_capabilities(|c| {
      let doc = c.text_document.as_mut().unwrap();
      doc.completion = None;
    });
  });
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/a.tsx",
        "languageId": "typescriptreact",
        "version": 1,
        "text": "function A({ type }: { type: string }) {\n  return type;\n}\n\nfunction B() {\n  return <A t\n}",
      }
    }),
  );
  let list = client.get_completion_list(
    "file:///a/a.tsx",
    (5, 13),
    json!({ "triggerKind": 1 }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(
    json!(list),
    json!({
      "isIncomplete": false,
      "items": [
        {
          "label": "type",
          "kind": 5,
          "sortText": "11",
          "commitCharacters": [
            ".",
            ",",
            ";",
            "("
          ],
          "data": {
            "tsc": {
              "specifier": "file:///a/a.tsx",
              "position": 87,
              "name": "type",
              "useCodeSnippet": false
            }
          }
        }
      ]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_npm() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import cjsDefault from 'npm:@denotest/cjs-default-export';import chalk from 'npm:chalk';\n\n",
      }
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["npm:@denotest/cjs-default-export", "npm:chalk"],
        "file:///a/file.ts",
      ],
    }),
  );

  // check importing a cjs default import
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 2, "character": 0 },
            "end": { "line": 2, "character": 0 }
          },
          "text": "cjsDefault."
        }
      ]
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(
    diagnostics
      .all()
      .iter()
      .map(|d| d.message.as_str())
      .collect::<Vec<_>>(),
    vec![
      "'chalk' is declared but its value is never read.",
      "Identifier expected."
    ]
  );

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (2, 11),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(
    list
      .items
      .iter()
      .map(|i| i.label.as_str())
      .collect::<Vec<_>>(),
    vec!["default", "MyClass", "named"]
  );

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "MyClass",
      "kind": 6,
      "sortText": "1",
      "insertTextFormat": 1,
      "data": {
        "tsc": {
          "specifier": "file:///a/file.ts",
          "position": 69,
          "name": "MyClass",
          "useCodeSnippet": false
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "MyClass",
      "kind": 6,
      "sortText": "1",
      "insertTextFormat": 1,
      "data": {
        "tsc": {
          "specifier": "file:///a/file.ts",
          "position": 69,
          "name": "MyClass",
          "useCodeSnippet": false
        }
      }
    })
  );

  // now check chalk, which is esm
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 3
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 2, "character": 0 },
            "end": { "line": 2, "character": 11 }
          },
          "text": "chalk."
        }
      ]
    }),
  );
  client.read_diagnostics();

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (2, 6),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert!(list.items.iter().any(|i| i.label == "green"));
  assert!(list.items.iter().any(|i| i.label == "red"));

  client.shutdown();
}

#[test]
fn lsp_npm_specifier_unopened_file() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  // create other.ts, which re-exports an npm specifier
  temp_dir.write(
    "other.ts",
    "export { default as chalk } from 'npm:chalk@5';",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  // cache the other.ts file to the DENO_DIR
  let deno = deno_cmd_with_deno_dir(client.deno_dir())
    .current_dir(temp_dir.path())
    .arg("cache")
    .arg("--quiet")
    .arg("other.ts")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());
  assert_eq!(output.status.code(), Some(0));

  let stdout = String::from_utf8(output.stdout).unwrap();
  assert!(stdout.is_empty());
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(stderr.is_empty());

  // open main.ts, which imports other.ts (unopened)
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("main.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import { chalk } from './other.ts';\n\n",
    }
  }));

  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("main.ts").unwrap(),
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 2, "character": 0 },
            "end": { "line": 2, "character": 0 }
          },
          "text": "chalk."
        }
      ]
    }),
  );
  client.read_diagnostics();

  // now ensure completions work
  let list = client.get_completion_list(
    temp_dir.uri().join("main.ts").unwrap(),
    (2, 6),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 63);
  assert!(list.items.iter().any(|i| i.label == "ansi256"));
  client.shutdown();
}

#[test]
fn lsp_completions_node_specifier() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import fs from 'node:non-existent';\n\n",
    }
  }));

  let non_existent_diagnostics = diagnostics
    .messages_with_file_and_source("file:///a/file.ts", "deno")
    .diagnostics
    .into_iter()
    .filter(|d| {
      d.code == Some(lsp::NumberOrString::String("resolver-error".to_string()))
    })
    .collect::<Vec<_>>();
  assert_eq!(
    json!(non_existent_diagnostics),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 15 },
          "end": { "line": 0, "character": 34 },
        },
        "severity": 1,
        "code": "resolver-error",
        "source": "deno",
        "message": "Unknown Node built-in module: non-existent"
      }
    ])
  );

  // update to have fs import
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 0, "character": 16 },
            "end": { "line": 0, "character": 33 },
          },
          "text": "fs"
        }
      ]
    }),
  );
  let diagnostics = client.read_diagnostics();
  let diagnostics = diagnostics
    .messages_with_file_and_source("file:///a/file.ts", "deno")
    .diagnostics
    .into_iter()
    .filter(|d| {
      d.code
        == Some(lsp::NumberOrString::String(
          "import-node-prefix-missing".to_string(),
        ))
    })
    .collect::<Vec<_>>();

  // get the quick fixes
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 16 },
        "end": { "line": 0, "character": 18 },
      },
      "context": {
        "diagnostics": json!(diagnostics),
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Update specifier to node:fs",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 15 },
            "end": { "line": 0, "character": 19 }
          },
          "severity": 1,
          "code": "import-node-prefix-missing",
          "source": "deno",
          "message": "Relative import path \"fs\" not prefixed with / or ./ or ../\nIf you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:fs\").",
          "data": {
            "specifier": "fs"
          },
        }
      ],
      "edit": {
        "changes": {
          "file:///a/file.ts": [
            {
              "range": {
                "start": { "line": 0, "character": 15 },
                "end": { "line": 0, "character": 19 }
              },
              "newText": "\"node:fs\""
            }
          ]
        }
      }
    }])
  );

  // update to have node:fs import
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 3,
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 0, "character": 15 },
            "end": { "line": 0, "character": 19 },
          },
          "text": "\"node:fs\"",
        }
      ]
    }),
  );

  let diagnostics = client.read_diagnostics();
  let cache_diagnostics = diagnostics
    .messages_with_file_and_source("file:///a/file.ts", "deno")
    .diagnostics
    .into_iter()
    .filter(|d| {
      d.code == Some(lsp::NumberOrString::String("no-cache-npm".to_string()))
    })
    .collect::<Vec<_>>();

  assert_eq!(
    json!(cache_diagnostics),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 15 },
          "end": { "line": 0, "character": 24 }
        },
        "data": {
          "specifier": "npm:@types/node",
        },
        "severity": 1,
        "code": "no-cache-npm",
        "source": "deno",
        "message": "Uncached or missing npm package: @types/node"
      }
    ])
  );

  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [["npm:@types/node"], "file:///a/file.ts"],
    }),
  );

  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "version": 4
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 2, "character": 0 },
            "end": { "line": 2, "character": 0 }
          },
          "text": "fs."
        }
      ]
    }),
  );
  client.read_diagnostics();

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (2, 3),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert!(list.items.iter().any(|i| i.label == "writeFile"));
  assert!(list.items.iter().any(|i| i.label == "writeFileSync"));

  client.shutdown();
}

#[test]
fn lsp_completions_node_specifier_node_modules_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    temp_dir.path().join("deno.json"),
    json!({
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import fs from \"node:fs\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
        "version": 2,
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 1, "character": 0 },
            "end": { "line": 1, "character": 0 },
          },
          "text": "fs.",
        },
      ],
    }),
  );
  let list = client.get_completion_list(
    temp_dir.uri().join("file.ts").unwrap(),
    (1, 3),
    json!({
      "triggerKind": 2,
      "triggerCharacter": ".",
    }),
  );
  assert!(!list.is_incomplete);
  assert!(list.items.iter().any(|i| i.label == "writeFile"));
  assert!(list.items.iter().any(|i| i.label == "writeFileSync"));
  client.shutdown();
}

#[test]
fn lsp_completions_registry() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.add_test_server_suggestions();
  });
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"http://localhost:4545/x/a@\""
    }
  }));
  let list = client.get_completion_list(
    "file:///a/file.ts",
    (0, 46),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "@"
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 3);

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "v2.0.0",
      "kind": 19,
      "detail": "(version)",
      "sortText": "0000000003",
      "filterText": "http://localhost:4545/x/a@v2.0.0",
      "textEdit": {
        "range": {
          "start": { "line": 0, "character": 20 },
          "end": { "line": 0, "character": 46 }
        },
        "newText": "http://localhost:4545/x/a@v2.0.0"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "v2.0.0",
      "kind": 19,
      "detail": "(version)",
      "sortText": "0000000003",
      "filterText": "http://localhost:4545/x/a@v2.0.0",
      "textEdit": {
        "range": {
          "start": { "line": 0, "character": 20 },
          "end": { "line": 0, "character": 46 }
        },
        "newText": "http://localhost:4545/x/a@v2.0.0"
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_registry_empty() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.add_test_server_suggestions();
  });
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"\""
    }
  }));
  let res = client.get_completion(
    "file:///a/file.ts",
    (0, 20),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "\""
    }),
  );
  assert_eq!(
    json!(res),
    json!({
      "isIncomplete": false,
      "items": [{
        "label": ".",
        "kind": 19,
        "detail": "(local)",
        "sortText": "1",
        "insertText": ".",
        "commitCharacters": ["\"", "'"]
      }, {
        "label": "..",
        "kind": 19,
        "detail": "(local)",
        "sortText": "1",
        "insertText": "..",
        "commitCharacters": ["\"", "'" ]
      }, {
        "label": "http://localhost:4545",
        "kind": 19,
        "detail": "(registry)",
        "sortText": "2",
        "textEdit": {
          "range": {
            "start": { "line": 0, "character": 20 },
            "end": { "line": 0, "character": 20 }
          },
          "newText": "http://localhost:4545"
        },
        "commitCharacters": ["\"", "'"]
      }]
    })
  );
  client.shutdown();
}

#[test]
fn lsp_auto_discover_registry() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"http://localhost:4545/x/a@\""
    }
  }));
  client.get_completion(
    "file:///a/file.ts",
    (0, 46),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "@"
    }),
  );
  let (method, res) = client.read_notification();
  assert_eq!(method, "deno/registryState");
  assert_eq!(
    res,
    Some(json!({
      "origin": "http://localhost:4545",
      "suggestions": true,
    }))
  );
  client.shutdown();
}

#[test]
fn lsp_cache_location() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_cache(".cache").add_test_server_suggestions();
  });

  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    }
  }));
  let diagnostics =
    client.did_open(json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/xTypeScriptTypes.js\";\n// @deno-types=\"http://127.0.0.1:4545/type_definitions/foo.d.ts\"\nimport * as b from \"http://127.0.0.1:4545/type_definitions/foo.js\";\nimport * as c from \"http://127.0.0.1:4545/subdir/type_reference.js\";\nimport * as d from \"http://127.0.0.1:4545/subdir/mod1.ts\";\nimport * as e from \"data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=\";\nimport * as f from \"./file_01.ts\";\nimport * as g from \"http://localhost:4545/x/a/mod.ts\";\n\nconsole.log(a, b, c, d, e, f, g);\n"
      }
    }));
  assert_eq!(diagnostics.all().len(), 6);
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], "file:///a/file.ts"],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 0, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n\n**Types**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.d.ts\n"
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end": { "line": 0, "character": 62 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 7, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/x/a/mod.ts\n\n\n---\n\n**a**\n\nmod.ts"
      },
      "range": {
        "start": { "line": 7, "character": 19 },
        "end": { "line": 7, "character": 53 }
      }
    })
  );
  let cache_path = temp_dir.path().join(".cache");
  assert!(cache_path.is_dir());
  assert!(!cache_path.join("gen").is_dir()); // not created because no emitting has occurred
  client.shutdown();
}

/// Sets the TLS root certificate on startup, which allows the LSP to connect to
/// the custom signed test server and be able to retrieve the registry config
/// and cache files.
#[test]
fn lsp_tls_cert() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder
      .set_suggest_imports_hosts(vec![
        ("http://localhost:4545/".to_string(), true),
        ("https://localhost:5545/".to_string(), true),
      ])
      .set_tls_certificate("");
  });

  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    }
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"https://localhost:5545/xTypeScriptTypes.js\";\n// @deno-types=\"https://localhost:5545/type_definitions/foo.d.ts\"\nimport * as b from \"https://localhost:5545/type_definitions/foo.js\";\nimport * as c from \"https://localhost:5545/subdir/type_reference.js\";\nimport * as d from \"https://localhost:5545/subdir/mod1.ts\";\nimport * as e from \"data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=\";\nimport * as f from \"./file_01.ts\";\nimport * as g from \"http://localhost:4545/x/a/mod.ts\";\n\nconsole.log(a, b, c, d, e, f, g);\n"
    }
  }));
  let diagnostics = diagnostics.all();
  assert_eq!(diagnostics.len(), 6);
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], "file:///a/file.ts"],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 0, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: https&#8203;://localhost:5545/xTypeScriptTypes.js\n"
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end": { "line": 0, "character": 63 }
      }
    })
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
      },
      "position": { "line": 7, "character": 28 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/x/a/mod.ts\n\n\n---\n\n**a**\n\nmod.ts"
      },
      "range": {
        "start": { "line": 7, "character": 19 },
        "end": { "line": 7, "character": 53 }
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_npmrc() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .add_npm_env_vars()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    temp_dir.path().join("deno.json"),
    json!({
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  temp_dir.write(
    temp_dir.path().join("package.json"),
    json!({
      "name": "npmrc_test",
      "version": "0.0.1",
      "dependencies": {
        "@denotest/basic": "1.0.0",
      },
    })
    .to_string(),
  );
  temp_dir.write(
    temp_dir.path().join(".npmrc"),
    "\
@denotest:registry=http://localhost:4261/
//localhost:4261/:_authToken=private-reg-token
",
  );
  let file = source_file(
    temp_dir.path().join("main.ts"),
    r#"
      import { getValue, setValue } from "@denotest/basic";
      setValue(42);
      const n: string = getValue();
      console.log(n);
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], file.uri()],
    }),
  );
  let diagnostics = client.did_open_file(&file);
  assert_eq!(
    json!(diagnostics.all()),
    json!([
      {
        "range": {
          "start": {
            "line": 3,
            "character": 12,
          },
          "end": {
            "line": 3,
            "character": 13,
          },
        },
        "severity": 1,
        "code": 2322,
        "source": "deno-ts",
        "message": "Type 'number' is not assignable to type 'string'.",
      },
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_diagnostics_warn_redirect() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/x_deno_warning.js\";\n\nconsole.log(a)\n",
      },
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["http://127.0.0.1:4545/x_deno_warning.js"],
        "file:///a/file.ts",
      ],
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    lsp::PublishDiagnosticsParams {
      uri: Url::parse("file:///a/file.ts").unwrap(),
      diagnostics: vec![
        lsp::Diagnostic {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 19
            },
            end: lsp::Position {
              line: 0,
              character: 60
            }
          },
          severity: Some(lsp::DiagnosticSeverity::WARNING),
          code: Some(lsp::NumberOrString::String("deno-warn".to_string())),
          source: Some("deno".to_string()),
          message: "foobar".to_string(),
          ..Default::default()
        },
        lsp::Diagnostic {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 19
            },
            end: lsp::Position {
              line: 0,
              character: 60
            }
          },
          severity: Some(lsp::DiagnosticSeverity::INFORMATION),
          code: Some(lsp::NumberOrString::String("redirect".to_string())),
          source: Some("deno".to_string()),
          message: "The import of \"http://127.0.0.1:4545/x_deno_warning.js\" was redirected to \"http://127.0.0.1:4545/lsp/x_deno_warning_redirect.js\".".to_string(),
          data: Some(json!({"specifier": "http://127.0.0.1:4545/x_deno_warning.js", "redirect": "http://127.0.0.1:4545/lsp/x_deno_warning_redirect.js"})),
          ..Default::default()
        }
      ],
      version: Some(1),
    }
  );
  client.shutdown();
}

#[test]
fn lsp_redirect_quick_fix() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/x_deno_warning.js\";\n\nconsole.log(a)\n",
      },
    }),
  );
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["http://127.0.0.1:4545/x_deno_warning.js"],
        "file:///a/file.ts",
      ],
    }),
  );
  let diagnostics = client
    .read_diagnostics()
    .messages_with_source("deno")
    .diagnostics;
  let res = client.write_request(
    "textDocument/codeAction",
    json!(json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end": { "line": 0, "character": 60 }
      },
      "context": {
        "diagnostics": diagnostics,
        "only": ["quickfix"]
      }
    })),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Update specifier to its redirected specifier.",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 19 },
            "end": { "line": 0, "character": 60 }
          },
          "severity": 3,
          "code": "redirect",
          "source": "deno",
          "message": "The import of \"http://127.0.0.1:4545/x_deno_warning.js\" was redirected to \"http://127.0.0.1:4545/lsp/x_deno_warning_redirect.js\".",
          "data": {
            "specifier": "http://127.0.0.1:4545/x_deno_warning.js",
            "redirect": "http://127.0.0.1:4545/lsp/x_deno_warning_redirect.js"
          }
        }
      ],
      "edit": {
        "changes": {
          "file:///a/file.ts": [
            {
              "range": {
                "start": { "line": 0, "character": 19 },
                "end": { "line": 0, "character": 60 }
              },
              "newText": "\"http://127.0.0.1:4545/lsp/x_deno_warning_redirect.js\""
            }
          ]
        }
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_lockfile_redirect_resolution() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", json!({}).to_string());
  temp_dir.write("deno.lock", json!({
    "version": "3",
    "redirects": {
      "http://localhost:4545/subdir/mod1.ts": "http://localhost:4545/subdir/mod2.ts",
    },
    "remote": {},
  }).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod1.ts\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("file.ts").unwrap()],
    }),
  );
  client.read_diagnostics();
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": { "uri": temp_dir.uri().join("file.ts").unwrap() },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": "deno:/http/localhost%3A4545/subdir/mod2.ts",
      "targetRange": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 1, "character": 0 },
      },
      "targetSelectionRange": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 1, "character": 0 },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_diagnostics_deprecated() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "/** @deprecated */\nexport const a = \"a\";\n\na;\n",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([{
      "uri": "file:///a/file.ts",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 3, "character": 0 },
            "end": { "line": 3, "character": 1 }
          },
          "severity": 4,
          "code": 6385,
          "source": "deno-ts",
          "message": "'a' is deprecated.",
          "relatedInformation": [
            {
              "location": {
                "uri": "file:///a/file.ts",
                "range": {
                  "start": {
                    "line": 0,
                    "character": 4,
                  },
                  "end": {
                    "line": 0,
                    "character": 16,
                  },
                },
              },
              "message": "The declaration was marked as deprecated here.",
            },
          ],
          "tags": [2]
        }
      ],
      "version": 1
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_diagnostics_deno_types() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client
    .did_open(json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "languageId": "typescript",
          "version": 1,
          "text": "/// <reference types=\"https://example.com/a/b.d.ts\" />\n/// <reference path=\"https://example.com/a/c.ts\"\n\n// @deno-types=https://example.com/a/d.d.ts\nimport * as d from \"https://example.com/a/d.js\";\n\n// @deno-types=\"https://example.com/a/e.d.ts\"\nimport * as e from \"https://example.com/a/e.js\";\n\nconsole.log(d, e);\n"
        }
      }),
    );

  client.write_request(
    "textDocument/documentSymbol",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(diagnostics.all().len(), 5);
  client.shutdown();
}

#[test]
fn lsp_root_with_global_reference_types() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let file = source_file(
    temp_dir.path().join("file.ts"),
    "import 'http://localhost:4545/subdir/foo_types.d.ts'; Foo.bar;",
  );
  let file2 = source_file(
    temp_dir.path().join("file2.ts"),
    r#"/// <reference types="http://localhost:4545/subdir/foo_types.d.ts" />"#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], file2.uri()],
    }),
  );
  let diagnostics = client.did_open_file(&file);
  assert_eq!(json!(diagnostics.all()), json!([]));
}

#[test]
fn lsp_diagnostics_refresh_dependents() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_00.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export * from \"./file_00.ts\";\n",
    },
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_02.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import { a, b } from \"./file_01.ts\";\n\nconsole.log(a, b);\n"
    }
  }));
  assert_eq!(
    json!(diagnostics
      .messages_with_file_and_source("file:///a/file_02.ts", "deno-ts")),
    json!({
      "uri": "file:///a/file_02.ts",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 12 },
            "end": { "line": 0, "character": 13 }
          },
          "severity": 1,
          "code": 2305,
          "source": "deno-ts",
          "message": "Module '\"./file_01.ts\"' has no exported member 'b'."
        }
      ],
      "version": 1
    })
  );

  // fix the code causing the diagnostic
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": "file:///a/file_00.ts",
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 1, "character": 0 },
            "end": { "line": 1, "character": 0 }
          },
          "text": "export const b = \"b\";\n"
        }
      ]
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(json!(diagnostics.all()), json!([])); // no diagnostics now

  client.shutdown();
  assert_eq!(client.queue_len(), 0);
}

// Regression test for https://github.com/denoland/deno/issues/10897.
#[test]
fn lsp_ts_diagnostics_refresh_on_lsp_version_reset() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("file.ts", r#"Deno.readTextFileSync(1);"#);
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("file.ts"),
    },
  }));
  assert_eq!(diagnostics.all().len(), 1);
  client.write_notification(
    "textDocument/didClose",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
    }),
  );
  temp_dir.remove_file("file.ts");
  // VSCode opens with `version: 1` again because the file was deleted. Ensure
  // diagnostics are still refreshed.
  client.did_open_raw(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "",
    },
  }));
  temp_dir.write("file.ts", r#""#);
  client.did_save(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
    },
  }));
  let diagnostics = client.read_diagnostics();
  assert_eq!(diagnostics.all(), vec![]);
  client.shutdown();
}

#[test]
fn lsp_diagnostics_none_for_resolving_types() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  context
    .temp_dir()
    .write("deno.json", r#"{ "unstable": ["byonm"] }"#);
  context.temp_dir().write(
    "package.json",
    r#"{ "dependencies": { "@denotest/monaco-editor": "*" } }"#,
  );
  context.run_npm("install");

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  // The types for this package will succeed, but the code will fail
  // because the package is only made for bundling and not meant to
  // run in Deno or Node.
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": context.temp_dir().path().join("file.ts").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": concat!(
        "import * as a from \"@denotest/monaco-editor\";\n",
        "console.log(new a.Editor())\n",
      )
    },
  }));
  let diagnostics = diagnostics.all();
  assert!(diagnostics.is_empty(), "{:?}", diagnostics);
  client.shutdown();
}

#[test]
fn lsp_jupyter_diagnostics() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "deno-notebook-cell:/a/file.ts#abc",
      "languageId": "typescript",
      "version": 1,
      "text": "Deno.readTextFileSync(1234);",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([
      {
        "uri": "deno-notebook-cell:/a/file.ts#abc",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 22,
              },
              "end": {
                "line": 0,
                "character": 26,
              },
            },
            "severity": 1,
            "code": 2345,
            "source": "deno-ts",
            "message": "Argument of type 'number' is not assignable to parameter of type 'string | URL'.",
          },
        ],
        "version": 1,
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_untitled_file_diagnostics() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "untitled:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "Deno.readTextFileSync(1234);",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([
      {
        "uri": "untitled:///a/file.ts",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 22,
              },
              "end": {
                "line": 0,
                "character": 26,
              },
            },
            "severity": 1,
            "code": 2345,
            "source": "deno-ts",
            "message": "Argument of type 'number' is not assignable to parameter of type 'string | URL'.",
          },
        ],
        "version": 1,
      },
    ])
  );
  client.shutdown();
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAverage {
  pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceAverages {
  averages: Vec<PerformanceAverage>,
}

#[test]
fn lsp_performance() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Deno.args);\n"
    }
  }));
  client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "position": { "line": 0, "character": 19 }
    }),
  );
  let res = client.write_request_with_res_as::<PerformanceAverages>(
    "deno/performance",
    json!(null),
  );
  let mut averages = res
    .averages
    .iter()
    .map(|a| a.name.as_str())
    .collect::<Vec<_>>();
  averages.sort();
  assert_eq!(
    averages,
    vec![
      "lsp.did_open",
      "lsp.hover",
      "lsp.initialize",
      "lsp.testing_update",
      "lsp.update_cache",
      "lsp.update_diagnostics_deps",
      "lsp.update_diagnostics_lint",
      "lsp.update_diagnostics_ts",
      "lsp.update_global_cache",
      "tsc.host.$getAssets",
      "tsc.host.$getDiagnostics",
      "tsc.host.$getSupportedCodeFixes",
      "tsc.host.getQuickInfoAtPosition",
      "tsc.op.op_is_node_file",
      "tsc.op.op_load",
      "tsc.op.op_script_names",
      "tsc.request.$getAssets",
      "tsc.request.$getSupportedCodeFixes",
      "tsc.request.getQuickInfoAtPosition",
    ]
  );
  client.shutdown();
}

#[test]
fn lsp_format_no_changes() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console;\n"
    }
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.assert_no_notification("window/showMessage");
  client.shutdown();
}

#[test]
fn lsp_format_error() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console test test\n"
    }
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_format_mbc() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "const bar = '👍🇺🇸😃'\nconsole.log('hello deno')\n"
    }
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 12 },
        "end": { "line": 0, "character": 13 }
      },
      "newText": "\""
    }, {
      "range": {
        "start": { "line": 0, "character": 21 },
        "end": { "line": 0, "character": 22 }
      },
      "newText": "\";"
    }, {
      "range": {
        "start": { "line": 1, "character": 12 },
        "end": { "line": 1, "character": 13 }
      },
      "newText": "\""
    }, {
      "range": {
        "start": { "line": 1, "character": 23 },
        "end": { "line": 1, "character": 25 }
      },
      "newText": "\");"
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_format_exclude_with_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "deno.fmt.jsonc",
    r#"{
    "fmt": {
      "files": {
        "exclude": ["ignored.ts"]
      },
      "options": {
        "useTabs": true,
        "lineWidth": 40,
        "indentWidth": 8,
        "singleQuote": true,
        "proseWrap": "always"
      }
    }
  }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.fmt.jsonc");
  });

  let file_uri = temp_dir.uri().join("ignored.ts").unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "function   myFunc(){}"
    }
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": file_uri
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_format_exclude_default_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "deno.fmt.jsonc",
    r#"{
    "fmt": {
      "files": {
        "exclude": ["ignored.ts"]
      },
      "options": {
        "useTabs": true,
        "lineWidth": 40,
        "indentWidth": 8,
        "singleQuote": true,
        "proseWrap": "always"
      }
    }
  }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.fmt.jsonc");
  });

  let file_uri = temp_dir.uri().join("ignored.ts").unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "function   myFunc(){}"
    }
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": file_uri
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );
  assert_eq!(res, json!(null));
  client.shutdown();
}

#[test]
fn lsp_format_json() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let json_file =
    source_file(temp_dir.path().join("file.json"), "{\"key\":\"value\"}");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": json_file.uri(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );

  assert_eq!(
    res,
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 1 },
          "end": { "line": 0, "character": 1 }
        },
        "newText": " "
      }, {
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 7 }
        },
        "newText": " "
      }, {
        "range": {
          "start": { "line": 0, "character": 14 },
          "end": { "line": 0, "character": 15 }
        },
        "newText": " }\n"
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_format_editor_options() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let file = source_file(
    temp_dir.path().join("file.ts"),
    "if (true) {\n  console.log();\n}\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": file.uri(),
      },
      "options": {
        "tabSize": 4,
        "insertSpaces": true,
      },
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "range": {
          "start": { "line": 1, "character": 0 },
          "end": { "line": 1, "character": 0 },
        },
        "newText": "  ",
      },
    ])
  );
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": file.uri(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": false,
      },
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "range": {
          "start": { "line": 1, "character": 0 },
          "end": { "line": 1, "character": 2 },
        },
        "newText": "\t",
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_json_no_diagnostics() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open_raw(json!({
    "textDocument": {
      "uri": "file:///a/file.json",
      "languageId": "json",
      "version": 1,
      "text": "{\"key\":\"value\"}"
    }
  }));

  let res = client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.json"
      }
    }),
  );
  assert_eq!(res, json!(null));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.json"
      },
      "position": { "line": 0, "character": 3 }
    }),
  );
  assert_eq!(res, json!(null));

  client.shutdown();
}

#[test]
fn lsp_json_import_with_query_string() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("data.json", r#"{"k": "v"}"#);
  temp_dir.write(
    "main.ts",
    r#"
      import data from "./data.json?1" with { type: "json" };
      console.log(data);
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open_raw(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("data.json").unwrap(),
      "languageId": "json",
      "version": 1,
      "text": temp_dir.read_to_string("data.json"),
    }
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("main.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.read_to_string("main.ts"),
    }
  }));
  assert_eq!(diagnostics.all(), vec![]);
  client.shutdown();
}

#[test]
fn lsp_format_markdown() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let markdown_file =
    source_file(temp_dir.path().join("file.md"), "#   Hello World");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": markdown_file.uri()
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true
      }
    }),
  );

  assert_eq!(
    res,
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 1 },
          "end": { "line": 0, "character": 3 }
        },
        "newText": ""
      }, {
        "range": {
          "start": { "line": 0, "character": 15 },
          "end": { "line": 0, "character": 15 }
        },
        "newText": "\n"
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_format_with_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.fmt.jsonc",
    r#"{
    "fmt": {
      "options": {
        "useTabs": true,
        "lineWidth": 40,
        "indentWidth": 8,
        "singleQuote": true,
        "proseWrap": "always"
      }
    }
  }
  "#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.fmt.jsonc");
  });

  let ts_file = temp_dir.path().join("file.ts");
  client
    .did_open(
      json!({
        "textDocument": {
          "uri": ts_file.uri_file(),
          "languageId": "typescript",
          "version": 1,
          "text": "export async function someVeryLongFunctionName() {\nconst response = fetch(\"http://localhost:4545/some/non/existent/path.json\");\nconsole.log(response.text());\nconsole.log(\"finished!\")\n}"
        }
      }),
    );

  // The options below should be ignored in favor of configuration from config file.
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": ts_file.uri_file()
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": false
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
        "range": {
          "start": { "line": 1, "character": 0 },
          "end": { "line": 1, "character": 0 }
        },
        "newText": "\t"
      }, {
        "range": {
          "start": { "line": 1, "character": 23 },
          "end": { "line": 1, "character": 24 }
        },
        "newText": "\n\t\t'"
      }, {
        "range": {
          "start": { "line": 1, "character": 73 },
          "end": { "line": 1, "character": 74 }
        },
        "newText": "',\n\t"
      }, {
        "range": {
          "start": { "line": 2, "character": 0 },
          "end": { "line": 2, "character": 0 }
        },
        "newText": "\t"
      }, {
        "range": {
          "start": { "line": 3, "character": 0 },
          "end": { "line": 3, "character": 0 }
        },
        "newText": "\t"
      }, {
        "range": {
          "start": { "line": 3, "character": 12 },
          "end": { "line": 3, "character": 13 }
        },
        "newText": "'"
      }, {
        "range": {
          "start": { "line": 3, "character": 22 },
          "end": { "line": 3, "character": 24 }
        },
        "newText": "');"
      }, {
        "range": {
          "start": { "line": 4, "character": 1 },
          "end": { "line": 4, "character": 1 }
        },
        "newText": "\n"
      }]
    )
  );
  client.shutdown();
}

#[test]
fn lsp_markdown_no_diagnostics() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open_raw(json!({
    "textDocument": {
      "uri": "file:///a/file.md",
      "languageId": "markdown",
      "version": 1,
      "text": "# Hello World"
    }
  }));

  let res = client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.md"
      }
    }),
  );
  assert_eq!(res, json!(null));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.md"
      },
      "position": { "line": 0, "character": 3 }
    }),
  );
  assert_eq!(res, json!(null));

  client.shutdown();
}

#[test]
fn lsp_configuration_did_change() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"http://localhost:4545/x/a@\""
    }
  }));
  client.change_configuration(json!({ "deno": {
    "enable": true,
    "codeLens": {
      "implementations": true,
      "references": true,
    },
    "importMap": null,
    "lint": true,
    "suggest": {
      "autoImports": true,
      "completeFunctionCalls": false,
      "names": true,
      "paths": true,
      "imports": {
        "hosts": {
          "http://localhost:4545/": true,
        },
      },
    },
    "unstable": false,
  } }));

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (0, 46),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "@"
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 3);

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "v2.0.0",
      "kind": 19,
      "detail": "(version)",
      "sortText": "0000000003",
      "filterText": "http://localhost:4545/x/a@v2.0.0",
      "textEdit": {
        "range": {
          "start": { "line": 0, "character": 20 },
          "end": { "line": 0, "character": 46 }
        },
        "newText": "http://localhost:4545/x/a@v2.0.0"
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "v2.0.0",
      "kind": 19,
      "detail": "(version)",
      "sortText": "0000000003",
      "filterText": "http://localhost:4545/x/a@v2.0.0",
      "textEdit": {
        "range": {
          "start": { "line": 0, "character": 20 },
          "end": { "line": 0, "character": 46 }
        },
        "newText": "http://localhost:4545/x/a@v2.0.0"
      }
    })
  );
  client.shutdown();
}

#[test]
fn lsp_completions_complete_function_calls() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "[]."
    }
  }));
  client.change_configuration(json!({
    "deno": {
      "enable": true,
    },
    "typescript": {
      "suggest": {
        "completeFunctionCalls": true,
      },
    },
  }));

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (0, 3),
    json!({
      "triggerKind": 2,
      "triggerCharacter": ".",
    }),
  );
  assert!(!list.is_incomplete);

  let res = client.write_request(
    "completionItem/resolve",
    json!({
      "label": "map",
      "kind": 2,
      "sortText": "1",
      "insertTextFormat": 1,
      "data": {
        "tsc": {
          "specifier": "file:///a/file.ts",
          "position": 3,
          "name": "map",
          "useCodeSnippet": true
        }
      }
    }),
  );
  assert_eq!(
    res,
    json!({
      "label": "map",
      "kind": 2,
      "detail": "(method) Array<never>.map<U>(callbackfn: (value: never, index: number, array: never[]) => U, thisArg?: any): U[]",
      "documentation": {
        "kind": "markdown",
        "value": "Calls a defined callback function on each element of an array, and returns an array that contains the results.\n\n*@param* - callbackfn A function that accepts up to three arguments. The map method calls the callbackfn function one time for each element in the array.  \n\n*@param* - thisArg An object to which the this keyword can refer in the callbackfn function. If thisArg is omitted, undefined is used as the this value."
      },
      "sortText": "1",
      "insertText": "map(${1:callbackfn})",
      "insertTextFormat": 2,
    })
  );
  client.shutdown();
}

#[test]
fn lsp_workspace_symbol() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export class A {\n  fieldA: string;\n  fieldB: string;\n}\n",
    }
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export class B {\n  fieldC: string;\n  fieldD: string;\n}\n",
    }
  }));
  let mut res = client.write_request(
    "workspace/symbol",
    json!({
      "query": "field"
    }),
  );

  // Replace `range` fields with `null` values. These are not important
  // for assertion and require to be updated if we change unstable APIs.
  for obj in res.as_array_mut().unwrap().iter_mut() {
    *obj
      .as_object_mut()
      .unwrap()
      .get_mut("location")
      .unwrap()
      .as_object_mut()
      .unwrap()
      .get_mut("range")
      .unwrap() = Value::Null;
  }

  assert_eq!(
    res,
    json!([
      {
        "name": "fieldA",
        "kind": 8,
        "location": {
          "uri": "file:///a/file.ts",
          "range": null,
        },
        "containerName": "A"
      },
      {
        "name": "fieldB",
        "kind": 8,
        "location": {
          "uri": "file:///a/file.ts",
          "range": null,
        },
        "containerName": "A"
      },
      {
        "name": "fieldC",
        "kind": 8,
        "location": {
          "uri": "file:///a/file_01.ts",
          "range": null,
        },
        "containerName": "B"
      },
      {
        "name": "fieldD",
        "kind": 8,
        "location": {
          "uri": "file:///a/file_01.ts",
          "range": null,
        },
        "containerName": "B"
      },
      {
        "name": "fields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "CalendarProtocol"
      },
      {
        "name": "fields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Calendar"
      },
      {
        "name": "ClassFieldDecoratorContext",
        "kind": 11,
        "location": {
          "uri": "deno:/asset/lib.decorators.d.ts",
          "range": null,
        },
        "containerName": ""
      },
      {
        "name": "dateFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "CalendarProtocol"
      },
      {
        "name": "dateFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Calendar"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "PlainDate"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "PlainDateTime"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "PlainMonthDay"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "PlainTime"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "PlainYearMonth"
      },
      {
        "name": "getISOFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "ZonedDateTime"
      },
      {
        "name": "mergeFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "CalendarProtocol"
      },
      {
        "name": "mergeFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Calendar"
      },
      {
        "name": "monthDayFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "CalendarProtocol"
      },
      {
        "name": "monthDayFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Calendar"
      },
      {
        "name": "PlainDateISOFields",
        "kind": 5,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Temporal"
      },
      {
        "name": "PlainDateTimeISOFields",
        "kind": 5,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Temporal"
      },
      {
        "name": "PlainTimeISOFields",
        "kind": 5,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Temporal"
      },
      {
        "name": "yearMonthFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "CalendarProtocol"
      },
      {
        "name": "yearMonthFromFields",
        "kind": 6,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Calendar"
      },
      {
        "name": "ZonedDateTimeISOFields",
        "kind": 5,
        "location": {
          "uri": "deno:/asset/lib.deno.unstable.d.ts",
          "range": null,
        },
        "containerName": "Temporal"
      }
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_ignore_lint() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "let message = 'Hello, Deno!';\nconsole.log(message);\n"
    }
  }));
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 1, "character": 5 },
        "end": { "line": 1, "character": 12 }
      },
      "context": {
        "diagnostics": [
          {
            "range": {
              "start": { "line": 1, "character": 5 },
              "end": { "line": 1, "character": 12 }
            },
            "severity": 1,
            "code": "prefer-const",
            "source": "deno-lint",
            "message": "'message' is never reassigned\nUse 'const' instead",
            "relatedInformation": []
          }
        ],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Disable prefer-const for this line",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 5 },
          "end": { "line": 1, "character": 12 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'message' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 1, "character": 0 },
              "end": { "line": 1, "character": 0 }
            },
            "newText": "// deno-lint-ignore prefer-const\n"
          }]
        }
      }
    }, {
      "title": "Disable prefer-const for the entire file",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 5 },
          "end": { "line": 1, "character": 12 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'message' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore-file prefer-const\n"
          }]
        }
      }
    }, {
      "title": "Ignore lint errors for the entire file",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 5 },
          "end": { "line": 1, "character": 12 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'message' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore-file\n"
          }]
        }
      }
    }])
  );
  client.shutdown();
}

/// This test exercises updating an existing deno-lint-ignore-file comment.
#[test]
fn lsp_code_actions_update_ignore_lint() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text":
"#!/usr/bin/env -S deno run
// deno-lint-ignore-file camelcase
let snake_case = 'Hello, Deno!';
console.log(snake_case);
",
    }
  }));
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 3, "character": 5 },
        "end": { "line": 3, "character": 15 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 3, "character": 5 },
            "end": { "line": 3, "character": 15 }
          },
          "severity": 1,
          "code": "prefer-const",
          "source": "deno-lint",
          "message": "'snake_case' is never reassigned\nUse 'const' instead",
          "relatedInformation": []
        }],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Disable prefer-const for this line",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 3, "character": 5 },
          "end": { "line": 3, "character": 15 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'snake_case' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 3, "character": 0 },
              "end": { "line": 3, "character": 0 }
            },
            "newText": "// deno-lint-ignore prefer-const\n"
          }]
        }
      }
    }, {
      "title": "Disable prefer-const for the entire file",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 3, "character": 5 },
          "end": { "line": 3, "character": 15 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'snake_case' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 1, "character": 34 },
              "end": { "line": 1, "character": 34 }
            },
            "newText": " prefer-const"
          }]
        }
      }
    }, {
      "title": "Ignore lint errors for the entire file",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 3, "character": 5 },
          "end": { "line": 3, "character": 15 }
        },
        "severity": 1,
        "code": "prefer-const",
        "source": "deno-lint",
        "message": "'snake_case' is never reassigned\nUse 'const' instead",
        "relatedInformation": []
      }],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore-file\n"
          }]
        }
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_code_actions_lint_fixes() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "window;",
    }
  }));
  let diagnostics = diagnostics.all();
  let diagnostic = &diagnostics[0];
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      },
      "range": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 6 }
      },
      "context": {
        "diagnostics": [diagnostic],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Rename window to globalThis",
      "kind": "quickfix",
      "diagnostics": [diagnostic],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 6 }
            },
            "newText": "globalThis"
          }]
        }
      }
    }, {
      "title": "Disable no-window for this line",
      "kind": "quickfix",
      "diagnostics": [diagnostic],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore no-window\n"
          }]
        }
      }
    }, {
      "title": "Disable no-window for the entire file",
      "kind": "quickfix",
      "diagnostics": [diagnostic],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore-file no-window\n"
          }]
        }
      }
    }, {
      "title": "Ignore lint errors for the entire file",
      "kind": "quickfix",
      "diagnostics": [diagnostic],
      "edit": {
        "changes": {
          "file:///a/file.ts": [{
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 0 }
            },
            "newText": "// deno-lint-ignore-file\n"
          }]
        }
      }
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_lint_with_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "deno.lint.jsonc",
    r#"{
    "lint": {
      "rules": {
        "exclude": ["camelcase"],
        "include": ["ban-untagged-todo"],
        "tags": []
      }
    }
  }
  "#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.lint.jsonc");
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "// TODO: fixme\nexport async function non_camel_case() {\nconsole.log(\"finished!\")\n}"
    }
  }));
  let diagnostics = diagnostics.all();
  assert_eq!(diagnostics.len(), 1);
  assert_eq!(
    diagnostics[0].code,
    Some(lsp::NumberOrString::String("ban-untagged-todo".to_string()))
  );
  client.shutdown();
}

#[test]
fn lsp_lint_exclude_with_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "deno.lint.jsonc",
    r#"{
      "lint": {
        "files": {
          "exclude": ["ignored.ts"]
        },
        "rules": {
          "exclude": ["camelcase"],
          "include": ["ban-untagged-todo"],
          "tags": []
        }
      }
    }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_config("./deno.lint.jsonc");
  });

  let diagnostics = client.did_open(
    json!({
      "textDocument": {
        "uri": ModuleSpecifier::from_file_path(temp_dir.path().join("ignored.ts")).unwrap().to_string(),
        "languageId": "typescript",
        "version": 1,
        "text": "// TODO: fixme\nexport async function non_camel_case() {\nconsole.log(\"finished!\")\n}"
      }
    }),
  );
  let diagnostics = diagnostics.all();
  assert_eq!(diagnostics, Vec::new());
  client.shutdown();
}

#[test]
fn lsp_jsx_import_source_pragma() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.tsx",
      "languageId": "typescriptreact",
      "version": 1,
      "text":
"/** @jsxImportSource http://localhost:4545/jsx */

function A() {
  return \"hello\";
}

export function B() {
  return <A></A>;
}
",
    }
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        ["http://127.0.0.1:4545/jsx/jsx-runtime"],
        "file:///a/file.tsx",
      ],
    }),
  );
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.tsx"
      },
      "position": { "line": 0, "character": 25 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/jsx/jsx-runtime\n",
      },
      "range": {
        "start": { "line": 0, "character": 21 },
        "end": { "line": 0, "character": 46 }
      }
    })
  );

  let diagnostics = client.read_diagnostics();
  println!("{:?}", diagnostics);

  client.shutdown();
}

#[ignore = "https://github.com/denoland/deno/issues/21770"]
#[test]
fn lsp_jsx_import_source_config_file_automatic_cache() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "compilerOptions": {
        "jsx": "react-jsx",
        "jsxImportSource": "http://localhost:4545/jsx",
      },
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let mut diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.tsx").unwrap(),
      "languageId": "typescriptreact",
      "version": 1,
      "text": "
        export function Foo() {
          return <div></div>;
        }
      ",
    },
  }));
  // The caching is done on an asynchronous task spawned after init, so there's
  // a chance it wasn't done in time and we need to wait for another batch of
  // diagnostics.
  while !diagnostics.all().is_empty() {
    std::thread::sleep(std::time::Duration::from_millis(50));
    // The post-cache diagnostics update triggers inconsistently on CI for some
    // reason. Force it with this notification.
    diagnostics = client.did_open(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.tsx").unwrap(),
        "languageId": "typescriptreact",
        "version": 1,
        "text": "
          export function Foo() {
            return <div></div>;
          }
        ",
      },
    }));
  }
  assert_eq!(diagnostics.all(), vec![]);
  client.shutdown();
}

#[test]
fn lsp_jsx_import_source_types_pragma() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.tsx",
      "languageId": "typescriptreact",
      "version": 1,
      "text":
"/** @jsxImportSource http://localhost:4545/jsx */
/** @jsxImportSourceTypes http://localhost:4545/jsx-types */
/** @jsxRuntime automatic */

function A() {
  return <a>Hello</a>;
}

export function B() {
  return <A></A>;
}
",
    }
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [
        [],
        "file:///a/file.tsx",
      ],
    }),
  );

  let diagnostics = client.read_diagnostics();
  assert_eq!(json!(diagnostics.all()), json!([]));

  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": "file:///a/file.tsx"
      },
      "position": { "line": 0, "character": 25 }
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/jsx/jsx-runtime\n\n**Types**: http&#8203;://localhost:4545/jsx-types/jsx-runtime\n",
      },
      "range": {
        "start": { "line": 0, "character": 21 },
        "end": { "line": 0, "character": 46 }
      }
    })
  );

  client.shutdown();
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TestData {
  id: String,
  label: String,
  steps: Option<Vec<TestData>>,
  range: Option<lsp::Range>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum TestModuleNotificationKind {
  Insert,
  Replace,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestModuleNotificationParams {
  text_document: lsp::TextDocumentIdentifier,
  kind: TestModuleNotificationKind,
  label: String,
  tests: Vec<TestData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnqueuedTestModule {
  text_document: lsp::TextDocumentIdentifier,
  ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestRunResponseParams {
  enqueued: Vec<EnqueuedTestModule>,
}

#[test]
fn lsp_testing_api() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  let contents = r#"
Deno.test({
  name: "test a",
  async fn(t) {
    console.log("test a");
    await t.step("step of test a", () => {});
  }
});
"#;
  temp_dir.write("./test.ts", contents);
  temp_dir.write("./deno.jsonc", "{}");
  let specifier = temp_dir.uri().join("test.ts").unwrap();

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  client.did_open(json!({
    "textDocument": {
      "uri": specifier,
      "languageId": "typescript",
      "version": 1,
      "text": contents,
    }
  }));

  let notification =
    client.read_notification_with_method::<Value>("deno/testModule");
  let params: TestModuleNotificationParams =
    serde_json::from_value(notification.unwrap()).unwrap();
  assert_eq!(params.text_document.uri, specifier);
  assert_eq!(params.kind, TestModuleNotificationKind::Replace);
  assert_eq!(params.label, "test.ts");
  assert_eq!(params.tests.len(), 1);
  let test = &params.tests[0];
  assert_eq!(test.label, "test a");
  assert_eq!(
    test.range,
    Some(lsp::Range {
      start: lsp::Position {
        line: 1,
        character: 5,
      },
      end: lsp::Position {
        line: 1,
        character: 9,
      }
    })
  );
  let steps = test.steps.as_ref().unwrap();
  assert_eq!(steps.len(), 1);
  let step = &steps[0];
  assert_eq!(step.label, "step of test a");
  assert_eq!(
    step.range,
    Some(lsp::Range {
      start: lsp::Position {
        line: 5,
        character: 12,
      },
      end: lsp::Position {
        line: 5,
        character: 16,
      }
    })
  );

  let res = client.write_request_with_res_as::<TestRunResponseParams>(
    "deno/testRun",
    json!({
      "id": 1,
      "kind": "run",
    }),
  );
  assert_eq!(res.enqueued.len(), 1);
  assert_eq!(res.enqueued[0].text_document.uri, specifier);
  assert_eq!(res.enqueued[0].ids.len(), 1);
  let id = res.enqueued[0].ids[0].clone();
  let notification =
    client.read_notification_with_method::<Value>("deno/testRunProgress");
  assert_eq!(
    notification,
    Some(json!({
      "id": 1,
      "message": {
        "type": "started",
        "test": {
          "textDocument": {
            "uri": specifier,
          },
          "id": id,
        },
      }
    }))
  );

  let notification =
    client.read_notification_with_method::<Value>("deno/testRunProgress");
  let notification_value = notification
    .as_ref()
    .unwrap()
    .as_object()
    .unwrap()
    .get("message")
    .unwrap()
    .as_object()
    .unwrap()
    .get("value")
    .unwrap()
    .as_str()
    .unwrap();
  // deno test's output capturing flushes with a zero-width space in order to
  // synchronize the output pipes. Occasionally this zero width space
  // might end up in the output so strip it from the output comparison here.
  assert_eq!(notification_value.replace('\u{200B}', ""), "test a\r\n");
  assert_eq!(
    notification,
    Some(json!({
      "id": 1,
      "message": {
        "type": "output",
        "value": notification_value,
        "test": {
          "textDocument": {
            "uri": specifier,
          },
          "id": id,
        },
      }
    }))
  );

  let notification =
    client.read_notification_with_method::<Value>("deno/testRunProgress");
  assert_eq!(
    notification,
    Some(json!({
      "id": 1,
      "message": {
        "type": "started",
        "test": {
          "textDocument": {
            "uri": specifier,
          },
          "id": id,
          "stepId": step.id,
        },
      }
    }))
  );

  let notification =
    client.read_notification_with_method::<Value>("deno/testRunProgress");
  let mut notification = notification.unwrap();
  let duration = notification
    .as_object_mut()
    .unwrap()
    .get_mut("message")
    .unwrap()
    .as_object_mut()
    .unwrap()
    .remove("duration");
  assert!(duration.is_some());
  assert_eq!(
    notification,
    json!({
      "id": 1,
      "message": {
        "type": "passed",
        "test": {
          "textDocument": {
            "uri": specifier,
          },
          "id": id,
          "stepId": step.id,
        },
      }
    })
  );

  let notification =
    client.read_notification_with_method::<Value>("deno/testRunProgress");
  let notification = notification.unwrap();
  let obj = notification.as_object().unwrap();
  assert_eq!(obj.get("id"), Some(&json!(1)));
  let message = obj.get("message").unwrap().as_object().unwrap();
  match message.get("type").and_then(|v| v.as_str()) {
    Some("passed") => {
      assert_eq!(
        message.get("test"),
        Some(&json!({
          "textDocument": {
            "uri": specifier
          },
          "id": id,
        }))
      );
      assert!(message.contains_key("duration"));

      let notification =
        client.read_notification_with_method::<Value>("deno/testRunProgress");
      assert_eq!(
        notification,
        Some(json!({
          "id": 1,
          "message": {
            "type": "end",
          }
        }))
      );
    }
    // sometimes on windows, the messages come out of order, but it actually is
    // working, so if we do get the end before the passed, we will simply let
    // the test pass
    Some("end") => (),
    _ => panic!("unexpected message {}", json!(notification)),
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/899.
  temp_dir.write("./test.ts", "");
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("test.ts").unwrap(),
        "version": 2
      },
      "contentChanges": [{ "text": "" }],
    }),
  );

  assert_eq!(client.read_diagnostics().all().len(), 0);

  let notification =
    client.read_notification_with_method::<Value>("deno/testModuleDelete");
  assert_eq!(
    notification,
    Some(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("test.ts").unwrap()
      }
    }))
  );

  client.shutdown();
}

#[test]
fn lsp_closed_file_find_references() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("./mod.ts", "export const a = 5;");
  temp_dir.write(
    "./mod.test.ts",
    "import { a } from './mod.ts'; console.log(a);",
  );
  let temp_dir_url = temp_dir.uri();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir_url.join("mod.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"export const a = 5;"#
    }
  }));
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": temp_dir_url.join("mod.ts").unwrap(),
      },
      "position": { "line": 0, "character": 13 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
      "uri": temp_dir_url.join("mod.test.ts").unwrap(),
      "range": {
        "start": { "line": 0, "character": 9 },
        "end": { "line": 0, "character": 10 }
      }
    }, {
      "uri": temp_dir_url.join("mod.test.ts").unwrap(),
      "range": {
        "start": { "line": 0, "character": 42 },
        "end": { "line": 0, "character": 43 }
      }
    }])
  );

  client.shutdown();
}

#[test]
fn lsp_closed_file_find_references_low_document_pre_load() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("sub_dir");
  temp_dir.write("./other_file.ts", "export const b = 5;");
  temp_dir.write("./sub_dir/mod.ts", "export const a = 5;");
  temp_dir.write(
    "./sub_dir/mod.test.ts",
    "import { a } from './mod.ts'; console.log(a);",
  );
  let temp_dir_url = temp_dir.uri();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_preload_limit(1);
  });
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir_url.join("sub_dir/mod.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"export const a = 5;"#
    }
  }));
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": temp_dir_url.join("sub_dir/mod.ts").unwrap(),
      },
      "position": { "line": 0, "character": 13 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  // won't have results because the document won't be pre-loaded
  assert_eq!(res, json!([]));

  client.shutdown();
}

#[test]
fn lsp_closed_file_find_references_excluded_path() {
  // we exclude any files or folders in the "exclude" part of
  // the config file from being pre-loaded
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("sub_dir");
  temp_dir.create_dir_all("other_dir/sub_dir");
  temp_dir.write("./sub_dir/mod.ts", "export const a = 5;");
  temp_dir.write(
    "./sub_dir/mod.test.ts",
    "import { a } from './mod.ts'; console.log(a);",
  );
  temp_dir.write(
    "./other_dir/sub_dir/mod.test.ts",
    "import { a } from '../../sub_dir/mod.ts'; console.log(a);",
  );
  temp_dir.write(
    "deno.json",
    r#"{
  "exclude": [
    "./sub_dir/mod.test.ts",
    "./other_dir/sub_dir",
  ]
}"#,
  );
  let temp_dir_url = temp_dir.uri();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir_url.join("sub_dir/mod.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"export const a = 5;"#
    }
  }));
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": temp_dir_url.join("sub_dir/mod.ts").unwrap(),
      },
      "position": { "line": 0, "character": 13 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  // won't have results because the documents won't be pre-loaded
  assert_eq!(res, json!([]));

  client.shutdown();
}

#[test]
fn lsp_data_urls_with_jsx_compiler_option() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    r#"{ "compilerOptions": { "jsx": "react-jsx" } }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let uri = temp_dir.uri().join("main.ts").unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import a from \"data:application/typescript,export default 5;\";\na;"
    }
  })).all();

  assert_eq!(diagnostics.len(), 0);

  let res: Value = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": uri
      },
      "position": { "line": 1, "character": 1 },
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  assert_eq!(
    res,
    json!([{
      "uri": uri,
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 8 }
      }
    }, {
      "uri": uri,
      "range": {
        "start": { "line": 1, "character": 0 },
        "end": { "line": 1, "character": 1 }
      }
    }, {
      "uri": "deno:/ed0224c51f7e2a845dfc0941ed6959675e5e3e3d2a39b127f0ff569c1ffda8d8/data_url.ts",
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": {"line": 0, "character": 14 },
      },
    }])
  );

  client.shutdown();
}

#[test]
fn lsp_node_modules_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  // having a package.json should have no effect on whether
  // a node_modules dir is created
  temp_dir.write("package.json", "{}");

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let file_uri = temp_dir.uri().join("file.ts").unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import chalk from 'npm:chalk';\nimport path from 'node:path';\n\nconsole.log(chalk.green(path.join('a', 'b')));",
    }
  }));
  let cache = |client: &mut LspClient| {
    client.write_request(
      "workspace/executeCommand",
      json!({
        "command": "deno.cache",
        "arguments": [["npm:chalk", "npm:@types/node"], file_uri],
      }),
    );
  };

  cache(&mut client);

  assert!(!temp_dir.path().join("node_modules").exists());

  // a lockfile will be created here because someone did an explicit cache
  let lockfile_path = temp_dir.path().join("deno.lock");
  assert!(lockfile_path.exists());
  lockfile_path.remove_file();

  temp_dir.write(
    temp_dir.path().join("deno.json"),
    "{ \"nodeModulesDir\": true, \"lock\": false }\n",
  );
  let refresh_config = |client: &mut LspClient| {
    client.change_configuration(json!({ "deno": {
      "enable": true,
      "config": "./deno.json",
      "codeLens": {
        "implementations": true,
        "references": true,
      },
      "importMap": null,
      "lint": false,
      "suggest": {
        "autoImports": true,
        "completeFunctionCalls": false,
        "names": true,
        "paths": true,
        "imports": {},
      },
      "unstable": false,
    } }));
  };
  refresh_config(&mut client);

  let diagnostics = client.read_diagnostics();
  assert_eq!(diagnostics.all().len(), 2, "{:#?}", diagnostics); // not cached

  cache(&mut client);

  assert!(temp_dir.path().join("node_modules/chalk").exists());
  assert!(temp_dir.path().join("node_modules/@types/node").exists());
  assert!(!lockfile_path.exists()); // was disabled

  // now add a lockfile and cache
  temp_dir.write(
    temp_dir.path().join("deno.json"),
    "{ \"nodeModulesDir\": true }\n",
  );
  refresh_config(&mut client);
  cache(&mut client);

  let diagnostics = client.read_diagnostics();
  assert_eq!(diagnostics.all().len(), 0, "{:#?}", diagnostics);

  assert!(lockfile_path.exists());

  // the declaration should be found in the node_modules directory
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": file_uri,
      },
      "position": { "line": 0, "character": 7 }, // chalk
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  // ensure that it's using the node_modules directory
  let references = res.as_array().unwrap();
  assert_eq!(references.len(), 2, "references: {:#?}", references);
  let uri = references[1]
    .as_object()
    .unwrap()
    .get("uri")
    .unwrap()
    .as_str()
    .unwrap();
  // canonicalize for mac
  let path = temp_dir.path().join("node_modules").canonicalize();
  assert_starts_with!(
    uri,
    ModuleSpecifier::from_file_path(&path).unwrap().as_str()
  );

  client.shutdown();
}

#[test]
fn lsp_vendor_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let local_file_uri = temp_dir.uri().join("file.ts").unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": local_file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { returnsHi } from 'http://localhost:4545/subdir/mod1.ts';\nconst test: string = returnsHi();\nconsole.log(test);",
    }
  }));
  let cache = |client: &mut LspClient| {
    client.write_request(
      "workspace/executeCommand",
      json!({
        "command": "deno.cache",
        "arguments": [["http://localhost:4545/subdir/mod1.ts"], local_file_uri],
      }),
    );
  };

  cache(&mut client);

  assert!(!temp_dir.path().join("vendor").exists());

  // read the diagnostic update after caching
  let diagnostics = client.read_diagnostics();
  assert_eq!(diagnostics.all().len(), 0);

  temp_dir.write(
    temp_dir.path().join("deno.json"),
    "{ \"vendor\": true, \"lock\": false }\n",
  );
  client.change_configuration(json!({ "deno": {
    "enable": true,
    "config": "./deno.json",
    "codeLens": {
      "implementations": true,
      "references": true,
    },
    "importMap": null,
    "lint": false,
    "suggest": {
      "autoImports": true,
      "completeFunctionCalls": false,
      "names": true,
      "paths": true,
      "imports": {},
    },
    "unstable": false,
  } }));

  let diagnostics = client.read_diagnostics();
  // won't be cached until a manual cache occurs
  assert_eq!(
    diagnostics
      .all()
      .iter()
      .map(|d| d.message.as_str())
      .collect::<Vec<_>>(),
    vec![
      "Uncached or missing remote URL: http://localhost:4545/subdir/mod1.ts"
    ]
  );

  assert!(!temp_dir
    .path()
    .join("vendor/http_localhost_4545/subdir/mod1.ts")
    .exists());

  // now cache
  cache(&mut client);
  let diagnostics = client.read_diagnostics();
  assert_eq!(diagnostics.all().len(), 0, "{:#?}", diagnostics); // cached
  assert!(temp_dir
    .path()
    .join("vendor/http_localhost_4545/subdir/mod1.ts")
    .exists());

  // the declaration should be found in the vendor directory
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": {
        "uri": local_file_uri,
      },
      "position": { "line": 0, "character": 9 }, // returnsHi
      "context": {
        "includeDeclaration": false
      }
    }),
  );

  // ensure that it's using the vendor directory
  let references = res.as_array().unwrap();
  assert_eq!(references.len(), 2, "references: {:#?}", references);
  let uri = references[1]
    .as_object()
    .unwrap()
    .get("uri")
    .unwrap()
    .as_str()
    .unwrap();
  let file_path = temp_dir
    .path()
    .join("vendor/http_localhost_4545/subdir/mod1.ts");
  let remote_file_uri = file_path.uri_file();
  assert_eq!(uri, remote_file_uri.as_str());

  let file_text = file_path.read_to_string();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": remote_file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": file_text,
    }
  }));
  assert_eq!(diagnostics.all(), Vec::new());

  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": remote_file_uri,
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 17, "character": 0 },
          },
          "text": "export function returnsHi(): number { return new Date(); }"
        }
      ]
    }),
  );

  let diagnostics = client.read_diagnostics();

  assert_eq!(
    json!(
      diagnostics
        .messages_with_file_and_source(remote_file_uri.as_str(), "deno-ts")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 38 },
          "end": { "line": 0, "character": 44 }
        },
        "severity": 1,
        "code": 2322,
        "source": "deno-ts",
        "message": "Type 'Date' is not assignable to type 'number'."
      }
    ]),
  );

  assert_eq!(
    json!(
      diagnostics
        .messages_with_file_and_source(local_file_uri.as_str(), "deno-ts")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": { "line": 1, "character": 6 },
          "end": { "line": 1, "character": 10 }
        },
        "severity": 1,
        "code": 2322,
        "source": "deno-ts",
        "message": "Type 'number' is not assignable to type 'string'."
      }
    ]),
  );
  assert_eq!(diagnostics.all().len(), 2);

  // now try doing a relative import into the vendor directory
  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": local_file_uri,
        "version": 2
      },
      "contentChanges": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 2, "character": 0 },
          },
          "text": "import { returnsHi } from './vendor/subdir/mod1.ts';\nconst test: string = returnsHi();\nconsole.log(test);"
        }
      ]
    }),
  );

  let diagnostics = client.read_diagnostics();

  assert_eq!(
    json!(
      diagnostics
        .messages_with_file_and_source(local_file_uri.as_str(), "deno")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 26 },
          "end": { "line": 0, "character": 51 }
        },
        "severity": 1,
        "code": "resolver-error",
        "source": "deno",
        "message": "Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring."
      }
    ]),
  );

  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_import_map() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2/project3");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "imports": {
        "foo": "./foo1.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("project1/foo1.ts", "");
  temp_dir.write(
    "project2/deno.json",
    json!({
      "imports": {
        "foo": "./foo2.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("project2/foo2.ts", "");
  temp_dir.write(
    "project2/project3/deno.json",
    json!({
      "imports": {
        "foo": "./foo3.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("project2/project3/foo3.ts", "");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"foo\";\n",
    },
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": format!("**Resolved Dependency**\n\n**Code**: file&#8203;{}\n", temp_dir.uri().join("project1/foo1.ts").unwrap().as_str().trim_start_matches("file")),
      },
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 12 },
      },
    })
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"foo\";\n",
    },
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": format!("**Resolved Dependency**\n\n**Code**: file&#8203;{}\n", temp_dir.uri().join("project2/foo2.ts").unwrap().as_str().trim_start_matches("file")),
      },
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 12 },
      },
    })
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"foo\";\n",
    },
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": format!("**Resolved Dependency**\n\n**Code**: file&#8203;{}\n", temp_dir.uri().join("project2/project3/foo3.ts").unwrap().as_str().trim_start_matches("file")),
      },
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 12 },
      },
    })
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_vendor_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2/project3");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "vendor": true,
    })
    .to_string(),
  );
  temp_dir.write(
    "project2/deno.json",
    json!({
      "vendor": true,
    })
    .to_string(),
  );
  temp_dir.write(
    "project2/project3/deno.json",
    json!({
      "vendor": true,
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod1.ts\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project1/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": temp_dir.uri().join("project1/vendor/http_localhost_4545/subdir/mod1.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 17,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 17,
          "character": 0,
        },
      },
    }]),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod2.ts\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project2/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": temp_dir.uri().join("project2/vendor/http_localhost_4545/subdir/mod2.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod3.js\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project2/project3/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": temp_dir.uri().join("project2/project3/vendor/http_localhost_4545/subdir/mod3.js").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_node_modules_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2/project3");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  temp_dir.write(
    "project2/deno.json",
    json!({
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  temp_dir.write(
    "project2/project3/deno.json",
    json!({
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"npm:@denotest/add@1\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project1/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  // The temp dir is symlinked in macos, and `node_modules` is canonicalized.
  let canon_temp_dir =
    Url::from_directory_path(temp_dir.path().canonicalize()).unwrap();
  assert_eq!(
    res,
    json!([{
      "targetUri": canon_temp_dir.join("project1/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add/index.d.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"npm:@denotest/add@1\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project2/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": canon_temp_dir.join("project2/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add/index.d.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"npm:@denotest/add@1\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project2/project3/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/project3/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": canon_temp_dir.join("project2/project3/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add/index.d.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_ts_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write(
    "project2/deno.json",
    json!({
      "compilerOptions": {
        "lib": ["deno.worker"],
      },
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "Window;\nWorkerGlobalScope;\n",
    },
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "Window;\nWorkerGlobalScope;\n",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([
      {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
        "version": 1,
        "diagnostics": [
          {
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 6 },
            },
            "severity": 1,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'Window'.",
          },
        ],
      },
      {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
        "version": 1,
        "diagnostics": [
          {
            "range": {
              "start": { "line": 1, "character": 0 },
              "end": { "line": 1, "character": 17 },
            },
            "severity": 1,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'WorkerGlobalScope'.",
          },
        ],
      }
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_declaration_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  temp_dir.write("project1/foo.d.ts", "declare type Foo = number;\n");
  temp_dir.write("project2/bar.d.ts", "declare type Bar = number;\n");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "export const foo: Foo = 1;\nexport const bar: Bar = 1;\n",
    },
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "export const foo: Foo = 1;\nexport const bar: Bar = 1;\n",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([
      {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
        "version": 1,
        "diagnostics": [
          {
            "range": {
              "start": { "line": 0, "character": 18 },
              "end": { "line": 0, "character": 21 },
            },
            "severity": 1,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'Foo'.",
          },
        ],
      },
      {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
        "version": 1,
        "diagnostics": [
          {
            "range": {
              "start": { "line": 1, "character": 18 },
              "end": { "line": 1, "character": 21 },
            },
            "severity": 1,
            "code": 2304,
            "source": "deno-ts",
            "message": "Cannot find name 'Bar'.",
          },
        ],
      }
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_find_references() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let file1 = source_file(
    temp_dir.path().join("project1/file.ts"),
    "export const foo = 1;\n",
  );
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    "export { foo } from \"../project1/file.ts\";\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": file1.identifier(),
      "position": file1.range_of("foo").start,
      "context": {
        "includeDeclaration": true,
      },
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "uri": file1.uri(),
        "range": file1.range_of("foo"),
      },
      {
        "uri": file2.uri(),
        "range": file2.range_of("foo"),
      },
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_file_rename_import_edits() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let file1 = source_file(temp_dir.path().join("project1/file.ts"), "");
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    "import \"../project1/file.ts\";\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "workspace/willRenameFiles",
    json!({
      "files": [
        {
          "oldUri": file1.uri(),
          "newUri": file1.uri().join("file_renamed.ts").unwrap(),
        },
      ],
    }),
  );
  assert_eq!(
    res,
    json!({
      "documentChanges": [
        {
          "textDocument": {
            "uri": file2.uri(),
            "version": null,
          },
          "edits": [
            {
              "range": file2.range_of("../project1/file.ts"),
              "newText": "../project1/file_renamed.ts",
            },
          ],
        },
      ],
    }),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_goto_implementations() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let file1 = source_file(
    temp_dir.path().join("project1/file.ts"),
    "export interface Foo {}\n",
  );
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    r#"
      import type { Foo } from "../project1/file.ts";
      export class SomeFoo implements Foo {}
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/implementation",
    json!({
      "textDocument": file1.identifier(),
      "position": file1.range_of("Foo").start,
    }),
  );
  assert_eq!(
    res,
    json!([
      {
        "targetUri": file2.uri(),
        "targetRange": file2.range_of("export class SomeFoo implements Foo {}"),
        "targetSelectionRange": file2.range_of("SomeFoo"),
      },
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_call_hierarchy() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.create_dir_all("project3");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  temp_dir.write("project3/deno.json", json!({}).to_string());
  let file1 = source_file(
    temp_dir.path().join("project1/file.ts"),
    r#"
      export function foo() {}
    "#,
  );
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    r#"
      import { foo } from "../project1/file.ts";
      export function bar() {
        foo();
      }
    "#,
  );
  let file3 = source_file(
    temp_dir.path().join("project3/file.ts"),
    r#"
      import { bar } from "../project2/file.ts";
      bar();
    "#,
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/prepareCallHierarchy",
    json!({
      "textDocument": file2.identifier(),
      "position": file2.range_of("bar").start,
    }),
  );
  assert_eq!(
    &res,
    &json!([
      {
        "name": "bar",
        "kind": 12,
        "detail": "",
        "uri": file2.uri(),
        "range": {
          "start": { "line": 2, "character": 6 },
          "end": { "line": 4, "character": 7 },
        },
        "selectionRange": file2.range_of("bar"),
      },
    ]),
  );
  let item = res.as_array().unwrap().first().unwrap();
  let res = client
    .write_request("callHierarchy/incomingCalls", json!({ "item": item }));
  assert_eq!(
    res,
    json!([
      {
        "from": {
          "name": "file.ts",
          "kind": 2,
          "detail": "project3",
          "uri": file3.uri(),
          "range": {
            "start": { "line": 1, "character": 6 },
            "end": { "line": 3, "character": 4 },
          },
          "selectionRange": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 },
          },
        },
        "fromRanges": [
          {
            "start": { "line": 2, "character": 6 },
            "end": { "line": 2, "character": 9 },
          },
        ],
      },
    ]),
  );
  let res = client
    .write_request("callHierarchy/outgoingCalls", json!({ "item": item }));
  assert_eq!(
    res,
    json!([
      {
        "to": {
          "name": "foo",
          "kind": 12,
          "detail": "",
          "uri": file1.uri(),
          "range": file1.range_of("export function foo() {}"),
          "selectionRange": file1.range_of("foo"),
        },
        "fromRanges": [
          {
            "start": { "line": 3, "character": 8 },
            "end": { "line": 3, "character": 11 },
          },
        ],
      },
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_rename_symbol() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let file1 = source_file(
    temp_dir.path().join("project1/file.ts"),
    "export const foo = 1;\n",
  );
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    "export { foo } from \"../project1/file.ts\";\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res = client.write_request(
    "textDocument/rename",
    json!({
      "textDocument": file1.identifier(),
      "position": file1.range_of("foo").start,
      "newName": "bar",
    }),
  );
  assert_eq!(
    res,
    json!({
      "documentChanges": [
        {
          "textDocument": {
            "uri": file1.uri(),
            "version": null,
          },
          "edits": [
            {
              "range": file1.range_of("foo"),
              "newText": "bar",
            },
          ],
        },
        {
          "textDocument": {
            "uri": file2.uri(),
            "version": null,
          },
          "edits": [
            {
              "range": file2.range_of("foo"),
              "newText": "bar",
            },
          ],
        },
      ],
    }),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_scopes_search_symbol() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1");
  temp_dir.create_dir_all("project2");
  temp_dir.write("project1/deno.json", json!({}).to_string());
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let file1 = source_file(
    temp_dir.path().join("project1/file.ts"),
    "export const someSymbol1 = 1;\n",
  );
  let file2 = source_file(
    temp_dir.path().join("project2/file.ts"),
    "export const someSymbol2 = 2;\n",
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let res =
    client.write_request("workspace/symbol", json!({ "query": "someSymbol" }));
  assert_eq!(
    res,
    json!([
      {
        "name": "someSymbol1",
        "kind": 13,
        "location": {
          "uri": file1.uri(),
          "range": file1.range_of("someSymbol1 = 1"),
        },
        "containerName": "",
      },
      {
        "name": "someSymbol2",
        "kind": 13,
        "location": {
          "uri": file2.uri(),
          "range": file2.range_of("someSymbol2 = 2"),
        },
        "containerName": "",
      },
    ]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_fmt_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "workspace": ["project1", "project2"],
      "fmt": {
        "semiColons": false,
      },
    })
    .to_string(),
  );
  temp_dir.create_dir_all("project1");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "fmt": {
        "singleQuote": true,
      },
    })
    .to_string(),
  );
  temp_dir.create_dir_all("project2");
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(\"\");\n",
    },
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true,
      },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 15 },
        "end": { "line": 0, "character": 16 },
      },
      "newText": "",
    }])
  );
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(\"\");\n",
    },
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true,
      },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 12 },
        "end": { "line": 0, "character": 16 },
      },
      "newText": "'')",
    }])
  );
  // `project2/file.ts` should use the fmt settings from `deno.json`, since it
  // has no fmt field.
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(\"\");\n",
    },
  }));
  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      },
      "options": {
        "tabSize": 2,
        "insertSpaces": true,
      },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "range": {
        "start": { "line": 0, "character": 15 },
        "end": { "line": 0, "character": 16 },
      },
      "newText": "",
    }])
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_lint_config() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "workspace": ["project1", "project2"],
      "lint": {
        "rules": {
          "include": ["camelcase"],
        },
      },
    })
    .to_string(),
  );
  temp_dir.create_dir_all("project1");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "lint": {
        "rules": {
          "include": ["ban-untagged-todo"],
        },
      },
    })
    .to_string(),
  );
  temp_dir.create_dir_all("project2");
  temp_dir.write("project2/deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        // TODO: Unused var
        const snake_case_var = 1;
        console.log(snake_case_var);
      "#,
    },
  }));
  assert_eq!(
    json!(diagnostics.messages_with_source("deno-lint")),
    json!({
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "diagnostics": [{
        "range": {
          "start": { "line": 2, "character": 14 },
          "end": { "line": 2, "character": 28 },
        },
        "severity": 2,
        "code": "camelcase",
        "source": "deno-lint",
        "message": "Identifier 'snake_case_var' is not in camel case.\nConsider renaming `snake_case_var` to `snakeCaseVar`",
      }],
      "version": 1,
    })
  );
  client.write_notification(
    "textDocument/didClose",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
    }),
  );
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        // TODO: Unused var
        const snake_case_var = 1;
        console.log(snake_case_var);
      "#,
    },
  }));
  assert_eq!(
    json!(diagnostics.messages_with_source("deno-lint")),
    json!({
      "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      "diagnostics": [{
        "range": {
          "start": { "line": 1, "character": 8 },
          "end": { "line": 1, "character": 27 },
        },
        "severity": 2,
        "code": "ban-untagged-todo",
        "source": "deno-lint",
        "message": "TODO should be tagged with (@username) or (#issue)\nAdd a user tag or issue reference to the TODO comment, e.g. TODO(@djones), TODO(djones), TODO(#123)",
      }, {
        "range": {
          "start": { "line": 2, "character": 14 },
          "end": { "line": 2, "character": 28 },
        },
        "severity": 2,
        "code": "camelcase",
        "source": "deno-lint",
        "message": "Identifier 'snake_case_var' is not in camel case.\nConsider renaming `snake_case_var` to `snakeCaseVar`",
      }],
      "version": 1,
    })
  );
  client.write_notification(
    "textDocument/didClose",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
      },
    }),
  );
  // `project2/file.ts` should use the lint settings from `deno.json`, since it
  // has no lint field.
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        // TODO: Unused var
        const snake_case_var = 1;
        console.log(snake_case_var);
      "#,
    },
  }));
  assert_eq!(
    json!(diagnostics.messages_with_source("deno-lint")),
    json!({
      "uri": temp_dir.uri().join("project2/file.ts").unwrap(),
      "diagnostics": [{
        "range": {
          "start": { "line": 2, "character": 14 },
          "end": { "line": 2, "character": 28 },
        },
        "severity": 2,
        "code": "camelcase",
        "source": "deno-lint",
        "message": "Identifier 'snake_case_var' is not in camel case.\nConsider renaming `snake_case_var` to `snakeCaseVar`",
      }],
      "version": 1,
    })
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_import_map() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1/project2");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "workspace": ["project2"],
      "imports": {
        "foo": "./foo1.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("project1/foo1.ts", "");
  temp_dir.write(
    "project1/project2/deno.json",
    json!({
      // should overwrite the "foo" entry in the parent for this scope
      "imports": {
        "foo": "./foo2.ts",
      },
    })
    .to_string(),
  );
  temp_dir.write("project1/project2/foo2.ts", "");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  // project1 resolution
  {
    client.did_open(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "import \"foo\";\n",
      },
    }));
    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": temp_dir.uri().join("project1/file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 7 },
      }),
    );
    assert_eq!(
      res,
      json!({
        "contents": {
          "kind": "markdown",
          "value": format!("**Resolved Dependency**\n\n**Code**: file&#8203;{}\n", temp_dir.uri().join("project1/foo1.ts").unwrap().as_str().trim_start_matches("file")),
        },
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 12 },
        },
      })
    );
  }

  // project1/project2 resolution
  {
    client.did_open(json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "import \"foo\";\n",
      },
    }));
    let res = client.write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
        },
        "position": { "line": 0, "character": 7 },
      }),
    );
    assert_eq!(
      res,
      json!({
        "contents": {
          "kind": "markdown",
          "value": format!("**Resolved Dependency**\n\n**Code**: file&#8203;{}\n", temp_dir.uri().join("project1/project2/foo2.ts").unwrap().as_str().trim_start_matches("file")),
        },
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 12 },
        },
      })
    );
  }
  client.shutdown();
}

#[test]
fn lsp_workspace_lockfile() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1/project2");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "workspace": ["project2"],
    })
    .to_string(),
  );
  temp_dir.write("project1/deno.lock", json!({
    "version": "3",
    "redirects": {
      "http://localhost:4545/subdir/mod1.ts": "http://localhost:4545/subdir/mod2.ts",
    },
    "remote": {},
  }).to_string());
  temp_dir.write("project1/project2/deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod1.ts\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project1/project2/file.ts").unwrap()],
    }),
  );
  client.read_diagnostics();
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": { "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap() },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": "deno:/http/localhost%3A4545/subdir/mod2.ts",
      "targetRange": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 1, "character": 0 },
      },
      "targetSelectionRange": {
        "start": { "line": 0, "character": 0 },
        "end": { "line": 1, "character": 0 },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_vendor_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1/project2");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "workspace": ["project2"],
      "vendor": true,
    })
    .to_string(),
  );
  temp_dir.write("project1/project2/deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"http://localhost:4545/subdir/mod1.ts\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project1/project2/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!([{
      "targetUri": temp_dir.uri().join("project1/vendor/http_localhost_4545/subdir/mod1.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 17,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 17,
          "character": 0,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_node_modules_dir() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("project1/project2");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "workspace": ["project2"],
      "nodeModulesDir": true,
    })
    .to_string(),
  );
  temp_dir.write("project1/project2/deno.json", json!({}).to_string());
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"npm:@denotest/add@1\";\n",
    },
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], temp_dir.uri().join("project1/project2/file.ts").unwrap()],
    }),
  );
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("project1/project2/file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  // The temp dir is symlinked in macos, and `node_modules` is canonicalized.
  let canon_temp_dir =
    Url::from_directory_path(temp_dir.path().canonicalize()).unwrap();
  assert_eq!(
    res,
    json!([{
      "targetUri": canon_temp_dir.join("project1/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add/index.d.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 1,
          "character": 0,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_deno_json_workspace_jsr_resolution() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "workspace": ["project1"],
    })
    .to_string(),
  );
  temp_dir.create_dir_all("project1");
  temp_dir.write(
    "project1/deno.json",
    json!({
      "name": "@org/project1",
      "version": "1.0.0",
      "exports": {
        ".": "./mod.ts",
      },
    })
    .to_string(),
  );
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import \"jsr:@org/project1@^1.0.0\";\n",
    },
  }));
  let res = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("file.ts").unwrap(),
      },
      "position": { "line": 0, "character": 7 },
    }),
  );
  assert_eq!(
    res,
    json!({
      "contents": {
        "kind": "markdown",
        "value": format!("**Resolved Dependency**\n\n**Code**: jsr&#8203;:&#8203;@org/project1&#8203;@^1.0.0 (<{}project1/mod.ts>)\n", temp_dir.uri()),
      },
      "range": {
        "start": { "line": 0, "character": 7 },
        "end": { "line": 0, "character": 33 },
      },
    }),
  );
  client.shutdown();
}

#[test]
fn lsp_npm_workspace() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "package.json",
    json!({
      "workspaces": ["packages/*"]
    })
    .to_string(),
  );
  {
    temp_dir.create_dir_all("packages/add");
    temp_dir.write(
      "packages/add/package.json",
      json!({
        "name": "add",
        "version": "1.0.0",
        "exports": "./index.ts"
      })
      .to_string(),
    );
    temp_dir.write(
      "packages/add/index.ts",
      "export function add(a: number, b: number): number { return a + b; }",
    );
  }
  {
    temp_dir.create_dir_all("packages/subtract");
    temp_dir.write(
      "packages/subtract/package.json",
      json!({
        "name": "add",
        "version": "1.0.0",
        "exports": "./index.ts",
        "dependencies": {
          "add": "^1.0.0"
        }
      })
      .to_string(),
    );
  }
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("packages/subtract/index.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "import { add } from 'add';\nexport function subtract(a: number, b: number): number { return add(a, -b); }",
    },
  }));
  assert_eq!(json!(diagnostics.all()), json!([]));
  let res = client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": temp_dir.uri().join("packages/subtract/index.ts").unwrap(),
      },
      "position": { "line": 0, "character": 9 },
    }),
  );
  // The temp dir is symlinked on the CI
  assert_eq!(
    res,
    json!([{
      "targetUri": temp_dir.uri().join("packages/add/index.ts").unwrap(),
      "targetRange": {
        "start": {
          "line": 0,
          "character": 0,
        },
        "end": {
          "line": 0,
          "character": 67,
        },
      },
      "targetSelectionRange": {
        "start": {
          "line": 0,
          "character": 16,
        },
        "end": {
          "line": 0,
          "character": 19,
        },
      },
    }]),
  );
  client.shutdown();
}

#[test]
fn lsp_import_unstable_bare_node_builtins_auto_discovered() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  let contents = r#"import path from "path";"#;
  temp_dir.write("main.ts", contents);
  temp_dir.write("deno.json", r#"{ "unstable": ["bare-node-builtins"] }"#);
  let main_script = temp_dir.uri().join("main.ts").unwrap();

  let mut client = context.new_lsp_command().capture_stderr().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": main_script,
      "languageId": "typescript",
      "version": 1,
      "text": contents,
    }
  }));

  let diagnostics = diagnostics
    .messages_with_file_and_source(main_script.as_ref(), "deno")
    .diagnostics
    .into_iter()
    .filter(|d| {
      d.code
        == Some(lsp::NumberOrString::String(
          "import-node-prefix-missing".to_string(),
        ))
    })
    .collect::<Vec<_>>();

  // get the quick fixes
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": main_script
      },
      "range": {
        "start": { "line": 0, "character": 16 },
        "end": { "line": 0, "character": 18 },
      },
      "context": {
        "diagnostics": json!(diagnostics),
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Update specifier to node:path",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 17 },
            "end": { "line": 0, "character": 23 }
          },
          "severity": 2,
          "code": "import-node-prefix-missing",
          "source": "deno",
          "message": "\"path\" is resolved to \"node:path\". If you want to use a built-in Node module, add a \"node:\" prefix.",
          "data": {
            "specifier": "path"
          },
        }
      ],
      "edit": {
        "changes": {
          main_script: [
            {
              "range": {
                "start": { "line": 0, "character": 17 },
                "end": { "line": 0, "character": 23 }
              },
              "newText": "\"node:path\""
            }
          ]
        }
      }
    }])
  );

  client.shutdown();
}

#[test]
fn lsp_jupyter_byonm_diagnostics() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("package.json").write_json(&json!({
    "dependencies": {
      "@denotest/esm-basic": "*"
    }
  }));
  temp_dir.join("deno.json").write_json(&json!({
    "unstable": ["byonm"]
  }));
  context.run_npm("install");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let notebook_specifier = temp_dir.join("notebook.ipynb").uri_file();
  let notebook_specifier = format!(
    "{}#abc",
    notebook_specifier
      .to_string()
      .replace("file://", "deno-notebook-cell:")
  );
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": notebook_specifier,
      "languageId": "typescript",
      "version": 1,
      "text": "import { getValue, nonExistent } from '@denotest/esm-basic';\n console.log(getValue, nonExistent);",
    },
  }));
  assert_eq!(
    json!(diagnostics.all_messages()),
    json!([
      {
        "uri": notebook_specifier,
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 19,
              },
              "end": {
                "line": 0,
                "character": 30,
              },
            },
            "severity": 1,
            "code": 2305,
            "source": "deno-ts",
            "message": "Module '\"@denotest/esm-basic\"' has no exported member 'nonExistent'.",
          },
        ],
        "version": 1,
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_deno_future_env_byonm() {
  let context = TestContextBuilder::for_npm()
    .env("DENO_FUTURE", "1")
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.path().join("package.json").write_json(&json!({
    "dependencies": {
      "@denotest/esm-basic": "*",
    },
  }));
  context.run_npm("install");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("file.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": r#"
        import "npm:chalk";
        import "@denotest/esm-basic";
      "#,
    },
  }));
  assert_eq!(
    json!(diagnostics.all()),
    json!([
      {
        "range": {
          "start": {
            "line": 1,
            "character": 15,
          },
          "end": {
            "line": 1,
            "character": 26,
          },
        },
        "severity": 1,
        "code": "resolver-error",
        "source": "deno",
        "message": "Could not find a matching package for 'npm:chalk' in a package.json file. You must specify this as a package.json dependency when the node_modules folder is not managed by Deno.",
      },
    ])
  );
  client.shutdown();
}

#[test]
fn lsp_sloppy_imports() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let temp_dir = temp_dir.path();
  temp_dir
    .join("deno.json")
    .write(r#"{ "unstable": ["sloppy-imports"] }"#);
  // for sloppy imports, the file must exist on the file system
  // to be resolved correctly
  temp_dir.join("a.ts").write("export class A {}");
  temp_dir.join("b.ts").write("export class B {}");
  temp_dir.join("c.js").write("export class C {}");
  temp_dir.join("c.d.ts").write("export class C {}");
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_root_uri(temp_dir.uri_dir());
  });
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.join("b.ts").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": "export class B {}",
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.join("c.js").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": "export class C {}",
    },
  }));
  client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.join("c.d.ts").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": "export class C {}",
    },
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.join("file.ts").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": concat!(
        "import * as a from './a';\n",
        "import * as b from './b.js';\n",
        // this one's types resolve to a .d.ts file and we don't
        // bother warning about it because it's a bit complicated
        // to explain to use @deno-types in a diagnostic
        "import * as c from './c.js';\n",
        "console.log(a)\n",
        "console.log(b);\n",
        "console.log(c);\n",
      ),
    },
  }));

  assert_eq!(
    json!(diagnostics.all()),
    json!([{
    "range": {
          "start": { "line": 0, "character": 19 },
          "end": { "line": 0, "character": 24 }
        },
        "severity": 2,
        "code": "no-sloppy-imports",
        "source": "deno-lint",
        "message": "Sloppy imports are not allowed.",
        "data": [{
          "description": "Add a '.ts' extension.",
          "changes": [{
            "range": {
              "start": { "line": 0, "character": 19 },
              "end": { "line": 0, "character": 24 },
            },
           "new_text": "'./a.ts'"
          }]
        }]
    }, {
    "range": {
        "start": { "line": 1, "character": 19 },
        "end": { "line": 1, "character": 27 }
      },
      "severity": 2,
      "code": "no-sloppy-imports",
      "source": "deno-lint",
      "message": "Sloppy imports are not allowed.",
      "data": [{
        "description": "Change the extension to '.ts'.",
        "changes": [{
          "range": {
            "start": { "line": 1, "character": 19 },
            "end": { "line": 1, "character": 27 },
          },
         "new_text": "'./b.ts'"
        }]
      }]
    }, {
    "range": {
        "start": { "line": 2, "character": 19 },
        "end": { "line": 2, "character": 27 }
      },
      "severity": 2,
      "code": "no-sloppy-imports",
      "source": "deno-lint",
      "message": "Sloppy imports are not allowed.",
      "data": [{
        "description": "Change the extension to '.d.ts'.",
        "changes": [{
          "range": {
            "start": { "line": 2, "character": 19 },
            "end": { "line": 2, "character": 27 },
          },
         "new_text": "'./c.d.ts'"
        }]
      }]
    }])
  );

  client.shutdown();
}

#[test]
fn lsp_sloppy_imports_prefers_dts() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let temp_dir = temp_dir.path();

  temp_dir
    .join("deno.json")
    .write(r#"{ "unstable": ["sloppy-imports"] }"#);

  let mut client: LspClient = context
    .new_lsp_command()
    .set_root_dir(temp_dir.clone())
    .build();
  client.initialize_default();

  temp_dir.join("a.js").write("export const foo: number;");

  let a_dts = source_file(temp_dir.join("a.d.ts"), "export const foo = 3;");
  let file = source_file(
    temp_dir.join("file.ts"),
    "import { foo } from './a.js';\nconsole.log(foo);",
  );
  let diagnostics = client.did_open_file(&file);
  // no other warnings because "a.js" exists
  assert_eq!(
    json!(diagnostics.all()),
    json!([{
      "range": {
        "start": { "line": 0, "character": 20 },
        "end": { "line": 0, "character": 28 }
      },
      "severity": 2,
      "code": "no-sloppy-imports",
      "source": "deno-lint",
      "message": "Sloppy imports are not allowed.",
      "data": [{
        "description": "Change the extension to '.d.ts'.",
        "changes": [{
          "range": {
            "start": { "line": 0, "character": 20 },
            "end": { "line": 0, "character": 28 },
          },
         "new_text": "'./a.d.ts'"
        }]
      }]
    }])
  );

  let diagnostics = client.did_open_file(&a_dts);
  assert_eq!(json!(diagnostics.for_file(&a_dts.uri())), json!([]));

  let response = client.write_request(
    "textDocument/references",
    json!({
      "textDocument": a_dts.identifier(),
      "position": a_dts.range_of("foo").start,
      "context": {
        "includeDeclaration": false
      }
    }),
  );
  assert_json_subset(
    response,
    json!([
      {
        "uri": file.uri(),
        // the import
        "range": file.range_of("foo"),
      },
      {
        "uri": file.uri(),
        // the usage
        "range": file.range_of_nth(1, "foo"),
      }
    ]),
  );
}

#[test]
fn sloppy_imports_not_enabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let temp_dir = temp_dir.path();
  temp_dir.join("deno.json").write(r#"{}"#);
  // The enhanced, more helpful error message is only available
  // when the file exists on the file system at the moment because
  // it's a little more complicated to hook it up otherwise.
  temp_dir.join("a.ts").write("export class A {}");
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_root_uri(temp_dir.uri_dir());
  });
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.join("file.ts").uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from './a';\nconsole.log(a)\n",
    },
  }));
  assert_eq!(
    diagnostics.messages_with_source("deno"),
    lsp::PublishDiagnosticsParams {
      uri: temp_dir.join("file.ts").uri_file(),
      diagnostics: vec![lsp::Diagnostic {
        range: lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 19
          },
          end: lsp::Position {
            line: 0,
            character: 24
          }
        },
        severity: Some(lsp::DiagnosticSeverity::ERROR),
        code: Some(lsp::NumberOrString::String("no-local".to_string())),
        source: Some("deno".to_string()),
        message: format!(
          "Unable to load a local module: {}\nMaybe add a '.ts' extension.",
          temp_dir.join("a").uri_file(),
        ),
        data: Some(json!({
          "specifier": temp_dir.join("a").uri_file(),
          "to": temp_dir.join("a.ts").uri_file(),
          "message": "Add a '.ts' extension.",
        })),
        ..Default::default()
      }],
      version: Some(1),
    }
  );
  let res = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": temp_dir.join("file.ts").uri_file()
      },
      "range": {
        "start": { "line": 0, "character": 19 },
        "end": { "line": 0, "character": 24 }
      },
      "context": {
        "diagnostics": [{
          "range": {
            "start": { "line": 0, "character": 19 },
            "end": { "line": 0, "character": 24 }
          },
          "severity": 3,
          "code": "no-local",
          "source": "deno",
          "message": format!(
            "Unable to load a local module: {}\nMaybe add a '.ts' extension.",
            temp_dir.join("a").uri_file(),
          ),
          "data": {
            "specifier": temp_dir.join("a").uri_file(),
            "to": temp_dir.join("a.ts").uri_file(),
            "message": "Add a '.ts' extension.",
          },
        }],
        "only": ["quickfix"]
      }
    }),
  );
  assert_eq!(
    res,
    json!([{
      "title": "Add a '.ts' extension.",
      "kind": "quickfix",
      "diagnostics": [{
        "range": {
          "start": { "line": 0, "character": 19 },
          "end": { "line": 0, "character": 24 }
        },
        "severity": 3,
        "code": "no-local",
        "source": "deno",
        "message": format!(
          "Unable to load a local module: {}\nMaybe add a '.ts' extension.",
          temp_dir.join("a").uri_file(),
        ),
        "data": {
          "specifier": temp_dir.join("a").uri_file(),
          "to": temp_dir.join("a.ts").uri_file(),
          "message": "Add a '.ts' extension.",
        },
      }],
      "edit": {
        "changes": {
          temp_dir.join("file.ts").uri_file(): [{
            "range": {
              "start": { "line": 0, "character": 19 },
              "end": { "line": 0, "character": 24 }
            },
            "newText": "\"./a.ts\""
          }]
        }
      }
    }])
  );
  client.shutdown();
}

// Regression test for https://github.com/denoland/deno/issues/24457.
#[test]
fn lsp_byonm_js_import_resolves_to_dts() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .add_npm_env_vars()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    json!({
      "unstable": ["byonm"],
    })
    .to_string(),
  );
  temp_dir.write(
    "package.json",
    json!({
      "dependencies": {
        "postcss": "*",
      },
    })
    .to_string(),
  );
  context.run_npm("install");
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": temp_dir.uri().join("node_modules/postcss/lib/comment.d.ts").unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": temp_dir.path().join("node_modules/postcss/lib/comment.d.ts").read_to_string(),
    }
  }));
  assert_eq!(json!(diagnostics.all()), json!([]));
  client.shutdown();
}

#[test]
fn decorators_tc39() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", r#"{}"#);

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let uri = temp_dir.uri().join("main.ts").unwrap();

  let diagnostics = client
    .did_open(json!({
      "textDocument": {
        "uri": uri,
        "languageId": "typescript",
        "version": 1,
        "text": r#"// deno-lint-ignore no-explicit-any
function logged(value: any, { kind, name }: { kind: string; name: string }) {
  if (kind === "method") {
    return function (...args: unknown[]) {
      console.log(`starting ${name} with arguments ${args.join(", ")}`);
      // @ts-ignore this has implicit any type
      const ret = value.call(this, ...args);
      console.log(`ending ${name}`);
      return ret;
    };
  }
}

class C {
  @logged
  m(arg: number) {
    console.log("C.m", arg);
  }
}

new C().m(1);
"#
      }
    }))
    .all();

  assert_eq!(diagnostics.len(), 0);

  client.shutdown();
}

#[test]
fn decorators_ts() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    r#"{ "compilerOptions": { "experimentalDecorators": true } }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let uri = temp_dir.uri().join("main.ts").unwrap();

  let diagnostics = client
    .did_open(json!({
      "textDocument": {
        "uri": uri,
        "languageId": "typescript",
        "version": 1,
        "text": r#"// deno-lint-ignore-file
function a() {
  console.log("@A evaluated");
  return function (
    _target: any,
    _propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    console.log("@A called");
    const fn = descriptor.value;
    descriptor.value = function () {
      console.log("fn() called from @A");
      fn();
    };
  };
}

class C {
  @a()
  static test() {
    console.log("C.test() called");
  }
}

C.test();
"#
      }
    }))
    .all();

  assert_eq!(json!(diagnostics), json!([]));

  client.shutdown();
}

#[test]
fn lsp_uses_lockfile_for_npm_initialization() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "{}");
  // use two npm packages here
  temp_dir.write("main.ts", "import 'npm:@denotest/esm-basic'; import 'npm:@denotest/cjs-default-export';");
  context
    .new_command()
    .args("run main.ts")
    .run()
    .skip_output_check();
  // remove one of the npm packages and let the other one be found via the lockfile
  temp_dir.write("main.ts", "import 'npm:@denotest/esm-basic';");
  assert!(temp_dir.path().join("deno.lock").exists());
  let mut client = context
    .new_lsp_command()
    .capture_stderr()
    .log_debug()
    .build();
  client.initialize_default();
  let mut skipping_count = 0;
  client.wait_until_stderr_line(|line| {
    if line.contains("Skipping npm resolution.") {
      skipping_count += 1;
    }
    assert!(!line.contains("Running npm resolution."), "Line: {}", line);
    line.contains("Server ready.")
  });
  assert_eq!(skipping_count, 2);
  client.shutdown();
}

#[test]
fn lsp_cjs_internal_types_default_export() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .add_npm_env_vars()
    .env("DENO_FUTURE", "1")
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", r#"{}"#);
  temp_dir.write(
    "package.json",
    r#"{
  "dependencies": {
    "@denotest/cjs-internal-types-default-export": "*"
  }
}"#,
  );
  context.run_npm("install");

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  // this was previously being resolved as ESM and not correctly as CJS
  let node_modules_index_d_ts = temp_dir.path().join(
    "node_modules/@denotest/cjs-internal-types-default-export/index.d.ts",
  );
  client.did_open(json!({
    "textDocument": {
      "uri": node_modules_index_d_ts.uri_file(),
      "languageId": "typescript",
      "version": 1,
      "text": node_modules_index_d_ts.read_to_string(),
    }
  }));
  let main_url = temp_dir.path().join("main.ts").uri_file();
  let diagnostics = client.did_open(
    json!({
      "textDocument": {
        "uri": main_url,
        "languageId": "typescript",
        "version": 1,
        "text": "import * as mod from '@denotest/cjs-internal-types-default-export';\nmod.add(1, 2);",
      }
    }),
  );
  // previously, diagnostic about `add` not being callable
  assert_eq!(json!(diagnostics.all()), json!([]));
}

#[test]
fn lsp_cjs_import_dual() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .add_npm_env_vars()
    .env("DENO_FUTURE", "1")
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", r#"{}"#);
  temp_dir.write(
    "package.json",
    r#"{
  "dependencies": {
    "@denotest/cjs-import-dual": "1"
  }
}"#,
  );
  context.run_npm("install");

  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  let main_url = temp_dir.path().join("main.ts").uri_file();
  let diagnostics = client.did_open(
    json!({
      "textDocument": {
        "uri": main_url,
        "languageId": "typescript",
        "version": 1,
        // getKind() should resolve as "cjs" and cause a type checker error
        "text": "import { getKind } from 'npm:@denotest/cjs-import-dual@1';\nconst kind: 'esm' = getKind(); console.log(kind);",
      }
    }),
  );
  assert_eq!(
    json!(diagnostics.all()),
    json!([{
      "range": {
          "start": { "line": 1, "character": 6, },
          "end": { "line": 1, "character": 10, },
      },
      "severity": 1,
      "code": 2322,
      "source": "deno-ts",
      "message": "Type '\"cjs\"' is not assignable to type '\"esm\"'.",
    }])
  );
}

#[test]
fn lsp_ts_code_fix_any_param() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let src = "export function foo(param) { console.log(param); }";

  let param_def = range_of("param", src);

  let main_url = temp_dir.path().join("main.ts").uri_file();
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": main_url,
      "languageId": "typescript",
      "version": 1,
      "text": src,
    }
  }));
  // make sure the "implicit any type" diagnostic is there for "param"
  assert_json_subset(
    json!(diagnostics.all()),
    json!([{
      "range": param_def,
      "code": 7006,
      "message": "Parameter 'param' implicitly has an 'any' type."
    }]),
  );

  // response is array of fixes
  let response = client.write_request(
    "textDocument/codeAction",
    json!({
      "textDocument": {
        "uri": main_url,
      },
      "range": lsp::Range {
        start: param_def.end,
        ..param_def
      },
      "context": {
        "diagnostics": diagnostics.all(),
      }
    }),
  );
  let fixes = response.as_array().unwrap();

  // we're looking for the quick fix that pertains to our diagnostic,
  // specifically the "Infer parameter types from usage" fix
  for fix in fixes {
    let Some(diags) = fix.get("diagnostics") else {
      continue;
    };
    let Some(fix_title) = fix.get("title").and_then(|s| s.as_str()) else {
      continue;
    };
    if diags == &json!(diagnostics.all())
      && fix_title == "Infer parameter types from usage"
    {
      // found it!
      return;
    }
  }

  panic!("failed to find 'Infer parameter types from usage' fix in fixes: {fixes:#?}");
}

#[test]
fn lsp_semantic_token_caching() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();

  let mut client: LspClient = context
    .new_lsp_command()
    .collect_perf()
    .set_root_dir(temp_dir.clone())
    .build();
  client.initialize_default();

  let a = source_file(
    temp_dir.join("a.ts"),
    r#"
    export const a = 1;
    export const b = 2;
    export const bar = () => "bar";
    function foo(fun: (number, number, number) => number, c: number) {
      const double = (x) => x * 2;
      return fun(double(a), b, c);
    }"#,
  );

  client.did_open_file(&a);

  // requesting a range won't cache the tokens, so this will
  // be computed
  let res = client.write_request(
    "textDocument/semanticTokens/range",
    json!({
      "textDocument": a.identifier(),
      "range": {
        "start": a.range_of("const bar").start,
        "end": a.range_of("}").end,
      }
    }),
  );

  assert_eq!(
    client
      .perf_wait_for_measure("lsp.semantic_tokens_range")
      .measure_count("tsc.request.getEncodedSemanticClassifications"),
    1,
  );

  // requesting for the full doc should compute and cache the tokens
  let _full = client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": a.identifier(),
    }),
  );

  assert_eq!(
    client
      .perf_wait_for_measure("lsp.semantic_tokens_full")
      .measure_count("tsc.request.getEncodedSemanticClassifications"),
    2,
  );

  // use the cached tokens
  let res_cached = client.write_request(
    "textDocument/semanticTokens/range",
    json!({
      "textDocument": a.identifier(),
      "range": {
        "start": a.range_of("const bar").start,
        "end": a.range_of("}").end,
      }
    }),
  );

  // make sure we actually used the cache
  assert_eq!(
    client
      .perf_wait_for_measure("lsp.semantic_tokens_range")
      .measure_count("tsc.request.getEncodedSemanticClassifications"),
    2,
  );

  assert_eq!(res, res_cached);
}

#[test]
fn lsp_jsdoc_named_example() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  let mut client = context
    .new_lsp_command()
    .set_root_dir(temp_dir.clone())
    .build();
  client.initialize_default();

  let main = source_file(
    temp_dir.join("main.ts"),
    r#"
    /**
     * @example Example1
     * ```ts
     * foo();
     * ```
     */
    export function foo(): number {
      return 1;
    }
    "#,
  );

  let diagnostics = client.did_open_file(&main);
  assert_eq!(diagnostics.all().len(), 0);

  let hover = client.write_request(
    "textDocument/hover",
    json!({
      "textDocument": main.identifier(),
      "position": main.range_of_nth(1, "foo").start,
    }),
  );

  assert_json_subset(
    hover,
    json!({
      "contents": [
        {
          "language": "typescript",
          "value": "function foo(): number"
        },
        "",
        // The example name `Example1` should not be enclosed in backticks
        "\n\n*@example*  \nExample1\n```ts\nfoo();\n```"
      ]
    }),
  );
}
