// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::fs;
use test_util::deno_exe_path;
use test_util::http_server;
use test_util::lsp::LspClient;
use test_util::testdata_path;
use test_util::TempDir;
use tower_lsp::lsp_types as lsp;

fn load_fixture(path: &str) -> Value {
  load_fixture_as(path)
}

fn load_fixture_as<T>(path: &str) -> T
where
  T: DeserializeOwned,
{
  let fixture_str = load_fixture_str(path);
  serde_json::from_str::<T>(&fixture_str).unwrap()
}

fn load_fixture_str(path: &str) -> String {
  let fixtures_path = testdata_path().join("lsp");
  let path = fixtures_path.join(path);
  fs::read_to_string(path).unwrap()
}

fn init(init_path: &str) -> LspClient {
  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", load_fixture(init_path))
    .unwrap();
  client.write_notification("initialized", json!({})).unwrap();
  client
}

fn did_open<V>(
  client: &mut LspClient,
  params: V,
) -> Vec<lsp::PublishDiagnosticsParams>
where
  V: Serialize,
{
  client
    .write_notification("textDocument/didOpen", params)
    .unwrap();

  handle_configuration_request(
    client,
    json!([{
      "enable": true,
      "codeLens": {
        "test": true
      }
    }]),
  );
  read_diagnostics(client).0
}

fn handle_configuration_request(client: &mut LspClient, result: Value) {
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client.write_response(id, result).unwrap();
}

fn read_diagnostics(client: &mut LspClient) -> CollectedDiagnostics {
  // diagnostics come in batches of three unless they're cancelled
  let mut diagnostics = vec![];
  for _ in 0..3 {
    let (method, response) = client
      .read_notification::<lsp::PublishDiagnosticsParams>()
      .unwrap();
    assert_eq!(method, "textDocument/publishDiagnostics");
    diagnostics.push(response.unwrap());
  }
  CollectedDiagnostics(diagnostics)
}

fn shutdown(client: &mut LspClient) {
  client
    .write_request::<_, _, Value>("shutdown", json!(null))
    .unwrap();
  client.write_notification("exit", json!(null)).unwrap();
}

pub fn ensure_directory_specifier(
  mut specifier: ModuleSpecifier,
) -> ModuleSpecifier {
  let path = specifier.path();
  if !path.ends_with('/') {
    let new_path = format!("{}/", path);
    specifier.set_path(&new_path);
  }
  specifier
}

struct TestSession {
  client: LspClient,
  open_file_count: usize,
}

impl TestSession {
  pub fn from_file(init_path: &str) -> Self {
    Self::from_client(init(init_path))
  }

  pub fn from_client(client: LspClient) -> Self {
    Self {
      client,
      open_file_count: 0,
    }
  }

  pub fn did_open<V>(&mut self, params: V) -> CollectedDiagnostics
  where
    V: Serialize,
  {
    self
      .client
      .write_notification("textDocument/didOpen", params)
      .unwrap();

    let (id, method, _) = self.client.read_request::<Value>().unwrap();
    assert_eq!(method, "workspace/configuration");
    self
      .client
      .write_response(
        id,
        json!([{
          "enable": true,
          "codeLens": {
            "test": true
          }
        }]),
      )
      .unwrap();

    self.open_file_count += 1;
    self.read_diagnostics()
  }

  pub fn read_diagnostics(&mut self) -> CollectedDiagnostics {
    let mut all_diagnostics = Vec::new();
    for _ in 0..self.open_file_count {
      all_diagnostics.extend(read_diagnostics(&mut self.client).0);
    }
    CollectedDiagnostics(all_diagnostics)
  }

  pub fn shutdown_and_exit(&mut self) {
    shutdown(&mut self.client);
  }
}

#[derive(Debug, Clone)]
struct CollectedDiagnostics(Vec<lsp::PublishDiagnosticsParams>);

impl CollectedDiagnostics {
  /// Gets the diagnostics that the editor will see after all the publishes.
  pub fn viewed(&self) -> Vec<lsp::Diagnostic> {
    self
      .viewed_messages()
      .into_iter()
      .flat_map(|m| m.diagnostics)
      .collect()
  }

  /// Gets the messages that the editor will see after all the publishes.
  pub fn viewed_messages(&self) -> Vec<lsp::PublishDiagnosticsParams> {
    // go over the publishes in reverse order in order to get
    // the final messages that will be shown in the editor
    let mut messages = Vec::new();
    let mut had_specifier = HashSet::new();
    for message in self.0.iter().rev() {
      if had_specifier.insert(message.uri.clone()) {
        messages.insert(0, message.clone());
      }
    }
    messages
  }

  pub fn with_source(&self, source: &str) -> lsp::PublishDiagnosticsParams {
    self
      .viewed_messages()
      .iter()
      .find(|p| {
        p.diagnostics
          .iter()
          .any(|d| d.source == Some(source.to_string()))
      })
      .map(ToOwned::to_owned)
      .unwrap()
  }

  pub fn with_file_and_source(
    &self,
    specifier: &str,
    source: &str,
  ) -> lsp::PublishDiagnosticsParams {
    let specifier = ModuleSpecifier::parse(specifier).unwrap();
    self
      .viewed_messages()
      .iter()
      .find(|p| {
        p.uri == specifier
          && p
            .diagnostics
            .iter()
            .any(|d| d.source == Some(source.to_string()))
      })
      .map(ToOwned::to_owned)
      .unwrap()
  }
}

#[test]
fn lsp_startup_shutdown() {
  let mut client = init("initialize_params.json");
  shutdown(&mut client);
}

#[test]
fn lsp_init_tsconfig() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let tsconfig =
    serde_json::to_vec_pretty(&load_fixture("lib.tsconfig.json")).unwrap();
  fs::write(temp_dir.path().join("lib.tsconfig.json"), tsconfig).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./lib.tsconfig.json"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "location.pathname;\n"
      }
    }),
  );

  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);

  shutdown(&mut client);
}

