// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use pretty_assertions::assert_eq;
use std::fs;
use std::process::Stdio;
use test_util::deno_cmd_with_deno_dir;
use test_util::env_vars_for_npm_tests;
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

  assert_eq!(diagnostics.viewed().len(), 0);

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
    builder.set_config("types.tsconfig.json");
  });

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": Url::from_file_path(temp_dir.path().join("test.ts")).unwrap(),
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(a);\n"
    }
  }));

  assert_eq!(diagnostics.viewed().len(), 0);

  client.shutdown();
}

#[test]
fn lsp_tsconfig_bad_config_path() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder
      .set_config("bad_tsconfig.json")
      .set_maybe_root_uri(None);
  });
  let (method, maybe_params) = client.read_notification();
  assert_eq!(method, "window/showMessage");
  assert_eq!(maybe_params, Some(lsp::ShowMessageParams {
    typ: lsp::MessageType::WARNING,
    message: "The path to the configuration file (\"bad_tsconfig.json\") is not resolvable.".to_string()
  }));
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Deno.args);\n"
    }
  }));
  assert_eq!(diagnostics.viewed().len(), 0);
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

  assert_eq!(diagnostics.viewed().len(), 0);

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

  let uri = Url::from_file_path(temp_dir.path().join("a.ts")).unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
    }
  }));

  assert_eq!(diagnostics.viewed().len(), 0);

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
fn lsp_import_map_data_url() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("data:application/json;utf8,{\"imports\": { \"example\": \"https://deno.land/x/example/mod.ts\" }}");
  });
  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import example from \"example\";\n"
    }
  }));

  // This indicates that the import map is applied correctly.
  assert!(diagnostics.viewed().iter().any(|diagnostic| diagnostic.code
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

  assert_eq!(diagnostics.viewed().len(), 0);

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

  assert_eq!(diagnostics.viewed().len(), 0);

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
        "detail": "deno test"
      }, {
        "name": "some:test",
        "detail": "deno bundle mod.ts"
      }
    ])
  );
}

#[test]
fn lsp_import_assertions() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize(|builder| {
    builder.set_import_map("data:application/json;utf8,{\"imports\": { \"example\": \"https://deno.land/x/example/mod.ts\" }}");
  });

  client.did_open_with_config(
    json!({
      "textDocument": {
        "uri": "file:///a/test.json",
        "languageId": "json",
        "version": 1,
        "text": "{\"a\":1}"
      }
    }),
    json!([{
      "enable": true,
      "codeLens": {
        "test": true
      }
    }]),
  );

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
        .with_file_and_source("file:///a/a.ts", "deno")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": { "line": 0, "character": 14 },
          "end": { "line": 0, "character": 27 }
        },
        "severity": 1,
        "code": "no-assert-type",
        "source": "deno",
        "message": "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement."
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
            "code": "no-assert-type",
            "source": "deno",
            "message": "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement."
          }],
          "only": ["quickfix"]
        }
      }),
    )
    ;
  assert_eq!(
    res,
    json!([{
      "title": "Insert import assertion.",
      "kind": "quickfix",
      "diagnostics": [
        {
          "range": {
            "start": { "line": 0, "character": 14 },
            "end": { "line": 0, "character": 27 }
          },
          "severity": 1,
          "code": "no-assert-type",
          "source": "deno",
          "message": "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement."
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
              "newText": " assert { type: \"json\" }"
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
        }
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
          "kind": 9,
          "detail": "(import map)",
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
        "Returns the script arguments to the program.\n\nGive the following command line invocation of Deno:\n\n```sh\ndeno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd\n```\n\nThen `Deno.args` will contain:\n\n```ts\n[ \"/etc/passwd\" ]\n```\n\nIf you are looking for a structured way to parse arguments, there is the\n[`std/flags`](https://deno.land/std/flags) module as part of the Deno\nstandard library.",
        "\n\n*@category* - Runtime Environment",
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
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Date.now());\n"
    }
  }));
  client.write_request(
    "textDocument/definition",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
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
        "Enables basic storage and retrieval of dates and times."
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
  client.did_open_with_config(
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
    json!([{ "enable": false }]),
  );

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
}

