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
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::time::Instant;

lazy_static! {
  static ref CONTENT_TYPE_REG: Regex =
    Regex::new(r"(?i)^content-length:\s+(\d+)").unwrap();
}

#[derive(Debug, Deserialize)]
struct LspResponse {
  result: Option<Value>,
  id: u32,
  error: Option<LspResponseError>,
}

#[derive(Debug, Deserialize)]
struct LspResponseError {
  code: i32,
  message: String,
  data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LspNotification {
  method: String,
  params: Option<Value>,
}

struct LspClient {
  reader: std::io::BufReader<ChildStdout>,
  child: std::process::Child,
  start: Instant,
  writer: std::io::BufWriter<ChildStdin>,
}

impl Drop for LspClient {
  fn drop(&mut self) {
    match self.child.try_wait() {
      Ok(None) => {
        self.child.kill().unwrap();
        let _ = self.child.wait();
      }
      Ok(Some(status)) => panic!("deno lsp exited unexpectedly {}", status),
      Err(e) => panic!("pebble error: {}", e),
    }
  }
}

impl LspClient {
  fn new(deno_exe: &PathBuf) -> Result<Self, AnyError> {
    let mut child = Command::new(deno_exe)
      .arg("lsp")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let stdin = child.stdin.take().unwrap();
    let writer = std::io::BufWriter::new(stdin);

    Ok(Self {
      child,
      reader,
      start: Instant::now(),
      writer,
    })
  }

  fn duration(&self) -> u128 {
    self.start.elapsed().as_millis()
  }

  fn read<V>(&mut self) -> Result<V, AnyError>
  where
    V: de::DeserializeOwned,
  {
    let mut content_length = 0_usize;
    loop {
      let mut buf = String::new();
      self.reader.read_line(&mut buf)?;
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
    self.reader.read_exact(&mut msg_buf)?;
    let msg = serde_json::from_slice(&msg_buf)?;

    Ok(msg)
  }

  fn write(&mut self, value: Value) -> Result<(), AnyError> {
    let msg = format!(
      "Content-Length: {}\r\n\r\n{}",
      value.to_string().as_bytes().len(),
      value
    );
    self.writer.write_all(msg.as_bytes())?;
    self.writer.flush()?;
    Ok(())
  }
}

pub(crate) fn benchmarks(
  deno_exe: &PathBuf,
) -> Result<HashMap<String, u128>, AnyError> {
  let mut client = LspClient::new(deno_exe)?;

  client.write(json!({
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
  }))?;

  let response: LspResponse = client.read()?;
  assert!(response.error.is_none());

  client.write(json!({
    "jsonrpc": "2.0",
    "method": "initialized",
    "params": {}
  }))?;

  client.write(json!({
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
  }))?;

  let notification: LspNotification = client.read()?;
  assert_eq!(notification.method, "textDocument/publishDiagnostics");

  client.write(json!({
    "jsonrpc": "2.0",
    "id": 2,
    "method": "shutdown",
    "params": null
  }))?;

  let response: LspResponse = client.read()?;
  assert!(response.error.is_none());

  client.write(json!({
    "jsonrpc": "2.0",
    "method": "exit",
    "params": null
  }))?;

  let mut exec_times = HashMap::new();
  exec_times.insert("startup_shutdown".to_string(), client.duration());

  println!("lsp bench: {:?}", exec_times);

  Ok(exec_times)
}
