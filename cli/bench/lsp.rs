// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
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
use std::time::Duration;
use std::time::Instant;

static FIXTURE_DB_TS: &str = include_str!("fixtures/db.ts");
static FIXTURE_EDITS_JSON: &[u8] = include_bytes!("fixtures/edits.json");
static FIXTURE_INIT_JSON: &[u8] =
  include_bytes!("fixtures/initialize_params.json");

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
  request_id: u32,
  start: Instant,
  writer: std::io::BufWriter<ChildStdin>,
}

fn read_lsp_message<R>(reader: &mut R) -> Result<Vec<u8>, AnyError>
where
  R: Read + BufRead,
{
  let mut content_length = 0_usize;
  loop {
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
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
  reader.read_exact(&mut msg_buf)?;
  Ok(msg_buf)
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
      .stderr(Stdio::null())
      .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let stdin = child.stdin.take().unwrap();
    let writer = std::io::BufWriter::new(stdin);

    Ok(Self {
      child,
      reader,
      request_id: 1,
      start: Instant::now(),
      writer,
    })
  }

  fn duration(&self) -> Duration {
    self.start.elapsed()
  }

  fn read<V>(&mut self) -> Result<V, AnyError>
  where
    V: de::DeserializeOwned,
  {
    let msg_buf = read_lsp_message(&mut self.reader)?;
    let msg = serde_json::from_slice(&msg_buf)?;
    Ok(msg)
  }

  fn read_notification<R>(&mut self) -> Result<(String, Option<R>), AnyError>
  where
    R: de::DeserializeOwned,
  {
    let notification: LspNotification = self.read()?;
    if let Some(params) = notification.params {
      let p = serde_json::from_value(params)?;
      Ok((notification.method, Some(p)))
    } else {
      Ok((notification.method, None))
    }
  }

  fn write(&mut self, value: Value) -> Result<(), AnyError> {
    let value_str = value.to_string();
    let msg = format!(
      "Content-Length: {}\r\n\r\n{}",
      value_str.as_bytes().len(),
      value_str
    );
    self.writer.write_all(msg.as_bytes())?;
    self.writer.flush()?;
    Ok(())
  }

  fn write_request<S, V, R>(
    &mut self,
    method: S,
    params: V,
  ) -> Result<(Option<R>, Option<LspResponseError>), AnyError>
  where
    S: AsRef<str>,
    V: Serialize,
    R: de::DeserializeOwned,
  {
    let value = json!({
      "jsonrpc": "2.0",
      "id": self.request_id,
      "method": method.as_ref(),
      "params": params,
    });
    self.write(value)?;
    let response: LspResponse = self.read()?;
    assert_eq!(response.id, self.request_id);
    self.request_id += 1;
    if let Some(result) = response.result {
      let r = serde_json::from_value(result)?;
      Ok((Some(r), response.error))
    } else {
      Ok((None, response.error))
    }
  }

  fn write_notification<S, V>(
    &mut self,
    method: S,
    params: V,
  ) -> Result<(), AnyError>
  where
    S: AsRef<str>,
    V: Serialize,
  {
    let value = json!({
      "jsonrpc": "2.0",
      "method": method.as_ref(),
      "params": params,
    });
    self.write(value)?;
    Ok(())
  }
}

fn bench_big_file_edits(deno_exe: &PathBuf) -> Result<Duration, AnyError> {
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

  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");

  let edits: Vec<Value> = serde_json::from_slice(FIXTURE_EDITS_JSON)?;

  for (i, edit) in edits.iter().enumerate() {
    client.write_notification("textDocument/didChange", edit)?;
    std::thread::sleep(Duration::from_millis(150)); // average typing rate of someone who touch types
    if i == 16 || i == 23 {
      // some cognitive delay when typing
      std::thread::sleep(Duration::from_millis(250));
    }
  }

  // there should be at least 3 diagnostic publishes
  // TODO(kitsonk) figure out a way to drain any notifications without blocking
  for _ in 0..3 {
    let (method, _): (String, Option<Value>) = client.read_notification()?;
    assert_eq!(method, "textDocument/publishDiagnostics");
  }

  Ok(client.duration())
}

fn bench_startup_shutdown(deno_exe: &PathBuf) -> Result<Duration, AnyError> {
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

  let (method, _): (String, Option<Value>) = client.read_notification()?;
  assert_eq!(method, "textDocument/publishDiagnostics");

  let (_, response_error): (Option<Value>, Option<LspResponseError>) =
    client.write_request("shutdown", json!(null))?;
  assert!(response_error.is_none());

  client.write_notification("exit", json!(null))?;

  Ok(client.duration())
}

pub(crate) fn benchmarks(
  deno_exe: &PathBuf,
) -> Result<HashMap<String, u64>, AnyError> {
  println!("-> Start benchmarking lsp");
  let mut exec_times = HashMap::new();

  println!("   - Simple Startup/Shutdown ");
  let mut times = Vec::new();
  for _ in 0..10 {
    times.push(bench_startup_shutdown(deno_exe)?);
  }
  let mean: u64 =
    times.iter().sum::<Duration>().as_millis() as u64 / times.len() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("startup_shutdown".to_string(), mean);

  println!("   - Big Document/Several Edits ");
  let mut times = Vec::new();
  for _ in 0..5 {
    times.push(bench_big_file_edits(deno_exe)?);
  }
  let mean: u64 =
    times.iter().sum::<Duration>().as_millis() as u64 / times.len() as u64;
  println!("      ({} runs, mean: {}ms)", times.len(), mean);
  exec_times.insert("big_file_edits".to_string(), mean);
  println!("");

  println!("<- End benchmarking lsp");

  Ok(exec_times)
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_read_lsp_message() {
    let msg = b"content-length: 11\r\n\r\nhello world";
    let reader = std::io::Cursor::new(msg);
    assert_eq!(read_lsp_message(reader).unwrap(), b"hello world");
  }
}
