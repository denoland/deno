// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use lsp_types::Uri;
use test_util::lsp::LspClientBuilder;
use test_util::PathRef;
use tower_lsp::lsp_types as lsp;

static FIXTURE_CODE_LENS_TS: &str = include_str!("testdata/code_lens.ts");
static FIXTURE_DB_TS: &str = include_str!("testdata/db.ts");
static FIXTURE_DB_MESSAGES: &[u8] = include_bytes!("testdata/db_messages.json");
static FIXTURE_DECO_APPS: &[u8] =
  include_bytes!("testdata/deco_apps_requests.json");

#[derive(Debug, Deserialize)]
enum FixtureType {
  #[serde(rename = "action")]
  Action,
  #[serde(rename = "change")]
  Change,
  #[serde(rename = "completion")]
  Completion,
  #[serde(rename = "highlight")]
  Highlight,
  #[serde(rename = "hover")]
  Hover,
}

#[derive(Debug, Deserialize)]
struct FixtureMessage {
  #[serde(rename = "type")]
  fixture_type: FixtureType,
  params: Value,
}

/// replaces the root directory in the URIs of the requests
/// with the given root path
fn patch_uris<'a>(
  reqs: impl IntoIterator<Item = &'a mut tower_lsp::jsonrpc::Request>,
  root: &PathRef,
) {
  for req in reqs {
    let mut params = req.params().unwrap().clone();
    let new_req = if let Some(doc) = params.get_mut("textDocument") {
      if let Some(uri_val) = doc.get_mut("uri") {
        let uri = uri_val.as_str().unwrap();
        *uri_val =
          Value::from(uri.replace(
            "file:///",
            &format!("file://{}/", root.to_string_lossy()),
          ));
      }
      let builder = tower_lsp::jsonrpc::Request::build(req.method().to_owned());
      let builder = if let Some(id) = req.id() {
        builder.id(id.clone())
      } else {
        builder
      };

      Some(builder.params(params).finish())
    } else {
      None
    };

    if let Some(new_req) = new_req {
      *req = new_req.request;
    }
  }
}

fn bench_deco_apps_edits(deno_exe: &Path) -> Duration {
  let mut requests: Vec<tower_lsp::jsonrpc::Request> =
    serde_json::from_slice(FIXTURE_DECO_APPS).unwrap();
  let apps =
    test_util::root_path().join("cli/bench/testdata/lsp_benchdata/apps");

  // it's a bit wasteful to do this for every run, but it's the easiest with the way things
  // are currently structured
  patch_uris(&mut requests, &apps);

  let mut client = LspClientBuilder::new()
    .use_diagnostic_sync(false)
    .set_root_dir(apps.clone())
    .deno_exe(deno_exe)
    .build();
  client.initialize(|c| {
    c.set_workspace_folders(vec![lsp_types::WorkspaceFolder {
      uri: apps.uri_dir(),
      name: "apps".to_string(),
    }]);
    c.set_deno_enable(true);
    c.set_unstable(true);
    c.set_preload_limit(1000);
    c.set_config(apps.join("deno.json").as_path().to_string_lossy());
  });

  let start = std::time::Instant::now();

  let mut reqs = 0;
  for req in requests {
    if req.id().is_none() {
      client.write_notification(req.method(), req.params());
    } else {
      reqs += 1;
      client.write_jsonrpc(req.method(), req.params());
    }
  }
  for _ in 0..reqs {
    let _ = client.read_latest_response();
  }

  let end = start.elapsed();

  // part of the motivation of including this benchmark is to see how we perform
  // with a fairly large number of documents in memory.
  // make sure that's the case
  let res = client.write_request(
    "deno/virtualTextDocument",
    json!({
      "textDocument": {
        "uri": "deno:/status.md"
      }
    }),
  );
  let open_re = lazy_regex::regex!(r"Open: (\d+)");
  let server_re = lazy_regex::regex!(r"Server: (\d+)");
  let res = res.as_str().unwrap().to_string();
  assert!(res.starts_with("# Deno Language Server Status"));
  let open_count = open_re
    .captures(&res)
    .unwrap()
    .get(1)
    .unwrap()
    .as_str()
    .parse::<usize>()
    .unwrap();
  let server_count = server_re
    .captures(&res)
    .unwrap()
    .get(1)
    .unwrap()
    .as_str()
    .parse::<usize>()
    .unwrap();
  let count = open_count + server_count;
  assert!(count > 1000, "count: {}", count);

  client.shutdown();

  end
}

/// A benchmark that opens a 8000+ line TypeScript document, adds a function to
/// the end of the document and does a level of hovering and gets quick fix
/// code actions.
fn bench_big_file_edits(deno_exe: &Path) -> Duration {
  let mut client = LspClientBuilder::new()
    .use_diagnostic_sync(false)
    .deno_exe(deno_exe)
    .build();
  client.initialize_default();
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");
  client.change_configuration(json!({ "deno": { "enable": true } }));
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");

  client.write_notification(
    "textDocument/didOpen",
    json!({
      "textDocument": {
        "uri": "file:///testdata/db.ts",
        "languageId": "typescript",
        "version": 1,
        "text": FIXTURE_DB_TS
      }
    }),
  );

  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");

  let messages: Vec<FixtureMessage> =
    serde_json::from_slice(FIXTURE_DB_MESSAGES).unwrap();

  for msg in messages {
    match msg.fixture_type {
      FixtureType::Action => {
        client.write_request("textDocument/codeAction", msg.params);
      }
      FixtureType::Change => {
        client.write_notification("textDocument/didChange", msg.params);
      }
      FixtureType::Completion => {
        client.write_request("textDocument/completion", msg.params);
      }
      FixtureType::Highlight => {
        client.write_request("textDocument/documentHighlight", msg.params);
      }
      FixtureType::Hover => {
        client.write_request("textDocument/hover", msg.params);
      }
    }
  }

  client.write_request("shutdown", json!(null));
  client.write_notification("exit", json!(null));

  client.duration()
}