#[test]
fn lsp_tsconfig_types() {
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let temp_dir = TempDir::new();
  let tsconfig =
    serde_json::to_vec_pretty(&load_fixture("types.tsconfig.json")).unwrap();
  fs::write(temp_dir.path().join("types.tsconfig.json"), tsconfig).unwrap();
  let a_dts = load_fixture_str("a.d.ts");
  fs::write(temp_dir.path().join("a.d.ts"), a_dts).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./types.tsconfig.json"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": Url::from_file_path(temp_dir.path().join("test.ts")).unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(a);\n"
      }
    }),
  );

  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);

  shutdown(&mut client);
}

#[test]
fn lsp_tsconfig_bad_config_path() {
  let mut client = init("initialize_params_bad_config_option.json");
  let (method, maybe_params) = client.read_notification().unwrap();
  assert_eq!(method, "window/showMessage");
  assert_eq!(maybe_params, Some(lsp::ShowMessageParams {
    typ: lsp::MessageType::WARNING,
    message: "The path to the configuration file (\"bad_tsconfig.json\") is not resolvable.".to_string()
  }));
  let diagnostics = did_open(
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
  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);
}

#[test]
fn lsp_triple_slash_types() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let a_dts = load_fixture_str("a.d.ts");
  fs::write(temp_dir.path().join("a.d.ts"), a_dts).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": Url::from_file_path(temp_dir.path().join("test.ts")).unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "/// <reference types=\"./a.d.ts\" />\n\nconsole.log(a);\n"
      }
    }),
  );

  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);

  shutdown(&mut client);
}

#[test]
fn lsp_import_map() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let import_map =
    serde_json::to_vec_pretty(&load_fixture("import-map.json")).unwrap();
  fs::write(temp_dir.path().join("import-map.json"), import_map).unwrap();
  fs::create_dir(temp_dir.path().join("lib")).unwrap();
  fs::write(
    temp_dir.path().join("lib").join("b.ts"),
    r#"export const b = "b";"#,
  )
  .unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("importMap".to_string(), json!("import-map.json"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();
  let uri = Url::from_file_path(temp_dir.path().join("a.ts")).unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": uri,
        "languageId": "typescript",
        "version": 1,
        "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
      }
    }),
  );

  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": uri
        },
        "position": {
          "line": 2,
          "character": 12
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
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": {
          "line": 2,
          "character": 12
        },
        "end": {
          "line": 2,
          "character": 13
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_import_map_data_url() {
  let mut client = init("initialize_params_import_map.json");
  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import example from \"example\";\n"
      }
    }),
  );

  let mut diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  // This indicates that the import map from initialize_params_import_map.json
  // is applied correctly.
  assert!(diagnostics.any(|diagnostic| diagnostic.code
    == Some(lsp::NumberOrString::String("no-cache".to_string()))
    && diagnostic
      .message
      .contains("https://deno.land/x/example/mod.ts")));
  shutdown(&mut client);
}

#[test]
fn lsp_import_map_config_file() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();

  let deno_import_map_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.import_map.jsonc")).unwrap();
  fs::write(
    temp_dir.path().join("deno.import_map.jsonc"),
    deno_import_map_jsonc,
  )
  .unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./deno.import_map.jsonc"));
    params.initialization_options = Some(Value::Object(map));
  }
  let import_map =
    serde_json::to_vec_pretty(&load_fixture("import-map.json")).unwrap();
  fs::write(temp_dir.path().join("import-map.json"), import_map).unwrap();
  fs::create_dir(temp_dir.path().join("lib")).unwrap();
  fs::write(
    temp_dir.path().join("lib").join("b.ts"),
    r#"export const b = "b";"#,
  )
  .unwrap();

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();
  let uri = Url::from_file_path(temp_dir.path().join("a.ts")).unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": uri,
        "languageId": "typescript",
        "version": 1,
        "text": "import { b } from \"/~/b.ts\";\n\nconsole.log(b);\n"
      }
    }),
  );

  let diagnostics = diagnostics.into_iter().flat_map(|x| x.diagnostics);
  assert_eq!(diagnostics.count(), 0);

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": uri
        },
        "position": {
          "line": 2,
          "character": 12
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
          "value":"(alias) const b: \"b\"\nimport b"
        },
        ""
      ],
      "range": {
        "start": {
          "line": 2,
          "character": 12
        },
        "end": {
          "line": 2,
          "character": 13
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_deno_task() {
  let temp_dir = TempDir::new();
  let workspace_root = temp_dir.path().canonicalize().unwrap();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  fs::write(
    workspace_root.join("deno.jsonc"),
    r#"{
    "tasks": {
      "build": "deno test",
      "some:test": "deno bundle mod.ts"
    }
  }"#,
  )
  .unwrap();

  params.root_uri = Some(Url::from_file_path(workspace_root).unwrap());

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>("deno/task", json!(null))
    .unwrap();

  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "name": "build",
        "detail": "deno test"
      },
      {
        "name": "some:test",
        "detail": "deno bundle mod.ts"
      }
    ]))
  );
}

