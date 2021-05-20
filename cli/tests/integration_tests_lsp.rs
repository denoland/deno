// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use lspower::lsp;
use std::fs;
use tempfile::TempDir;
use test_util::deno_exe_path;
use test_util::http_server;
use test_util::lsp::LspClient;
use test_util::root_path;

fn load_fixture(path: &str) -> Value {
  let fixtures_path = root_path().join("cli/tests/lsp");
  let path = fixtures_path.join(path);
  let fixture_str = fs::read_to_string(path).unwrap();
  serde_json::from_str(&fixture_str).unwrap()
}

fn init(init_path: &str) -> LspClient {
  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe).unwrap();
  client
    .write_request::<_, _, Value>("initialize", load_fixture(init_path))
    .unwrap();
  client.write_notification("initialized", json!({})).unwrap();
  client
}

fn did_open<V>(client: &mut LspClient, params: V)
where
  V: Serialize,
{
  client
    .write_notification("textDocument/didOpen", params)
    .unwrap();

  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!({ "enable": true }))
    .unwrap();

  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
}

fn shutdown(client: &mut LspClient) {
  client
    .write_request::<_, _, Value>("shutdown", json!(null))
    .unwrap();
  client.write_notification("exit", json!(null)).unwrap();
}

#[test]
fn lsp_startup_shutdown() {
  let mut client = init("initialize_params.json");
  shutdown(&mut client);
}

#[test]
fn lsp_hover() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.args);\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 19
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const Deno.args: string[]"
        },
        "Returns the script arguments to the program. If for example we run a\nprogram:\n\ndeno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd\n\nThen `Deno.args` will contain:\n\n[ \"/etc/passwd\" ]"
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 17
        },
        "end": {
          "line": 0,
          "character": 21
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_asset() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );
  let (_, maybe_error) = client
    .write_request::<_, _, Value>(
      "textDocument/definition",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 14
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  let (_, maybe_error) = client
    .write_request::<_, _, Value>(
      "deno/virtualTextDocument",
      json!({
        "textDocument": {
          "uri": "deno:/asset//lib.deno.shared_globals.d.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "deno:/asset//lib.es2015.symbol.wellknown.d.ts"
        },
        "position": {
          "line": 109,
          "character": 13
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "interface Date",
        },
        "Enables basic storage and retrieval of dates and times."
      ],
      "range": {
        "start": {
          "line": 109,
          "character": 10,
        },
        "end": {
          "line": 109,
          "character": 14,
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_disabled() {
  let mut client = init("initialize_params_disabled.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "languageId": "typescript",
          "version": 1,
          "text": "console.log(Date.now());\n"
        }
      }),
    )
    .unwrap();

  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!({ "enable": false }))
    .unwrap();

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 19
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));
  shutdown(&mut client);
}