fn bench_code_lens(deno_exe: &Path) -> Duration {
  let mut client = LspClientBuilder::new()
    .use_diagnostic_sync(false)
    .deno_exe(deno_exe)
    .build();
  client.initialize_default();
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");
  client.change_configuration(json!({ "deno": {
    "enable": true,
    "codeLens": {
      "implementations": true,
      "references": true,
      "test": true,
    },
  } }));
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");

  client.write_notification(
    "textDocument/didOpen",
    json!({
      "textDocument": {
        "uri": "file:///testdata/code_lens.ts",
        "languageId": "typescript",
        "version": 1,
        "text": FIXTURE_CODE_LENS_TS
      }
    }),
  );

  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");

  let res = client.write_request_with_res_as::<Vec<lsp::CodeLens>>(
    "textDocument/codeLens",
    json!({
      "textDocument": {
        "uri": "file:///testdata/code_lens.ts"
      }
    }),
  );
  assert!(!res.is_empty());

  for code_lens in res {
    client.write_request("codeLens/resolve", code_lens);
  }

  client.duration()
}

fn bench_find_replace(deno_exe: &Path) -> Duration {
  let mut client = LspClientBuilder::new()
    .use_diagnostic_sync(false)
    .deno_exe(deno_exe)
    .build();
  client.initialize_default();
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");
  client.change_configuration(json!({ "deno": { "enable": true } }));
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");

  for i in 0..10 {
    client.write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": format!("file:///a/file_{i}.ts"),
          "languageId": "typescript",
          "version": 1,
          "text": "console.log(\"000\");\n"
        }
      }),
    );
  }

  for _ in 0..3 {
    let (method, _): (String, Option<Value>) = client.read_notification();
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  for i in 0..10 {
    let file_name = format!("file:///a/file_{i}.ts");
    client.write_notification(
      "textDocument/didChange",
      lsp::DidChangeTextDocumentParams {
        text_document: lsp::VersionedTextDocumentIdentifier {
          uri: Uri::from_str(&file_name).unwrap(),
          version: 2,
        },
        content_changes: vec![lsp::TextDocumentContentChangeEvent {
          range: Some(lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 13,
            },
            end: lsp::Position {
              line: 0,
              character: 16,
            },
          }),
          range_length: None,
          text: "111".to_string(),
        }],
      },
    );
  }

  for i in 0..10 {
    let file_name = format!("file:///a/file_{i}.ts");
    client.write_request(
      "textDocument/formatting",
      lsp::DocumentFormattingParams {
        text_document: lsp::TextDocumentIdentifier {
          uri: Uri::from_str(&file_name).unwrap(),
        },
        options: lsp::FormattingOptions {
          tab_size: 2,
          insert_spaces: true,
          ..Default::default()
        },
        work_done_progress_params: Default::default(),
      },
    );
  }

  for _ in 0..3 {
    let (method, _): (String, Option<Value>) = client.read_notification();
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  client.write_request("shutdown", json!(null));
  client.write_notification("exit", json!(null));

  client.duration()
}

/// A test that starts up the LSP, opens a single line document, and exits.
fn bench_startup_shutdown(deno_exe: &Path) -> Duration {
  let mut client = LspClientBuilder::new()
    .use_diagnostic_sync(false)
    .deno_exe(deno_exe)
    .build();
  client.initialize_default();
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");
  client.change_configuration(json!({ "deno": { "enable": true } }));
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");

  client.write_notification(
    "textDocument/didOpen",
    json!({
      "textDocument": {
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.args);\n"
      }
    }),
  );

  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "textDocument/publishDiagnostics");

  client.write_request("shutdown", json!(null));

  client.write_notification("exit", json!(null));

  client.duration()
}

/// Generate benchmarks for the LSP server.
pub fn benchmarks(deno_exe: &Path) -> HashMap<String, i64> {
  println!("-> Start benchmarking lsp");
  let mut exec_times = HashMap::new();

  println!("   - Simple Startup/Shutdown ");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_startup_shutdown(deno_exe));
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as i64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("startup_shutdown".to_string(), mean);

  println!("   - Big Document/Several Edits ");
  let mut times = Vec::new();
  for _ in 0..5 {
    times.push(bench_big_file_edits(deno_exe));
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as i64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("big_file_edits".to_string(), mean);

  println!("   - Find/Replace");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_find_replace(deno_exe));
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as i64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("find_replace".to_string(), mean);

  println!("   - Code Lens");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_code_lens(deno_exe));
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as i64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("code_lens".to_string(), mean);

  println!("   - deco-cx/apps Multiple Edits + Navigation");
  let mut times = Vec::new();
  for _ in 0..5 {
    times.push(bench_deco_apps_edits(deno_exe));
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as i64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("deco_apps_edits_nav".to_string(), mean);

  println!("<- End benchmarking lsp");

  exec_times
}