#[test]
fn lsp_import_assertions() {
  let mut client = init("initialize_params_import_map.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/test.json",
          "languageId": "json",
          "version": 1,
          "text": "{\"a\":1}"
        }
      }),
    )
    .unwrap();
  handle_configuration_request(
    &mut client,
    json!([{
      "enable": true,
      "codeLens": {
        "test": true
      }
    }]),
  );

  let diagnostics = CollectedDiagnostics(did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/a.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import a from \"./test.json\";\n\nconsole.log(a);\n"
      }
    }),
  ));

  assert_eq!(
    json!(
      diagnostics
        .with_file_and_source("file:///a/a.ts", "deno")
        .diagnostics
    ),
    json!([
      {
        "range": {
          "start": {
            "line": 0,
            "character": 14
          },
          "end": {
            "line": 0,
            "character": 27
          }
        },
        "severity": 1,
        "code": "no-assert-type",
        "source": "deno",
        "message": "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement."
      }
    ])
  );

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_params_import_assertion.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_response_import_assertion.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_import_map_import_completions() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let import_map =
    serde_json::to_vec_pretty(&load_fixture("import-map-completions.json"))
      .unwrap();
  fs::write(temp_dir.path().join("import-map.json"), import_map).unwrap();
  fs::create_dir(temp_dir.path().join("lib")).unwrap();
  fs::write(
    temp_dir.path().join("lib").join("b.ts"),
    r#"export const b = "b";"#,
  )
  .unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("importMap".to_string(), json!("import-map.json"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();
  let uri = Url::from_file_path(temp_dir.path().join("a.ts")).unwrap();

  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": uri,
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"/~/b.ts\";\nimport * as b from \"\""
      }
    }),
  );

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/completion",
      json!({
        "textDocument": {
          "uri": uri
        },
        "position": {
          "line": 1,
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
    Some(json!({
      "isIncomplete": false,
      "items": [
        {
          "label": ".",
          "kind": 19,
          "detail": "(local)",
          "sortText": "1",
          "insertText": ".",
          "commitCharacters": ["\"", "'"],
        },
        {
          "label": "..",
          "kind": 19,
          "detail": "(local)",
          "sortText": "1",
          "insertText": "..",
          "commitCharacters": ["\"", "'"],
        },
        {
          "label": "std",
          "kind": 19,
          "detail": "(import map)",
          "sortText": "std",
          "insertText": "std",
          "commitCharacters": ["\"", "'"],
        },
        {
          "label": "fs",
          "kind": 17,
          "detail": "(import map)",
          "sortText": "fs",
          "insertText": "fs",
          "commitCharacters": ["\"", "'"],
        },
        {
          "label": "/~",
          "kind": 19,
          "detail": "(import map)",
          "sortText": "/~",
          "insertText": "/~",
          "commitCharacters": ["\"", "'"],
        }
      ]
    }))
  );

  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": uri,
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 1,
                "character": 20
              },
              "end": {
                "line": 1,
                "character": 20
              }
            },
            "text": "/~/"
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
      "textDocument/completion",
      json!({
        "textDocument": {
          "uri": uri
        },
        "position": {
          "line": 1,
          "character": 23
        },
        "context": {
          "triggerKind": 2,
          "triggerCharacter": "/"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
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
              "start": {
                "line": 1,
                "character": 20
              },
              "end": {
                "line": 1,
                "character": 23
              }
            },
            "newText": "/~/b.ts"
          },
          "commitCharacters": ["\"", "'"],
        }
      ]
    }))
  );

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
          "uri": "deno:asset/lib.deno.shared_globals.d.ts"
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
          "uri": "deno:asset/lib.es2015.symbol.wellknown.d.ts"
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

  handle_configuration_request(&mut client, json!([{ "enable": false }]));

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
fn lsp_workspace_enable_paths() {
  let mut params: lsp::InitializeParams = serde_json::from_value(load_fixture(
    "initialize_params_workspace_enable_paths.json",
  ))
  .unwrap();
  // we aren't actually writing anything to the tempdir in this test, but we
  // just need a legitimate file path on the host system so that logic that
  // tries to convert to and from the fs paths works on all env
  let temp_dir = TempDir::new();

  let root_specifier =
    ensure_directory_specifier(Url::from_file_path(temp_dir.path()).unwrap());

  params.root_uri = Some(root_specifier.clone());
  params.workspace_folders = Some(vec![lsp::WorkspaceFolder {
    uri: root_specifier.clone(),
    name: "project".to_string(),
  }]);

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();

  handle_configuration_request(
    &mut client,
    json!([{
      "enable": false,
      "enablePaths": [
        "./worker"
      ],
    }]),
  );

  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": root_specifier.join("./file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );

  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": root_specifier.join("./other/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );

  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );

  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": root_specifier.join("./worker/subdir/file.ts").unwrap(),
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Date.now());\n"
      }
    }),
  );

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./file.ts").unwrap(),
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

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./other/file.ts").unwrap(),
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

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/file.ts").unwrap(),
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
          "value": "(method) DateConstructor.now(): number",
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 17,
        },
        "end": {
          "line": 0,
          "character": 20,
        }
      }
    }))
  );

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": root_specifier.join("./worker/subdir/file.ts").unwrap(),
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
          "value": "(method) DateConstructor.now(): number",
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 17,
        },
        "end": {
          "line": 0,
          "character": 20,
        }
      }
    }))
  );

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
        "text": "console.log(Deno.dlopen);\n"
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
          "character": 23
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
        "text": "console.log(Deno.ppid);\n"
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
          "value":"const Deno.ppid: number"
        },
        "The pid of the current process's parent."
      ],
      "range":{
        "start":{
          "line":0,
          "character":17
        },
        "end":{
          "line":0,
          "character":21
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
                // the LSP uses utf16 encoded characters indexes, so
                // after the deno emoiji is character index 15
                "character": 15
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
          "character": 15
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
          "character": 15,
        },
        "end": {
          "line": 2,
          "character": 16,
        },
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_closed_document() {
  let temp_dir_guard = TempDir::new();
  let temp_dir = temp_dir_guard.path();
  let a_path = temp_dir.join("a.ts");
  fs::write(a_path, r#"export const a = "a";"#).unwrap();
  let b_path = temp_dir.join("b.ts");
  fs::write(&b_path, r#"export * from "./a.ts";"#).unwrap();
  let b_specifier = Url::from_file_path(b_path).unwrap();
  let c_path = temp_dir.join("c.ts");
  fs::write(&c_path, "import { a } from \"./b.ts\";\nconsole.log(a);\n")
    .unwrap();
  let c_specifier = Url::from_file_path(c_path).unwrap();

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
    .write_response(id, json!([{ "enable": true }]))
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
    .write_response(id, json!([{ "enable": true }]))
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
fn lsp_hover_dependency() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file_01.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export const a = \"a\";\n",
      }
    }),
  );
  did_open(
    &mut client,
    load_fixture("did_open_params_import_hover.json"),
  );
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "deno/cache",
      json!({
        "referrer": {
          "uri": "file:///a/file.ts",
        },
        "uris": [],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 0,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n"
      },
      "range": {
        "start": {
          "line": 0,
          "character": 19
        },
        "end":{
          "line": 0,
          "character": 62
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 3,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/subdir/type_reference.js\n"
      },
      "range": {
        "start": {
          "line": 3,
          "character": 19
        },
        "end":{
          "line": 3,
          "character": 67
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 4,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/subdir/mod1.ts\n"
      },
      "range": {
        "start": {
          "line": 4,
          "character": 19
        },
        "end":{
          "line": 4,
          "character": 57
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 5,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: _(a data url)_\n"
      },
      "range": {
        "start": {
          "line": 5,
          "character": 19
        },
        "end":{
          "line": 5,
          "character": 132
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 6,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: file&#8203;:///a/file_01.ts\n"
      },
      "range": {
        "start": {
          "line": 6,
          "character": 19
        },
        "end":{
          "line": 6,
          "character": 33
        }
      }
    }))
  );
}