#[test]
fn lsp_hover_unstable_disabled() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.openPlugin);\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 19
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "any"
        }
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 17
        },
        "end": {
          "line": 0,
          "character": 27
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_unstable_enabled() {
  let mut client = init("initialize_params_unstable.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.openPlugin);\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 19
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents":[
        {
          "language":"typescript",
          "value":"function Deno.openPlugin(filename: string): number"
        },
        "**UNSTABLE**: new API, yet to be vetted.\n\nOpen and initialize a plugin.\n\n```ts\nimport { assert } from \"https://deno.land/std/testing/asserts.ts\";\nconst rid = Deno.openPlugin(\"./path/to/some/plugin.so\");\n\n// The Deno.core namespace is needed to interact with plugins, but this is\n// internal so we use ts-ignore to skip type checking these calls.\n// @ts-ignore\nconst { op_test_sync, op_test_async } = Deno.core.ops();\n\nassert(op_test_sync);\nassert(op_test_async);\n\n// @ts-ignore\nconst result = Deno.core.opSync(\"op_test_sync\");\n\n// @ts-ignore\nconst result = await Deno.core.opAsync(\"op_test_sync\");\n```\n\nRequires `allow-plugin` permission.\n\nThe plugin system is not stable and will change in the future, hence the\nlack of docs. For now take a look at the example\nhttps://github.com/denoland/deno/tree/main/test_plugin"
      ],
      "range":{
        "start":{
          "line":0,
          "character":17
        },
        "end":{
          "line":0,
          "character":27
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_change_mbc() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "const a = `编写软件很难`;\nconst b = `👍🦕😃`;\nconsole.log(a, b);\n"
      }
    }),
  );
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 1,
                "character": 11
              },
              "end": {
                "line": 1,
                "character": 13
              }
            },
            "text": ""
          }
        ]
      }),
    )
    .unwrap();
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 2,
          "character": 14
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const b: \"😃\"",
        },
        "",
      ],
      "range": {
        "start": {
          "line": 2,
          "character": 13,
        },
        "end": {
          "line": 2,
          "character": 14,
        },
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_closed_document() {
  let temp_dir = TempDir::new()
    .expect("could not create temp dir")
    .into_path();
  let a_path = temp_dir.join("a.ts");
  fs::write(a_path, r#"export const a = "a";"#).expect("could not write file");
  let b_path = temp_dir.join("b.ts");
  fs::write(&b_path, r#"export * from "./a.ts";"#)
    .expect("could not write file");
  let b_specifier =
    Url::from_file_path(b_path).expect("could not convert path");
  let c_path = temp_dir.join("c.ts");
  fs::write(&c_path, "import { a } from \"./b.ts\";\nconsole.log(a);\n")
    .expect("could not write file");
  let c_specifier =
    Url::from_file_path(c_path).expect("could not convert path");

  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": b_specifier,
          "languageId": "typescript",
          "version": 1,
          "text": r#"export * from "./a.ts";"#
        }
      }),
    )
    .unwrap();
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!({ "enable": true }))
    .unwrap();

  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": c_specifier,
          "languageId": "typescript",
          "version": 1,
          "text": "import { a } from \"./b.ts\";\nconsole.log(a);\n",
        }
      }),
    )
    .unwrap();
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!({ "enable": true }))
    .unwrap();

  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": c_specifier,
        },
        "position": {
          "line": 0,
          "character": 10
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(alias) const a: \"a\"\nimport a"
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 9
        },
        "end": {
          "line": 0,
          "character": 10
        }
      }
    }))
  );
  client
    .write_notification(
      "textDocument/didClose",
      json!({
        "textDocument": {
          "uri": b_specifier,
        }
      }),
    )
    .unwrap();
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": c_specifier,
        },
        "position": {
          "line": 0,
          "character": 10
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "(alias) const a: \"a\"\nimport a"
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 9
        },
        "end": {
          "line": 0,
          "character": 10
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_call_hierarchy() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "function foo() {\n  return false;\n}\n\nclass Bar {\n  baz() {\n    return foo();\n  }\n}\n\nfunction main() {\n  const bar = new Bar();\n  bar.baz();\n}\n\nmain();"
      }
    }),
  );
  let (maybe_res, maybe_error) = client
    .write_request(
      "textDocument/prepareCallHierarchy",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 5,
          "character": 3
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("prepare_call_hierarchy_response.json"))
  );
  let (maybe_res, maybe_error) = client
    .write_request(
      "callHierarchy/incomingCalls",
      load_fixture("incoming_calls_params.json"),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("incoming_calls_response.json"))
  );
  let (maybe_res, maybe_error) = client
    .write_request(
      "callHierarchy/outgoingCalls",
      load_fixture("outgoing_calls_params.json"),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("outgoing_calls_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_format_mbc() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "const bar = '👍🇺🇸😃'\nconsole.log('hello deno')\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
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
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!(load_fixture("formatting_mbc_response.json")))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_large_doc_changes() {
  let mut client = init("initialize_params.json");
  did_open(&mut client, load_fixture("did_open_params_large.json"));
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 444,
                "character": 11
              },
              "end": {
                "line": 444,
                "character": 14
              }
            },
            "text": "+++"
          }
        ]
      }),
    )
    .unwrap();
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 445,
                "character": 4
              },
              "end": {
                "line": 445,
                "character": 4
              }
            },
            "text": "// "
          }
        ]
      }),
    )
    .unwrap();
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 477,
                "character": 4
              },
              "end": {
                "line": 477,
                "character": 9
              }
            },
            "text": "error"
          }
        ]
      }),
    )
    .unwrap();
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 421,
          "character": 30
        }
      }),
    )
    .unwrap();
  assert!(maybe_res.is_some());
  assert!(maybe_err.is_none());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 444,
          "character": 6
        }
      }),
    )
    .unwrap();
  assert!(maybe_res.is_some());
  assert!(maybe_err.is_none());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 461,
          "character": 34
        }
      }),
    )
    .unwrap();
  assert!(maybe_res.is_some());
  assert!(maybe_err.is_none());
  shutdown(&mut client);

  assert!(client.duration().as_millis() <= 15000);
}