#[test]
fn lsp_workspace_enable_paths() {
  fn run_test(use_trailing_slash: bool) {
    let context = TestContextBuilder::new().use_temp_cwd().build();
    let temp_dir = context.temp_dir();
    temp_dir.create_dir_all("worker");
    temp_dir.write("worker/shared.ts", "export const a = 1");
    temp_dir.write("worker/other.ts", "import { a } from './shared.ts';\na;");

    let root_specifier = temp_dir.uri();

    let mut client = context.new_lsp_command().build();
    client.initialize_with_config(
      |builder| {
        builder
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
          }])
          .set_deno_enable(false);
      },
      json!([{
        "enable": false,
        "enablePaths": ["./worker"],
      }]),
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
fn lsp_hover_unstable_disabled() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Deno.dlopen);\n"
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
          "value": "any"
        }
      ],
      "range": {
        "start": { "line": 0, "character": 17 },
        "end": { "line": 0, "character": 23 }
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
    builder.set_unstable(true);
  });
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "console.log(Deno.ppid);\n"
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
      "contents":[
        {
          "language":"typescript",
          "value":"const Deno.ppid: number"
        },
        "The process ID of parent process of this instance of the Deno CLI.\n\n```ts\nconsole.log(Deno.ppid);\n```",
        "\n\n*@category* - Runtime Environment",
      ],
      "range":{
        "start":{ "line":0, "character":17 },
        "end":{ "line":0, "character":21 }
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
              // after the deno emoiji is character index 15
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
        "text": "import * as a from \"http://127.0.0.1:4545/xTypeScriptTypes.js\";\n// @deno-types=\"http://127.0.0.1:4545/type_definitions/foo.d.ts\"\nimport * as b from \"http://127.0.0.1:4545/type_definitions/foo.js\";\nimport * as c from \"http://127.0.0.1:4545/subdir/type_reference.js\";\nimport * as d from \"http://127.0.0.1:4545/subdir/mod1.ts\";\nimport * as e from \"data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=\";\nimport * as f from \"./file_01.ts\";\nimport * as g from \"http://localhost:4545/x/a/mod.ts\";\n\nconsole.log(a, b, c, d, e, f, g);\n"
      }
    }),
  );
  client.write_request(
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [],
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
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [
        {
          "uri": "http://127.0.0.1:4545/xTypeScriptTypes.js",
        }
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
        "JSDoc [hello](file:///a/file.ts#L1,10) and [`b`](file:///a/file.ts#L5,7)"
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
        "text": "interface IFoo {\n  foo(): boolean;\n}\n\nclass Bar implements IFoo {\n  constructor(public x: number) { }\n  foo() { return true; }\n  /** @deprecated */\n  baz() { return false; }\n  get value(): number { return 0; }\n  set value(newVavlue: number) { return; }\n  static staticBar = new Bar(0);\n  private static getStaticBar() { return Bar.staticBar; }\n}\n\nenum Values { value1, value2 }\n\nvar bar: IFoo = new Bar(3);"
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
fn lsp_code_lens() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
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
        "command": "deno.showReferences",
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
        "command": "deno.showReferences",
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
fn lsp_code_lens_impl() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
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
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
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
        "command": "deno.showReferences",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
        "command": "deno.test",
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
  client
    .did_open_with_config(
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "languageId": "typescript",
          "version": 1,
          "text": "const { test } = Deno;\nconst { test: test2 } = Deno;\nconst test3 = Deno.test;\n\nDeno.test(\"test a\", () => {});\nDeno.test({\n  name: \"test b\",\n  fn() {},\n});\ntest({\n  name: \"test c\",\n  fn() {},\n});\ntest(\"test d\", () => {});\ntest2({\n  name: \"test e\",\n  fn() {},\n});\ntest2(\"test f\", () => {});\ntest3({\n  name: \"test g\",\n  fn() {},\n});\ntest3(\"test h\", () => {});\n"
        }
      }),
      // diable test code lens
      json!([{
        "enable": true,
        "codeLens": {
          "test": false
        }
      }]),
    );
  let res = client.write_request(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  assert_eq!(res, json!([]));
  client.shutdown();
}