// This tests for a regression covered by denoland/deno#12753 where the lsp was
// unable to resolve dependencies when there was an invalid syntax in the module
#[test]
fn lsp_hover_deps_preserved_when_invalid_parse() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file1.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export type Foo = { bar(): string };\n"
      }
    }),
  );
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file2.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import { Foo } from './file1.ts'; declare const f: Foo; f\n"
      }
    }),
  );
  let (maybe_res, maybe_error) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file2.ts"
        },
        "position": {
          "line": 0,
          "character": 56
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const f: Foo",
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 56,
        },
        "end": {
          "line": 0,
          "character": 57,
        }
      }
    }))
  );
  client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file2.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 57
              },
              "end": {
                "line": 0,
                "character": 58
              }
            },
            "text": "."
          }
        ]
      }),
    )
    .unwrap();
  let (maybe_res, maybe_error) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file2.ts"
        },
        "position": {
          "line": 0,
          "character": 56
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": [
        {
          "language": "typescript",
          "value": "const f: Foo",
        },
        ""
      ],
      "range": {
        "start": {
          "line": 0,
          "character": 56,
        },
        "end": {
          "line": 0,
          "character": 57,
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_typescript_types() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/xTypeScriptTypes.js\";\n\nconsole.log(a.foo);\n",
      }
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
            "uri": "http://127.0.0.1:4545/xTypeScriptTypes.js",
          }
        ],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 0,
          "character": 24
        }
      }),
    )
    .unwrap();
  assert!(maybe_res.is_some());
  assert!(maybe_err.is_none());
  assert_eq!(
    json!(maybe_res.unwrap()),
    json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n\n**Types**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.d.ts\n"
      },
      "range": {
        "start": {
          "line": 0,
          "character": 19
        },
        "end": {
          "line": 0,
          "character": 62
        }
      }
    })
  );
  shutdown(&mut client);
}

#[test]
fn lsp_hover_jsdoc_symbol_link() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/b.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export function hello() {}\n"
      }
    }),
  );
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import { hello } from \"./b.ts\";\n\nhello();\n\nconst b = \"b\";\n\n/** JSDoc {@link hello} and {@linkcode b} */\nfunction a() {}\n"
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
          "line": 7,
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
          "value": "function a(): void"
        },
        "JSDoc [hello](file:///a/file.ts#L1,10) and [`b`](file:///a/file.ts#L5,7)"
      ],
      "range": {
        "start": {
          "line": 7,
          "character": 9
        },
        "end": {
          "line": 7,
          "character": 10
        }
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_goto_type_definition() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  a: string;\n}\n\nexport class B implements A {\n  a = \"a\";\n  log() {\n    console.log(this.a);\n  }\n}\n\nconst b = new B();\nb;\n",
      }
    }),
  );
  let (maybe_res, maybe_error) = client
    .write_request::<_, _, Value>(
      "textDocument/typeDefinition",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "position": {
          "line": 12,
          "character": 1
        }
      }),
    )
    .unwrap();
  assert!(maybe_error.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "targetUri": "file:///a/file.ts",
        "targetRange": {
          "start": {
            "line": 4,
            "character": 0
          },
          "end": {
            "line": 9,
            "character": 1
          }
        },
        "targetSelectionRange": {
          "start": {
            "line": 4,
            "character": 13
          },
          "end": {
            "line": 4,
            "character": 14
          }
        }
      }
    ]))
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
        // this should not rename in comments and strings
        "text": "let variable = 'a'; // variable\nconsole.log(variable);\n\"variable\";\n"
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
        "text": "interface A {\n  b(): void;\n}\n\nclass B implements A {\n  b() {\n    console.log(\"b\");\n  }\n}\n\ninterface C {\n  c: string;\n}\n"
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
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "codeLens/resolve",
      json!({
        "range": {
          "start": {
            "line": 10,
            "character": 10
          },
          "end": {
            "line": 10,
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
    Some(json!({
      "range": {
        "start": {
          "line": 10,
          "character": 10
        },
        "end": {
          "line": 10,
          "character": 11
        }
      },
      "command": {
        "title": "0 implementations",
        "command": ""
      }
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_lens_test() {
  let mut client = init("initialize_params_code_lens_test.json");
  did_open(
    &mut client,
    load_fixture("did_open_params_test_code_lens.json"),
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
    Some(load_fixture("code_lens_response_test.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_lens_test_disabled() {
  let mut client = init("initialize_params_code_lens_test_disabled.json");
  client
    .write_notification(
      "textDocument/didOpen",
      load_fixture("did_open_params_test_code_lens.json"),
    )
    .unwrap();

  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(
      id,
      json!([{
        "enable": true,
        "codeLens": {
          "test": false
        }
      }]),
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
      "textDocument/codeLens",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!([])));
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
          "uri": "deno:asset/lib.deno.shared_globals.d.ts"
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
          "uri": "deno:asset/lib.deno.shared_globals.d.ts"
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
fn lsp_nav_tree_updates() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  b(): void;\n}\n\nclass B implements A {\n  b() {\n    console.log(\"b\");\n  }\n}\n\ninterface C {\n  c: string;\n}\n"
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
                "line": 10,
                "character": 0
              },
              "end": {
                "line": 13,
                "character": 0
              }
            },
            "text": ""
          }
        ]
      }),
    )
    .unwrap();
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
    Some(load_fixture("code_lens_response_changed.json"))
  );
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
            },
            {
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
            },
            {
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
  let mut session = TestSession::from_file("initialize_params.json");
  let diagnostics = session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "import * as a from \"https://deno.land/x/a/mod.ts\";\n\nconsole.log(a);\n"
    }
  }));
  assert_eq!(
    diagnostics.with_source("deno"),
    load_fixture_as("diagnostics_deno_deps.json")
  );

  let (maybe_res, maybe_err) = session
    .client
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
  session.shutdown_and_exit();
}

