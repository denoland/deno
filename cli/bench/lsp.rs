// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use lspower::lsp;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use test_util::lsp::LspClient;
use test_util::lsp::LspResponseError;

static FIXTURE_CODE_LENS_TS: &str = include_str!("testdata/code_lens.ts");
static FIXTURE_DB_TS: &str = include_str!("testdata/db.ts");
static FIXTURE_DB_MESSAGES: &[u8] = include_bytes!("testdata/db_messages.json");
static FIXTURE_INIT_JSON: &[u8] =
  include_bytes!("testdata/initialize_params.json");

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

/// A benchmark that opens a 8000+ line TypeScript document, adds a function to
/// the end of the document and does a level of hovering and gets quick fix
/// code actions.
fn bench_big_file_edits(deno_exe: &Path) -> Result<Duration, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON)?;
  let (_, response_error): (Option<Value>, Option<LspResponseError>) =
    client.write_request("initialize", params)?;
  assert!(response_error.is_none());

  client.write_notification("initialized", json!({}))?;

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
  )?;

  let (id, method, _): (u64, String, Option<Value>) = client.read_request()?;
  assert_eq!(method, "workspace/configuration");

  client.write_response(
    id,
    json!({
      "enable": true
    }),
  )?;

  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");

  let messages: Vec<FixtureMessage> =
    serde_json::from_slice(FIXTURE_DB_MESSAGES)?;

  for msg in messages {
    match msg.fixture_type {
      FixtureType::Action => {
        client.write_request::<_, _, Value>(
          "textDocument/codeAction",
          msg.params,
        )?;
      }
      FixtureType::Change => {
        client.write_notification("textDocument/didChange", msg.params)?;
      }
      FixtureType::Completion => {
        client.write_request::<_, _, Value>(
          "textDocument/completion",
          msg.params,
        )?;
      }
      FixtureType::Highlight => {
        client.write_request::<_, _, Value>(
          "textDocument/documentHighlight",
          msg.params,
        )?;
      }
      FixtureType::Hover => {
        client
          .write_request::<_, _, Value>("textDocument/hover", msg.params)?;
      }
    }
  }

  let (_, response_error): (Option<Value>, Option<LspResponseError>) =
    client.write_request("shutdown", json!(null))?;
  assert!(response_error.is_none());

  client.write_notification("exit", json!(null))?;

  Ok(client.duration())
}

fn bench_code_lens(deno_exe: &Path) -> Result<Duration, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON)?;
  let (_, maybe_err) =
    client.write_request::<_, _, Value>("initialize", params)?;
  assert!(maybe_err.is_none());
  client.write_notification("initialized", json!({}))?;

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
  )?;

  let (id, method, _): (u64, String, Option<Value>) = client.read_request()?;
  assert_eq!(method, "workspace/configuration");

  client.write_response(
    id,
    json!({
      "enable": true
    }),
  )?;

  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");

  let (maybe_res, maybe_err) = client
    .write_request::<_, _, Vec<lsp::CodeLens>>(
      "textDocument/codeLens",
      json!({
        "textDocument": {
          "uri": "file:///testdata/code_lens.ts"
        }
      }),
    )
    .unwrap();
  assert!(maybe_err.is_none());
  assert!(maybe_res.is_some());
  let res = maybe_res.unwrap();
  assert!(!res.is_empty());

  for code_lens in res {
    let (maybe_res, maybe_err) = client
      .write_request::<_, _, lsp::CodeLens>("codeLens/resolve", code_lens)
      .unwrap();
    assert!(maybe_err.is_none());
    assert!(maybe_res.is_some());
  }

  Ok(client.duration())
}

fn bench_find_replace(deno_exe: &Path) -> Result<Duration, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON)?;
  let (_, maybe_err) =
    client.write_request::<_, _, Value>("initialize", params)?;
  assert!(maybe_err.is_none());
  client.write_notification("initialized", json!({}))?;

  for i in 0..10 {
    client.write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": format!("file:///a/file_{}.ts", i),
          "languageId": "typescript",
          "version": 1,
          "text": "console.log(\"000\");\n"
        }
      }),
    )?;
  }

  for _ in 0..10 {
    let (id, method, _) = client.read_request::<Value>()?;
    assert_eq!(method, "workspace/configuration");
    client.write_response(id, json!({ "enable": true }))?;
  }

  for _ in 0..3 {
    let (method, _): (String, Option<Value>) = client.read_notification()?;
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  for i in 0..10 {
    let file_name = format!("file:///a/file_{}.ts", i);
    client.write_notification(
      "textDocument/didChange",
      lsp::DidChangeTextDocumentParams {
        text_document: lsp::VersionedTextDocumentIdentifier {
          uri: Url::parse(&file_name).unwrap(),
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
    )?;
  }

  for i in 0..10 {
    let file_name = format!("file:///a/file_{}.ts", i);
    let (maybe_res, maybe_err) = client.write_request::<_, _, Value>(
      "textDocument/formatting",
      lsp::DocumentFormattingParams {
        text_document: lsp::TextDocumentIdentifier {
          uri: Url::parse(&file_name).unwrap(),
        },
        options: lsp::FormattingOptions {
          tab_size: 2,
          insert_spaces: true,
          ..Default::default()
        },
        work_done_progress_params: Default::default(),
      },
    )?;
    assert!(maybe_err.is_none());
    assert!(maybe_res.is_some());
  }

  for _ in 0..3 {
    let (method, _): (String, Option<Value>) = client.read_notification()?;
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  let (_, response_error): (Option<Value>, Option<LspResponseError>) =
    client.write_request("shutdown", json!(null))?;
  assert!(response_error.is_none());

  client.write_notification("exit", json!(null))?;

  Ok(client.duration())
}

/// A test that starts up the LSP, opens a single line document, and exits.
fn bench_startup_shutdown(deno_exe: &Path) -> Result<Duration, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON)?;
  let (_, response_error) =
    client.write_request::<_, _, Value>("initialize", params)?;
  assert!(response_error.is_none());

  client.write_notification("initialized", json!({}))?;

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
  )?;

  let (id, method, _) = client.read_request::<Value>()?;
  assert_eq!(method, "workspace/configuration");

  client.write_response(
    id,
    json!({
      "enable": true
    }),
  )?;

  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");
  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");

  let (_, response_error): (Option<Value>, Option<LspResponseError>) =
    client.write_request("shutdown", json!(null))?;
  assert!(response_error.is_none());

  client.write_notification("exit", json!(null))?;

  Ok(client.duration())
}

/// Generate benchmarks for the LSP server.
pub(crate) fn benchmarks(
  deno_exe: &Path,
) -> Result<HashMap<String, u64>, AnyError> {
  println!("-> Start benchmarking lsp");
  let mut exec_times = HashMap::new();

  println!("   - Simple Startup/Shutdown ");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_startup_shutdown(deno_exe)?);
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("startup_shutdown".to_string(), mean);

  println!("   - Big Document/Several Edits ");
  let mut times = Vec::new();
  for _ in 0..5 {
    times.push(bench_big_file_edits(deno_exe)?);
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("big_file_edits".to_string(), mean);

  println!("   - Find/Replace");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_find_replace(deno_exe)?);
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("find_replace".to_string(), mean);

  println!("   - Code Lens");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_code_lens(deno_exe)?);
  }
  let mean =
    (times.iter().sum::<Duration>() / times.len() as u32).as_millis() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("code_lens".to_string(), mean);

  println!("<- End benchmarking lsp");

  Ok(exec_times)
}
