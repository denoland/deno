// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use test_util::lsp::LspClient;
use test_util::lsp::LspResponseError;

static FIXTURE_DB_TS: &str = include_str!("fixtures/db.ts");
static FIXTURE_DB_MESSAGES: &[u8] = include_bytes!("fixtures/db_messages.json");
static FIXTURE_INIT_JSON: &[u8] =
  include_bytes!("fixtures/initialize_params.json");

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
        "uri": "file:///fixtures/db.ts",
        "languageId": "typescript",
        "version": 1,
        "text": FIXTURE_DB_TS
      }
    }),
  )?;

  // TODO(@kitsonk) work around https://github.com/denoland/deno/issues/10603
  // let (id, method, _): (u64, String, Option<Value>) = client.read_request()?;
  // assert_eq!(method, "workspace/configuration");

  // client.write_response(
  //   id,
  //   json!({
  //     "enable": true
  //   }),
  // )?;

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

/// A test that starts up the LSP, opens a single line document, and exits.
fn bench_startup_shutdown(deno_exe: &Path) -> Result<Duration, AnyError> {
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
        "uri": "file:///a/file.ts",
        "languageId": "typescript",
        "version": 1,
        "text": "console.log(Deno.args);\n"
      }
    }),
  )?;

  // TODO(@kitsonk) work around https://github.com/denoland/deno/issues/10603
  // let (id, method, _): (u64, String, Option<Value>) = client.read_request()?;
  // assert_eq!(method, "workspace/configuration");

  // client.write_response(
  //   id,
  //   json!({
  //     "enable": true
  //   }),
  // )?;

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

  println!("<- End benchmarking lsp");

  Ok(exec_times)
}