#[test]
fn lsp_code_actions_imports() {
  let mut session = TestSession::from_file("initialize_params.json");
  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file00.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const abc = \"abc\";\nexport const def = \"def\";\n"
    }
  }));
  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "\nconsole.log(abc);\nconsole.log(def)\n"
    }
  }));

  let (maybe_res, maybe_err) = session
    .client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_params_imports.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_response_imports.json"))
  );
  let (maybe_res, maybe_err) = session
    .client
    .write_request(
      "codeAction/resolve",
      load_fixture("code_action_resolve_params_imports.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_resolve_response_imports.json"))
  );

  session.shutdown_and_exit();
}

#[test]
fn lsp_code_actions_refactor() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "var x: { a?: number; b?: string } = {};\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_params_refactor.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_response_refactor.json"))
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "codeAction/resolve",
      load_fixture("code_action_resolve_params_refactor.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_resolve_response_refactor.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_actions_refactor_no_disabled_support() {
  let mut client = init("initialize_params_ca_no_disabled.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "interface A {\n  a: string;\n}\n\ninterface B {\n  b: string;\n}\n\nclass AB implements A, B {\n  a = \"a\";\n  b = \"b\";\n}\n\nnew AB().a;\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
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
            "line": 14,
            "character": 0
          }
        },
        "context": {
          "diagnostics": [],
          "only": [
            "refactor"
          ]
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_response_no_disabled.json"))
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
    .write_response(id, json!([{ "enable": true }]))
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
  read_diagnostics(&mut client);
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

  read_diagnostics(&mut client);

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
fn lsp_completions_auto_import() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/b.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export const foo = \"foo\";\n",
      }
    }),
  );
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export {};\n\n",
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
          "line": 2,
          "character": 0,
        },
        "context": {
          "triggerKind": 1,
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  if let Some(lsp::CompletionResponse::List(list)) = maybe_res {
    assert!(!list.is_incomplete);
    if !list.items.iter().any(|item| item.label == "foo") {
      panic!("completions items missing 'foo' symbol");
    }
  } else {
    panic!("unexpected completion response");
  }
  let (maybe_res, maybe_err) = client
    .write_request(
      "completionItem/resolve",
      json!({
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
            "source": "./b",
            "data": {
              "exportName": "foo",
              "moduleSpecifier": "./b",
              "fileName": "file:///a/b.ts"
            },
            "useCodeSnippet": false
          }
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
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
            "start": {
              "line": 0,
              "character": 0
            },
            "end": {
              "line": 0,
              "character": 0
            }
          },
          "newText": "import { foo } from \"./b.ts\";\n\n"
        }
      ],
      "commitCharacters": [
        ".",
        ",",
        ";",
        "("
      ]
    }))
  );
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
fn lsp_auto_discover_registry() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
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
    .write_request::<_, _, Value>(
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
  assert!(maybe_res.is_some());
  let (method, maybe_res) = client.read_notification().unwrap();
  assert_eq!(method, "deno/registryState");
  assert_eq!(
    maybe_res,
    Some(json!({
      "origin": "http://localhost:4545",
      "suggestions": true,
    }))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_cache_location() {
  let _g = http_server();
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params_registry.json"))
      .unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("cache".to_string(), json!(".cache"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();
  client.write_notification("initialized", json!({})).unwrap();
  let mut session = TestSession::from_client(client);

  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    }
  }));
  let diagnostics =
    session.did_open(load_fixture("did_open_params_import_hover.json"));
  assert_eq!(diagnostics.viewed().len(), 7);
  let (maybe_res, maybe_err) = session
    .client
    .write_request::<_, _, Value>(
      "deno/cache",
      json!({
        "referrer": {
          "uri": "file:///a/file.ts",
        },
        "uris": [],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = session
    .client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 0,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://127.0.0.1:4545/xTypeScriptTypes.js\n"
      },
      "range": {
        "start": {
          "line": 0,
          "character": 19
        },
        "end":{
          "line": 0,
          "character": 62
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = session
    .client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 7,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/x/a/mod.ts\n\n\n---\n\n**a**\n\nmod.ts"
      },
      "range": {
        "start": {
          "line": 7,
          "character": 19
        },
        "end": {
          "line": 7,
          "character": 53
        }
      }
    }))
  );
  let cache_path = temp_dir.path().join(".cache");
  assert!(cache_path.is_dir());
  assert!(cache_path.join("gen").is_dir());
  session.shutdown_and_exit();
}