#[test]
fn lsp_document_symbol() {
  let mut client = init("initialize_params.json");
  did_open(&mut client, load_fixture("did_open_params_doc_symbol.json"));
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/documentSymbol",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("document_symbol_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_folding_range() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "// #region 1\n/*\n * Some comment\n */\nclass Foo {\n  bar(a, b) {\n    if (a === b) {\n      return true;\n    }\n    return false;\n  }\n}\n// #endregion"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/foldingRange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "startLine": 0,
        "endLine": 12,
        "kind": "region"
      },
      {
        "startLine": 1,
        "endLine": 3,
        "kind": "comment"
      },
      {
        "startLine": 4,
        "endLine": 10
      },
      {
        "startLine": 5,
        "endLine": 9
      },
      {
        "startLine": 6,
        "endLine": 7
      }
    ]))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_rename() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "let variable = 'a';\nconsole.log(variable);"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/rename",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 4
        },
        "newName": "variable_modified"
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(load_fixture("rename_response.json")));
  shutdown(&mut client);
}

#[test]
fn lsp_selection_range() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "class Foo {\n  bar(a, b) {\n    if (a === b) {\n      return true;\n    }\n    return false;\n  }\n}"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/selectionRange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "positions": [
          {
            "line": 2,
            "character": 8
          }
        ]
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("selection_range_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_semantic_tokens() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    load_fixture("did_open_params_semantic_tokens.json"),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/semanticTokens/full",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "data": [
        0, 5, 6, 1, 1, 0, 9, 6, 8, 9, 0, 8, 6, 8, 9, 2, 15, 3, 10, 5, 0, 4, 1,
        6, 1, 0, 12, 7, 2, 16, 1, 8, 1, 7, 41, 0, 4, 1, 6, 0, 0, 2, 5, 11, 16,
        1, 9, 1, 7, 40, 3, 10, 4, 2, 1, 1, 11, 1, 9, 9, 1, 2, 3, 11, 1, 3, 6, 3,
        0, 1, 0, 15, 4, 2, 0, 1, 30, 1, 6, 9, 1, 2, 3, 11,1, 1, 9, 9, 9, 3, 0,
        16, 3, 0, 0, 1, 17, 12, 11, 3, 0, 24, 3, 0, 0, 0, 4, 9, 9, 2
      ]
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/semanticTokens/range",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "range": {
          "start": {
            "line": 0,
            "character": 0
          },
          "end": {
            "line": 6,
            "character": 0
          }
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "data": [
        0, 5, 6, 1, 1, 0, 9, 6, 8, 9, 0, 8, 6, 8, 9, 2, 15, 3, 10, 5, 0, 4, 1,
        6, 1, 0, 12, 7, 2, 16, 1, 8, 1, 7, 41, 0, 4, 1, 6, 0, 0, 2, 5, 11, 16,
        1, 9, 1, 7, 40
      ]
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_lens() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "class A {\n  a = \"a\";\n\n  b() {\n    console.log(this.a);\n  }\n\n  c() {\n    this.a = \"c\";\n  }\n}\n\nconst a = new A();\na.b();\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeLens",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(load_fixture("code_lens_response.json")));
  let (maybe_res, maybe_err) = client
    .write_request(
      "codeLens/resolve",
      json!({
        "range": {
          "start": {
            "line": 0,
            "character": 6
          },
          "end": {
            "line": 0,
            "character": 7
          }
        },
        "data": {
          "specifier": "file:///a/file.ts",
          "source": "references"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_lens_resolve_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_lens_impl() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b(): void;\n}\n\nclass B implements A {\n  b() {\n    console.log(\"b\");\n  }\n}\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeLens",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_lens_response_impl.json"))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "codeLens/resolve",
      json!({
        "range": {
          "start": {
            "line": 0,
            "character": 10
          },
          "end": {
            "line": 0,
            "character": 11
          }
        },
        "data": {
          "specifier": "file:///a/file.ts",
          "source": "implementations"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_lens_resolve_response_impl.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_lens_non_doc_nav_tree() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/references",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 3
        },
        "context": {
          "includeDeclaration": true
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "deno/virtualTextDocument",
      json!({
        "textDocument": {
          "uri": "deno:/asset//lib.deno.shared_globals.d.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Vec<lsp::CodeLens>>(
      "textDocument/codeLens",
      json!({
        "textDocument": {
          "uri": "deno:/asset//lib.deno.shared_globals.d.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let res = maybe_res.unwrap();
  assert!(res.len() > 50);
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, lsp::CodeLens>(
      "codeLens/resolve",
      json!({
        "range": {
          "start": {
            "line": 416,
            "character": 12
          },
          "end": {
            "line": 416,
            "character": 19
          }
        },
        "data": {
          "specifier": "asset:///lib.deno.shared_globals.d.ts",
          "source": "references"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  shutdown(&mut client);
}

#[test]
fn lsp_signature_help() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "/**\n * Adds two numbers.\n * @param a This is a first number.\n * @param b This is a second number.\n */\nfunction add(a: number, b: number) {\n  return a + b;\n}\n\nadd("
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/signatureHelp",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "character": 4,
          "line": 9
        },
        "context": {
          "triggerKind": 2,
          "triggerCharacter": "(",
          "isRetrigger": false
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "signatures": [
        {
          "label": "add(a: number, b: number): number",
          "documentation": "Adds two numbers.",
          "parameters": [
            {
              "label": "a: number",
              "documentation": "This is a first number."
            },
            {
              "label": "b: number",
              "documentation": "This is a second number."
            }
          ]
        }
      ],
      "activeSignature": 0,
      "activeParameter": 0
    }))
  );
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 9,
                "character": 4
              },
              "end": {
                "line": 9,
                "character": 4
              }
            },
            "text": "123, "
          }
        ]
      }),
    )
    .unwrap();
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/signatureHelp",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "character": 8,
          "line": 9
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "signatures": [
        {
          "label": "add(a: number, b: number): number",
          "documentation": "Adds two numbers.",
          "parameters": [
            {
              "label": "a: number",
              "documentation": "This is a first number."
            },
            {
              "label": "b: number",
              "documentation": "This is a second number."
            }
          ]
        }
      ],
      "activeSignature": 0,
      "activeParameter": 1
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_actions() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export function a(): void {\n  await Promise.resolve(\"a\");\n}\n\nexport function b(): void {\n  await Promise.resolve(\"b\");\n}\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_params.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(load_fixture("code_action_response.json")));
  let (maybe_res, maybe_err) = client
    .write_request(
      "codeAction/resolve",
      load_fixture("code_action_resolve_params.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_resolve_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_actions_deno_cache() {
  let mut client = init("initialize_params.json");
  client
    .write_notification("textDocument/didOpen", json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"https://deno.land/x/a/mod.ts\";\n\nconsole.log(a);\n"
      }
    }))
    .unwrap();
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, params) = client.read_notification().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  assert_eq!(params, Some(load_fixture("diagnostics_deno_deps.json")));

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_params_cache.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_response_cache.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_actions_deadlock() {
  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      load_fixture("did_open_params_large.json"),
    )
    .unwrap();
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!({ "enable": true }))
    .unwrap();
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/semanticTokens/full",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  for _ in 0..3 {
    let (method, _) = client.read_notification::<Value>().unwrap();
    assert_eq!(method, "textDocument/publishDiagnostics");
  }
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 444,
                "character": 11
              },
              "end": {
                "line": 444,
                "character": 14
              }
            },
            "text": "+++"
          }
        ]
      }),
    )
    .unwrap();
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 445,
                "character": 4
              },
              "end": {
                "line": 445,
                "character": 4
              }
            },
            "text": "// "
          }
        ]
      }),
    )
    .unwrap();
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 477,
                "character": 4
              },
              "end": {
                "line": 477,
                "character": 9
              }
            },
            "text": "error"
          }
        ]
      }),
    )
    .unwrap();
  // diagnostics only trigger after changes have elapsed in a separate thread,
  // so we need to delay the next messages a little bit to attempt to create a
  // potential for a deadlock with the codeAction
  std::thread::sleep(std::time::Duration::from_millis(50));
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 609,
          "character": 33,
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/codeAction",
      load_fixture("code_action_params_deadlock.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());

  for _ in 0..3 {
    let (method, _) = client.read_notification::<Value>().unwrap();
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  assert!(client.queue_is_empty());
  shutdown(&mut client);
}

#[test]
fn lsp_completions() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "Deno."
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/completion",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 5
        },
        "context": {
          "triggerKind": 2,
          "triggerCharacter": "."
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  if let Some(lsp::CompletionResponse::List(list)) = maybe_res {
    assert!(!list.is_incomplete);
    assert!(list.items.len() > 90);
  } else {
    panic!("unexpected response");
  }
  let (maybe_res, maybe_err) = client
    .write_request(
      "completionItem/resolve",
      load_fixture("completion_resolve_params.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("completion_resolve_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_completions_optional() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b?: string;\n}\n\nconst o: A = {};\n\nfunction c(s: string) {}\n\nc(o.)"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/completion",
      load_fixture("completion_request_params_optional.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "isIncomplete": false,
      "items": [
        {
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
        }
      ]
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "completionItem/resolve",
      load_fixture("completion_resolve_params_optional.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
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
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_completions_registry() {
  let _g = http_server();
  let mut client = init("initialize_params_registry.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://localhost:4545/x/a@\""
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/completion",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 46
        },
        "context": {
          "triggerKind": 2,
          "triggerCharacter": "@"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  if let Some(lsp::CompletionResponse::List(list)) = maybe_res {
    assert!(!list.is_incomplete);
    assert_eq!(list.items.len(), 3);
  } else {
    panic!("unexpected response");
  }
  let (maybe_res, maybe_err) = client
    .write_request(
      "completionItem/resolve",
      load_fixture("completion_resolve_params_registry.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("completion_resolve_response_registry.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_completions_registry_empty() {
  let _g = http_server();
  let mut client = init("initialize_params_registry.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"\""
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/completion",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 20
        },
        "context": {
          "triggerKind": 2,
          "triggerCharacter": "\""
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("completion_request_response_empty.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_diagnostics_warn() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/cli/tests/x_deno_warning.js\";\n\nconsole.log(a)\n",
      },
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "deno/cache",
      json!({
        "referrer": {
          "uri": "file:///a/file.ts",
        },
        "uris": [
          {
            "uri": "http://127.0.0.1:4545/cli/tests/x_deno_warning.js",
          }
        ],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());

  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _) = client.read_notification::<Value>().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, maybe_params) = client
    .read_notification::<lsp::PublishDiagnosticsParams>()
    .unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  assert_eq!(
    maybe_params,
    Some(lsp::PublishDiagnosticsParams {
      uri: Url::parse("file:///a/file.ts").unwrap(),
      diagnostics: vec![lsp::Diagnostic {
        range: lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 19
          },
          end: lsp::Position {
            line: 0,
            character: 70
          }
        },
        severity: Some(lsp::DiagnosticSeverity::Warning),
        code: Some(lsp::NumberOrString::String("deno-warn".to_string())),
        source: Some("deno".to_string()),
        message: "foobar".to_string(),
        ..Default::default()
      }],
      version: Some(1),
    })
  );
  shutdown(&mut client);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAverage {
  pub name: String,
  pub count: u32,
  pub average_duration: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceAverages {
  averages: Vec<PerformanceAverage>,
}

#[test]
fn lsp_performance() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.args);\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 19
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, PerformanceAverages>("deno/performance", json!({}))
    .unwrap();
  assert!(maybe_err.is_none());
  if let Some(res) = maybe_res {
    assert!(res.averages.len() >= 6);
  } else {
    panic!("unexpected result");
  }
  shutdown(&mut client);
}

#[test]
fn lsp_format_json() {
  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/file.json",
          "languageId": "json",
          "version": 1,
          "text": "{\"key\":\"value\"}"
        }
      }),
    )
    .unwrap();

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/formatting",
      json!({
          "textDocument": {
            "uri": "file:///a/file.json"
          },
          "options": {
            "tabSize": 2,
            "insertSpaces": true
          }
      }),
    )
    .unwrap();

  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "range": {
          "start": {
            "line": 0,
            "character": 1
          },
          "end": {
            "line": 0,
            "character": 1
          }
        },
        "newText": " "
      },
      {
        "range": {
          "start": { "line": 0, "character": 7 },
          "end": { "line": 0, "character": 7 }
        },
        "newText": " "
      },
      {
        "range": {
          "start": { "line": 0, "character": 14 },
          "end": { "line": 0, "character": 15 }
        },
        "newText": " }\n"
      }
    ]))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_format_markdown() {
  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/file.md",
          "languageId": "markdown",
          "version": 1,
          "text": "#   Hello World"
        }
      }),
    )
    .unwrap();

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
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
    )
    .unwrap();

  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "range": {
          "start": { "line": 0, "character": 1 },
          "end": { "line": 0, "character": 3 }
        },
        "newText": ""
      },
      {
        "range": {
          "start": { "line": 0, "character": 15 },
          "end": { "line": 0, "character": 15 }
        },
        "newText": "\n"
      }
    ]))
  );
  shutdown(&mut client);
}
