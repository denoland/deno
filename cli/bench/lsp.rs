// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::io::Lines;
use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::runtime;
use tokio::time::Instant;

lazy_static! {
  static ref CONTENT_TYPE_REG: Regex =
    Regex::new(r"(?i)^content-length:\s+(\d+)").unwrap();
}

#[derive(Debug, Deserialize)]
struct LspResponse {
  pub result: Option<Value>,
  pub id: u32,
  pub error: Option<LspResponseError>,
}

#[derive(Debug, Deserialize)]
struct LspResponseError {
  pub code: i32,
  pub message: String,
  pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LspNotification {
  pub method: String,
  pub params: Option<Value>,
}

fn create_basic_runtime() -> runtime::Runtime {
  runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    // for performance purposes, we might as well allow this to go to as
    // many threads as it can, so that the harness doesn't become the bottle
    // neck
    // .max_blocking_threads(32)
    .build()
    .unwrap()
}

pub(crate) fn run_local<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_basic_runtime();
  rt.block_on(future)
}

struct LspClient {
  pub output: Lines<BufReader<ChildStderr>>,
  reader: BufReader<ChildStdout>,
  start: Instant,
  writer: BufWriter<ChildStdin>,
}

impl LspClient {
  pub fn new(deno_exe: &PathBuf) -> Result<Self, AnyError> {
    let mut child = Command::new(deno_exe)
      .kill_on_drop(true)
      .args(&["lsp"])
      .stderr(Stdio::piped())
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()?;

    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| generic_error("could not get handle to stdout"))?;
    let reader = BufReader::new(stdout);

    let stdin = child
      .stdin
      .take()
      .ok_or_else(|| generic_error("could not get handle to stdin"))?;
    let writer = BufWriter::new(stdin);

    let stderr = child
      .stderr
      .take()
      .ok_or_else(|| generic_error("could not get handle to stderr"))?;
    let output = BufReader::new(stderr).lines();

    tokio::spawn(async move {
      child.wait().await.expect("could not exit properly");
    });

    Ok(Self {
      output,
      reader,
      start: Instant::now(),
      writer,
    })
  }

  pub fn duration(&self) -> u128 {
    self.start.elapsed().as_millis()
  }

  pub async fn read<V>(&mut self) -> Result<V, AnyError>
  where
    V: de::DeserializeOwned,
  {
    let mut content_length = 0_usize;
    loop {
      let mut buf = String::new();
      self.reader.read_line(&mut buf).await?;
      if let Some(captures) = CONTENT_TYPE_REG.captures(&buf) {
        let content_length_match = captures
          .get(1)
          .ok_or_else(|| generic_error("missing capture"))?;
        content_length = content_length_match.as_str().parse::<usize>()?;
      }
      if &buf == "\r\n" {
        break;
      }
    }

    let mut msg_buf = vec![0_u8; content_length];
    let bytes_read = self.reader.read(&mut msg_buf).await?;
    assert_eq!(bytes_read, content_length);
    let msg = serde_json::from_slice(&msg_buf)?;

    Ok(msg)
  }

  pub async fn write(&mut self, value: Value) -> Result<(), AnyError> {
    let msg = format!(
      "Content-Length: {}\r\n\r\n{}",
      value.to_string().as_bytes().len(),
      value
    );
    self.writer.write_all(msg.as_bytes()).await?;
    self.writer.flush().await.map_err(|e| e.into())
  }
}

pub(crate) async fn benchmarks(
  deno_exe: &PathBuf,
) -> Result<HashMap<String, u128>, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  client
    .write(json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "initialize",
      "params": {
        "processId": 0,
        "clientInfo": {
          "name": "test-harness",
          "version": "1.0.0"
        },
        "rootUri": null,
        "initializationOptions": {
          "enable": true,
          "codeLens": {
            "implementations": true,
            "references": true
          },
          "lint": true,
          "importMap": null,
          "unstable": false
        },
        "capabilities": {
          "textDocument": {
            "codeAction": {
              "codeActionLiteralSupport": {
                "codeActionKind": {
                  "valueSet": [
                    "quickfix"
                  ]
                }
              },
              "isPreferredSupport": true,
              "dataSupport": true,
              "resolveSupport": {
                "properties": [
                  "edit"
                ]
              }
            },
            "synchronization": {
              "dynamicRegistration": true,
              "willSave": true,
              "willSaveWaitUntil": true,
              "didSave": true
            }
          }
        }
      }
    }))
    .await?;

  let response: LspResponse = client.read().await?;
  assert!(response.error.is_none());

  client
    .write(json!({
      "jsonrpc": "2.0",
      "method": "initialized",
      "params": {}
    }))
    .await?;

  client
    .write(json!({
      "jsonrpc": "2.0",
      "method": "textDocument/didOpen",
      "params": {
        "textDocument": {
          "uri": "file:///a/file.ts",
          "languageId": "typescript",
          "version": 1,
          "text": "console.log(Deno.args);\n"
        }
      }
    }))
    .await?;

  let notification: LspNotification = client.read().await?;
  assert_eq!(notification.method, "textDocument/publishDiagnostics");

  client
    .write(json!({
      "jsonrpc": "2.0",
      "id": 2,
      "method": "shutdown",
      "params": null
    }))
    .await?;

  let response: LspResponse = client.read().await?;
  assert!(response.error.is_none());

  client
    .write(json!({
      "jsonrpc": "2.0",
      "method": "exit",
      "params": null
    }))
    .await?;

  let mut exec_times = HashMap::new();
  exec_times.insert("startup_shutdown".to_string(), client.duration());

  println!("lsp bench: {:?}", exec_times);

  Ok(exec_times)
}