/// Sets the TLS root certificate on startup, which allows the LSP to connect to
/// the custom signed test server and be able to retrieve the registry config
/// and cache files.
#[test]
fn lsp_tls_cert() {
  let _g = http_server();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params_tls_cert.json"))
      .unwrap();

  params.root_uri = Some(Url::from_file_path(testdata_path()).unwrap());

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();
  client.write_notification("initialized", json!({})).unwrap();
  let mut session = TestSession::from_client(client);

  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    }
  }));
  let diagnostics =
    session.did_open(load_fixture("did_open_params_tls_cert.json"));
  let diagnostics = diagnostics.viewed();
  assert_eq!(diagnostics.len(), 7);
  let (maybe_res, maybe_err) = session
    .client
    .write_request::<_, _, Value>(
      "deno/cache",
      json!({
        "referrer": {
          "uri": "file:///a/file.ts",
        },
        "uris": [],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = session
    .client
    .write_request(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 0,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: https&#8203;://localhost:5545/xTypeScriptTypes.js\n"
      },
      "range": {
        "start": {
          "line": 0,
          "character": 19
        },
        "end":{
          "line": 0,
          "character": 63
        }
      }
    }))
  );
  let (maybe_res, maybe_err) = session
    .client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
        },
        "position": {
          "line": 7,
          "character": 28
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/x/a/mod.ts\n\n\n---\n\n**a**\n\nmod.ts"
      },
      "range": {
        "start": {
          "line": 7,
          "character": 19
        },
        "end": {
          "line": 7,
          "character": 53
        }
      }
    }))
  );
  session.shutdown_and_exit();
}

#[test]
fn lsp_diagnostics_warn_redirect() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/x_deno_warning.js\";\n\nconsole.log(a)\n",
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
            "uri": "http://127.0.0.1:4545/x_deno_warning.js",
          }
        ],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let diagnostics = read_diagnostics(&mut client);
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
          message: "The import of \"http://127.0.0.1:4545/x_deno_warning.js\" was redirected to \"http://127.0.0.1:4545/x_deno_warning_redirect.js\".".to_string(),
          data: Some(json!({"specifier": "http://127.0.0.1:4545/x_deno_warning.js", "redirect": "http://127.0.0.1:4545/x_deno_warning_redirect.js"})),
          ..Default::default()
        }
      ],
      version: Some(1),
    }
  );
  shutdown(&mut client);
}

#[test]
fn lsp_redirect_quick_fix() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "import * as a from \"http://127.0.0.1:4545/x_deno_warning.js\";\n\nconsole.log(a)\n",
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
            "uri": "http://127.0.0.1:4545/x_deno_warning.js",
          }
        ],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let diagnostics = read_diagnostics(&mut client)
    .with_source("deno")
    .diagnostics;
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      json!(json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        },
        "range": {
          "start": {
            "line": 0,
            "character": 19
          },
          "end": {
            "line": 0,
            "character": 60
          }
        },
        "context": {
          "diagnostics": diagnostics,
          "only": [
            "quickfix"
          ]
        }
      })),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_redirect_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_diagnostics_deprecated() {
  let mut client = init("initialize_params.json");
  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "/** @deprecated */\nexport const a = \"a\";\n\na;\n",
      },
    }),
  );
  assert_eq!(
    json!(diagnostics),
    json!([
      {
        "uri": "file:///a/file.ts",
        "diagnostics": [],
        "version": 1
      },
      {
        "uri": "file:///a/file.ts",
        "diagnostics": [],
        "version": 1
      },
      {
        "uri": "file:///a/file.ts",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 3,
                "character": 0
              },
              "end": {
                "line": 3,
                "character": 1
              }
            },
            "severity": 4,
            "code": 6385,
            "source": "deno-ts",
            "message": "'a' is deprecated.",
            "relatedInformation": [],
            "tags": [
              2
            ]
          }
        ],
        "version": 1
      }
    ])
  );
  shutdown(&mut client);
}

#[test]
fn lsp_diagnostics_deno_types() {
  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      load_fixture("did_open_params_deno_types.json"),
    )
    .unwrap();
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(id, json!([{ "enable": true }]))
    .unwrap();
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/documentSymbol",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_res.is_some());
  assert!(maybe_err.is_none());
  let diagnostics = read_diagnostics(&mut client);
  assert_eq!(diagnostics.viewed().len(), 5);
  shutdown(&mut client);
}

#[test]
fn lsp_diagnostics_refresh_dependents() {
  let mut session = TestSession::from_file("initialize_params.json");
  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_00.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export const a = \"a\";\n",
    },
  }));
  session.did_open(json!({
    "textDocument": {
      "uri": "file:///a/file_01.ts",
      "languageId": "typescript",
      "version": 1,
      "text": "export * from \"./file_00.ts\";\n",
    },
  }));
  let diagnostics = session.did_open(json!({
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
            "start": {
              "line": 0,
              "character": 12
            },
            "end": {
              "line": 0,
              "character": 13
            }
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
  session
    .client
    .write_notification(
      "textDocument/didChange",
      json!({
        "textDocument": {
          "uri": "file:///a/file_00.ts",
          "version": 2
        },
        "contentChanges": [
          {
            "range": {
              "start": {
                "line": 1,
                "character": 0
              },
              "end": {
                "line": 1,
                "character": 0
              }
            },
            "text": "export const b = \"b\";\n"
          }
        ]
      }),
    )
    .unwrap();
  let diagnostics = session.read_diagnostics();
  assert_eq!(diagnostics.viewed().len(), 0); // no diagnostics now

  session.shutdown_and_exit();
  assert_eq!(session.client.queue_len(), 0);
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
    .write_request::<_, _, PerformanceAverages>("deno/performance", json!(null))
    .unwrap();
  assert!(maybe_err.is_none());
  if let Some(res) = maybe_res {
    assert_eq!(res.averages.len(), 13);
  } else {
    panic!("unexpected result");
  }
  shutdown(&mut client);
}

#[test]
fn lsp_format_no_changes() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console;\n"
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
  assert_eq!(maybe_res, Some(json!(null)));
  client.assert_no_notification("window/showMessage");
  shutdown(&mut client);
}

#[test]
fn lsp_format_error() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console test test\n"
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
  assert_eq!(maybe_res, Some(json!(null)));
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
fn lsp_format_exclude_with_config() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let deno_fmt_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.fmt.exclude.jsonc")).unwrap();
  fs::write(temp_dir.path().join("deno.fmt.jsonc"), deno_fmt_jsonc).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./deno.fmt.jsonc"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  let file_uri =
    ModuleSpecifier::from_file_path(temp_dir.path().join("ignored.ts"))
      .unwrap()
      .to_string();
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": file_uri,
        "languageId": "typescript",
        "version": 1,
        "text": "function   myFunc(){}"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
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
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));
  shutdown(&mut client);
}