#[test]
fn lsp_code_lens_non_doc_nav_tree() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
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
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
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
        "start": { "line": 4, "character": 6 },
        "end": { "line": 4, "character": 7 }
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
      "text": r#"export const a = 1;\nconst b = 2;"#
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
    .write_request(      "codeAction/resolve",
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
    )
    ;
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
    diagnostics.with_source("deno"),
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
        "message": "Uncached or missing remote URL: \"https://deno.land/x/a/mod.ts\".",
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
        "arguments": [["https://deno.land/x/a/mod.ts"]]
      }
    }])
  );
  client.shutdown();
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
    diagnostics.with_source("deno"),
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
        "message": "Uncached or missing npm package: \"chalk\".",
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
          "message": "Uncached or missing npm package: \"chalk\".",
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
        "message": "Uncached or missing npm package: \"chalk\".",
        "data": { "specifier": "npm:chalk" }
      }],
      "command": {
        "title": "",
        "command": "deno.cache",
        "arguments": [["npm:chalk"]]
      }
    }])
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
  client.did_open_raw(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "javascript",
      "version": 1,
      "text": large_file_text,
    }
  }));
  client.handle_configuration_request(json!([{ "enable": true }]));
  client.write_request(
    "textDocument/semanticTokens/full",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts"
      }
    }),
  );
  client.read_diagnostics();
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
      "detail": "const Deno.build: {\n    target: string;\n    arch: \"x86_64\" | \"aarch64\";\n    os: \"darwin\" | \"linux\" | \"windows\" | \"freebsd\" | \"netbsd\" | \"aix\" | \"solaris\" | \"illumos\";\n    vendor: string;\n    env?: string | undefined;\n}",
      "documentation": {
        "kind": "markdown",
        "value": "Information related to the build of the current Deno runtime.\n\nUsers are discouraged from code branching based on this information, as\nassumptions about what is available in what build environment might change\nover time. Developers should specifically sniff out the features they\nintend to use.\n\nThe intended use for the information is for logging and debugging purposes.\n\n*@category* - Runtime Environment"
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
      "uri": "file:///a/b.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const foo = \"foo\";\n",
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
  let item = list.items.iter().find(|item| item.label == "foo");
  if item.is_none() {
    panic!("completions items missing 'foo' symbol");
  }

  let req = json!({
    "label": "foo",
    "kind": 6,
    "sortText": "￿16",
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
        "name": "foo",
        "source": "./b.ts",
        "data": {
          "exportName": "foo",
          "exportMapKey": "foo|6845|file:///a/b",
          "moduleSpecifier": "./b.ts",
          "fileName": "file:///a/b.ts"
        },
        "useCodeSnippet": false
      }
    }
  });
  assert_eq!(serde_json::to_value(item.unwrap()).unwrap(), req);

  let res = client.write_request("completionItem/resolve", req);
  assert_eq!(
    res,
    json!({
      "label": "foo",
      "kind": 6,
      "detail": "const foo: \"foo\"",
      "documentation": {
        "kind": "markdown",
        "value": ""
      },
      "sortText": "￿16",
      "additionalTextEdits": [
        {
          "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
          },
          "newText": "import { foo } from \"./b.ts\";\n\n"
        }
      ]
    })
  );
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
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [
        {
          "uri": "npm:@denotest/cjs-default-export",
        }, {
          "uri": "npm:chalk",
        }
      ]
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
  client.read_diagnostics();

  let list = client.get_completion_list(
    "file:///a/file.ts",
    (2, 11),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 3);
  assert!(list.items.iter().any(|i| i.label == "default"));
  assert!(list.items.iter().any(|i| i.label == "MyClass"));

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
  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  // create other.ts, which re-exports an npm specifier
  client.deno_dir().write(
    "other.ts",
    "export { default as chalk } from 'npm:chalk@5';",
  );

  // cache the other.ts file to the DENO_DIR
  let deno = deno_cmd_with_deno_dir(client.deno_dir())
    .current_dir(client.deno_dir().path())
    .arg("cache")
    .arg("--quiet")
    .arg("other.ts")
    .envs(env_vars_for_npm_tests())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
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
  let main_url =
    ModuleSpecifier::from_file_path(client.deno_dir().path().join("main.ts"))
      .unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": main_url,
      "languageId": "typescript",
      "version": 1,
      "text": "import { chalk } from './other.ts';\n\n",
    }
  }));

  client.write_notification(
    "textDocument/didChange",
    json!({
      "textDocument": {
        "uri": main_url,
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
    main_url,
    (2, 6),
    json!({
      "triggerKind": 2,
      "triggerCharacter": "."
    }),
  );
  assert!(!list.is_incomplete);
  assert_eq!(list.items.len(), 63);
  assert!(list.items.iter().any(|i| i.label == "ansi256"));
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
    .with_file_and_source("file:///a/file.ts", "deno")
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
    .with_file_and_source("file:///a/file.ts", "deno")
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
    .with_file_and_source("file:///a/file.ts", "deno")
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
        "message": "Uncached or missing npm package: \"@types/node\"."
      }
    ])
  );

  client.write_request(
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [
        {
          "uri": "npm:@types/node",
        }
      ]
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
        "commitCharacters": ["\"", "'", "/"]
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
  assert_eq!(diagnostics.viewed().len(), 7);
  client.write_request(
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [],
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
  assert!(cache_path.join("gen").is_dir());
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
  let diagnostics = diagnostics.viewed();
  assert_eq!(diagnostics.len(), 7);
  client.write_request(
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [],
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
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [
        {
          "uri": "http://127.0.0.1:4545/x_deno_warning.js",
        }
      ],
    }),
  );
  let diagnostics = client.read_diagnostics();
  assert_eq!(
    diagnostics.with_source("deno"),
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
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.ts",
      },
      "uris": [
        {
          "uri": "http://127.0.0.1:4545/x_deno_warning.js",
        }
      ],
    }),
  );
  let diagnostics = client.read_diagnostics().with_source("deno").diagnostics;
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
    json!(diagnostics.0),
    json!([
      {
        "uri": "file:///a/file.ts",
        "diagnostics": [],
        "version": 1
      }, {
        "uri": "file:///a/file.ts",
        "diagnostics": [],
        "version": 1
      }, {
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
            "relatedInformation": [],
            "tags": [2]
          }
        ],
        "version": 1
      }
    ])
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
  assert_eq!(diagnostics.viewed().len(), 5);
  client.shutdown();
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
    json!(diagnostics.with_file_and_source("file:///a/file_02.ts", "deno-ts")),
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
  assert_eq!(diagnostics.viewed().len(), 0); // no diagnostics now

  client.shutdown();
  assert_eq!(client.queue_len(), 0);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAverage {
  pub name: String,
  pub count: u32,
  pub average_duration: u32,
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
      "did_open",
      "hover",
      "initialize",
      "op_load",
      "request",
      "testing_update",
      "update_cache",
      "update_diagnostics_deps",
      "update_diagnostics_lint",
      "update_diagnostics_ts",
      "update_import_map",
      "update_registries",
      "update_tsconfig",
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
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      // Also test out using a non-json file extension here.
      // What should matter is the language identifier.
      "uri": "file:///a/file.lock",
      "languageId": "json",
      "version": 1,
      "text": "{\"key\":\"value\"}"
    }
  }));

  let res = client.write_request(
    "textDocument/formatting",
    json!({
        "textDocument": {
          "uri": "file:///a/file.lock"
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
fn lsp_json_no_diagnostics() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
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
fn lsp_format_markdown() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let mut client = context.new_lsp_command().build();
  client.initialize_default();
  client.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.md",
      "languageId": "markdown",
      "version": 1,
      "text": "#   Hello World"
    }
  }));

  let res = client.write_request(
    "textDocument/formatting",
    json!({
      "textDocument": {
        "uri": "file:///a/file.md"
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

  client
    .did_open(
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
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
  client.did_open(json!({
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
  client.write_notification(
    "workspace/didChangeConfiguration",
    json!({
      "settings": {}
    }),
  );
  let request = json!([{
    "enable": true,
    "codeLens": {
      "implementations": true,
      "references": true
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
          "http://localhost:4545/": true
        }
      }
    },
    "unstable": false
  }]);
  // one for the workspace
  client.handle_configuration_request(request.clone());
  // one for the specifier
  client.handle_configuration_request(request);

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
  let res = client.write_request(
    "workspace/symbol",
    json!({
      "query": "field"
    }),
  );
  assert_eq!(
    res,
    json!([{
      "name": "fieldA",
      "kind": 8,
      "location": {
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 17 }
        }
      },
      "containerName": "A"
    }, {
      "name": "fieldB",
      "kind": 8,
      "location": {
        "uri": "file:///a/file.ts",
        "range": {
          "start": { "line": 2, "character": 2 },
          "end": { "line": 2, "character": 17 }
        }
      },
      "containerName": "A"
    }, {
      "name": "fieldC",
      "kind": 8,
      "location": {
        "uri": "file:///a/file_01.ts",
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 17 }
        }
      },
      "containerName": "B"
    }, {
      "name": "fieldD",
      "kind": 8,
      "location": {
        "uri": "file:///a/file_01.ts",
        "range": {
          "start": { "line": 2, "character": 2 },
          "end": { "line": 2, "character": 17 }
        }
      },
      "containerName": "B"
    }, {
      "name": "ClassFieldDecoratorContext",
      "kind": 11,
      "location": {
        "uri": "deno:/asset/lib.decorators.d.ts",
        "range": {
          "start": {
            "line": 331,
            "character": 0,
          },
          "end": {
            "line": 371,
            "character": 1,
          },
        },
      },
      "containerName": "",
    }])
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
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "// TODO: fixme\nexport async function non_camel_case() {\nconsole.log(\"finished!\")\n}"
    }
  }));
  let diagnostics = diagnostics.viewed();
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
  let diagnostics = diagnostics.viewed();
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
    "deno/cache",
    json!({
      "referrer": {
        "uri": "file:///a/file.tsx",
      },
      "uris": [{
        "uri": "http://127.0.0.1:4545/jsx/jsx-runtime",
      }],
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
  fn() {
    console.log("test a");
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
  assert!(test.steps.is_none());
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

  let (method, notification) = client.read_notification::<Value>();
  assert_eq!(method, "deno/testRunProgress");
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

  let (method, notification) = client.read_notification::<Value>();
  assert_eq!(method, "deno/testRunProgress");
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
  // synchronize the output pipes. Occassionally this zero width space
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

  let (method, notification) = client.read_notification::<Value>();
  assert_eq!(method, "deno/testRunProgress");
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

      let (method, notification) = client.read_notification::<Value>();
      assert_eq!(method, "deno/testRunProgress");
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
fn lsp_data_urls_with_jsx_compiler_option() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    r#"{ "compilerOptions": { "jsx": "react-jsx" } }"#,
  );

  let mut client = context.new_lsp_command().build();
  client.initialize_default();

  let uri = Url::from_file_path(temp_dir.path().join("main.ts")).unwrap();

  let diagnostics = client.did_open(json!({
    "textDocument": {
      "uri": uri,
      "languageId": "typescript",
      "version": 1,
      "text": "import a from \"data:application/typescript,export default 5;\";\na;"
    }
  })).viewed();

  // there will be a diagnostic about not having cached the data url
  assert_eq!(diagnostics.len(), 1);
  assert_eq!(
    diagnostics[0].code,
    Some(lsp::NumberOrString::String("no-cache-data".to_string()))
  );

  // so cache it
  client.write_request(
    "deno/cache",
    json!({
      "referrer": {
        "uri": uri,
      },
      "uris": [],
    }),
  );

  let res = client.write_request(
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