#[test]
fn lsp_format_exclude_default_config() {
  let temp_dir = TempDir::new();
  let workspace_root = temp_dir.path().canonicalize().unwrap();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let deno_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.fmt.exclude.jsonc")).unwrap();
  fs::write(workspace_root.join("deno.jsonc"), deno_jsonc).unwrap();

  params.root_uri = Some(Url::from_file_path(workspace_root.clone()).unwrap());

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  let file_uri =
    ModuleSpecifier::from_file_path(workspace_root.join("ignored.ts"))
      .unwrap()
      .to_string();
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": file_uri,
        "languageId": "typescript",
        "version": 1,
        "text": "function   myFunc(){}"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
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
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));
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
fn lsp_json_no_diagnostics() {
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
    .write_request(
      "textDocument/semanticTokens/full",
      json!({
        "textDocument": {
          "uri": "file:///a/file.json"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.json"
        },
        "position": {
          "line": 0,
          "character": 3
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));

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

#[test]
fn lsp_format_with_config() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let deno_fmt_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.fmt.jsonc")).unwrap();
  fs::write(temp_dir.path().join("deno.fmt.jsonc"), deno_fmt_jsonc).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./deno.fmt.jsonc"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/file.ts",
          "languageId": "typescript",
          "version": 1,
          "text": "export async function someVeryLongFunctionName() {\nconst response = fetch(\"http://localhost:4545/some/non/existent/path.json\");\nconsole.log(response.text());\nconsole.log(\"finished!\")\n}"
        }
      }),
    )
    .unwrap();

  // The options below should be ignored in favor of configuration from config file.
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
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
    Some(json!([{
        "range": {
          "start": {
            "line": 1,
            "character": 0
          },
          "end": {
            "line": 1,
            "character": 0
          }
        },
        "newText": "\t"
      },
      {
        "range": {
          "start": {
            "line": 1,
            "character": 23
          },
          "end": {
            "line": 1,
            "character": 24
          }
        },
        "newText": "\n\t\t'"
      },
      {
        "range": {
          "start": {
            "line": 1,
            "character": 73
          },
          "end": {
            "line": 1,
            "character": 74
          }
        },
        "newText": "',\n\t"
      },
      {
        "range": {
          "start": {
            "line": 2,
            "character": 0
          },
          "end": {
            "line": 2,
            "character": 0
          }
        },
        "newText": "\t"
      },
      {
        "range": {
          "start": {
            "line": 3,
            "character": 0
          },
          "end": {
            "line": 3,
            "character": 0
          }
        },
        "newText": "\t"
      },
      {
        "range": {
          "start": {
            "line": 3,
            "character": 12
          },
          "end": {
            "line": 3,
            "character": 13
          }
        },
        "newText": "'"
      },
      {
        "range": {
          "start": {
            "line": 3,
            "character": 22
          },
          "end": {
            "line": 3,
            "character": 24
          }
        },
        "newText": "');"
      },
      {
        "range": {
          "start": {
            "line": 4,
            "character": 1
          },
          "end": {
            "line": 4,
            "character": 1
          }
        },
        "newText": "\n"
      }]
    ))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_markdown_no_diagnostics() {
  let mut client = init("initialize_params.json");
  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///a/file.md",
          "languageId": "markdown",
          "version": 1,
          "text": "# Hello World"
        }
      }),
    )
    .unwrap();

  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/semanticTokens/full",
      json!({
        "textDocument": {
          "uri": "file:///a/file.md"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.md"
        },
        "position": {
          "line": 0,
          "character": 3
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(maybe_res, Some(json!(null)));

  shutdown(&mut client);
}

#[test]
fn lsp_configuration_did_change() {
  let _g = http_server();
  let mut client = init("initialize_params_did_config_change.json");
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
  client
    .write_notification(
      "workspace/didChangeConfiguration",
      json!({
        "settings": {}
      }),
    )
    .unwrap();
  let (id, method, _) = client.read_request::<Value>().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(
      id,
      json!([{
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
      }]),
    )
    .unwrap();
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
fn lsp_workspace_symbol() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export class A {\n  fieldA: string;\n  fieldB: string;\n}\n",
      }
    }),
  );
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file_01.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "export class B {\n  fieldC: string;\n  fieldD: string;\n}\n",
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "workspace/symbol",
      json!({
        "query": "field"
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!([
      {
        "name": "fieldA",
        "kind": 8,
        "location": {
          "uri": "file:///a/file.ts",
          "range": {
            "start": {
              "line": 1,
              "character": 2
            },
            "end": {
              "line": 1,
              "character": 17
            }
          }
        },
        "containerName": "A"
      },
      {
        "name": "fieldB",
        "kind": 8,
        "location": {
          "uri": "file:///a/file.ts",
          "range": {
            "start": {
              "line": 2,
              "character": 2
            },
            "end": {
              "line": 2,
              "character": 17
            }
          }
        },
        "containerName": "A"
      },
      {
        "name": "fieldC",
        "kind": 8,
        "location": {
          "uri": "file:///a/file_01.ts",
          "range": {
            "start": {
              "line": 1,
              "character": 2
            },
            "end": {
              "line": 1,
              "character": 17
            }
          }
        },
        "containerName": "B"
      },
      {
        "name": "fieldD",
        "kind": 8,
        "location": {
          "uri": "file:///a/file_01.ts",
          "range": {
            "start": {
              "line": 2,
              "character": 2
            },
            "end": {
              "line": 2,
              "character": 17
            }
          }
        },
        "containerName": "B"
      }
    ]))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_code_actions_ignore_lint() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "let message = 'Hello, Deno!';\nconsole.log(message);\n"
      }
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_ignore_lint_params.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_ignore_lint_response.json"))
  );
  shutdown(&mut client);
}

/// This test exercises updating an existing deno-lint-ignore-file comment.
#[test]
fn lsp_code_actions_update_ignore_lint() {
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
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
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request(
      "textDocument/codeAction",
      load_fixture("code_action_update_ignore_lint_params.json"),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(load_fixture("code_action_update_ignore_lint_response.json"))
  );
  shutdown(&mut client);
}

#[test]
fn lsp_lint_with_config() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let deno_lint_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.lint.jsonc")).unwrap();
  fs::write(temp_dir.path().join("deno.lint.jsonc"), deno_lint_jsonc).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./deno.lint.jsonc"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();
  let mut session = TestSession::from_client(client);

  let diagnostics = session.did_open(load_fixture("did_open_lint.json"));
  let diagnostics = diagnostics.viewed();
  assert_eq!(diagnostics.len(), 1);
  assert_eq!(
    diagnostics[0].code,
    Some(lsp::NumberOrString::String("ban-untagged-todo".to_string()))
  );
  session.shutdown_and_exit();
}

#[test]
fn lsp_lint_exclude_with_config() {
  let temp_dir = TempDir::new();
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let deno_lint_jsonc =
    serde_json::to_vec_pretty(&load_fixture("deno.lint.exclude.jsonc"))
      .unwrap();
  fs::write(temp_dir.path().join("deno.lint.jsonc"), deno_lint_jsonc).unwrap();

  params.root_uri = Some(Url::from_file_path(temp_dir.path()).unwrap());
  if let Some(Value::Object(mut map)) = params.initialization_options {
    map.insert("config".to_string(), json!("./deno.lint.jsonc"));
    params.initialization_options = Some(Value::Object(map));
  }

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  let diagnostics = did_open(
    &mut client,
    json!({
      "textDocument": {
        "uri": ModuleSpecifier::from_file_path(temp_dir.path().join("ignored.ts")).unwrap().to_string(),
        "languageId": "typescript",
        "version": 1,
        "text": "// TODO: fixme\nexport async function non_camel_case() {\nconsole.log(\"finished!\")\n}"
      }
    }),
  );
  let diagnostics = diagnostics
    .into_iter()
    .flat_map(|x| x.diagnostics)
    .collect::<Vec<_>>();
  assert_eq!(diagnostics, Vec::new());
  shutdown(&mut client);
}

#[test]
fn lsp_jsx_import_source_pragma() {
  let _g = http_server();
  let mut client = init("initialize_params.json");
  did_open(
    &mut client,
    json!({
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
    }),
  );
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "deno/cache",
      json!({
        "referrer": {
          "uri": "file:///a/file.tsx",
        },
        "uris": [
          {
            "uri": "http://127.0.0.1:4545/jsx/jsx-runtime",
          }
        ],
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Value>(
      "textDocument/hover",
      json!({
        "textDocument": {
          "uri": "file:///a/file.tsx"
        },
        "position": {
          "line": 0,
          "character": 25
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert_eq!(
    maybe_res,
    Some(json!({
      "contents": {
        "kind": "markdown",
        "value": "**Resolved Dependency**\n\n**Code**: http&#8203;://localhost:4545/jsx/jsx-runtime\n",
      },
      "range": {
        "start": {
          "line": 0,
          "character": 21
        },
        "end": {
          "line": 0,
          "character": 46
        }
      }
    }))
  );
  shutdown(&mut client);
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
  let mut params: lsp::InitializeParams =
    serde_json::from_value(load_fixture("initialize_params.json")).unwrap();
  let temp_dir = TempDir::new();

  let root_specifier =
    ensure_directory_specifier(Url::from_file_path(temp_dir.path()).unwrap());

  let module_path = temp_dir.path().join("./test.ts");
  let specifier = ModuleSpecifier::from_file_path(&module_path).unwrap();
  let contents = r#"
Deno.test({
  name: "test a",
  fn() {
    console.log("test a");
  }
});
"#;
  fs::write(&module_path, &contents).unwrap();
  fs::write(temp_dir.path().join("./deno.jsonc"), r#"{}"#).unwrap();

  params.root_uri = Some(root_specifier);

  let deno_exe = deno_exe_path();
  let mut client = LspClient::new(&deno_exe, false).unwrap();
  client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();

  client.write_notification("initialized", json!({})).unwrap();

  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": specifier,
          "languageId": "typescript",
          "version": 1,
          "text": contents,
        }
      }),
    )
    .unwrap();

  handle_configuration_request(
    &mut client,
    json!([{
      "enable": true,
      "codeLens": {
        "test": true
      }
    }]),
  );

  for _ in 0..4 {
    let result = client.read_notification::<Value>();
    assert!(result.is_ok());
    let (method, notification) = result.unwrap();
    if method.as_str() == "deno/testModule" {
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
    }
  }

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, TestRunResponseParams>(
      "deno/testRun",
      json!({
        "id": 1,
        "kind": "run",
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let res = maybe_res.unwrap();
  assert_eq!(res.enqueued.len(), 1);
  assert_eq!(res.enqueued[0].text_document.uri, specifier);
  assert_eq!(res.enqueued[0].ids.len(), 1);
  let id = res.enqueued[0].ids[0].clone();

  let res = client.read_notification::<Value>();
  assert!(res.is_ok());
  let (method, notification) = res.unwrap();
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

  let res = client.read_notification::<Value>();
  assert!(res.is_ok());
  let (method, notification) = res.unwrap();
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

  let res = client.read_notification::<Value>();
  assert!(res.is_ok());
  let (method, notification) = res.unwrap();
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

      let res = client.read_notification::<Value>();
      assert!(res.is_ok());
      let (method, notification) = res.unwrap();
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

  shutdown(&mut client);
}
