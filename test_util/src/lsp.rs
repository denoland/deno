// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::new_deno_dir;

use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use serde::de;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::collections::VecDeque;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;

lazy_static! {
  static ref CONTENT_TYPE_REG: Regex =
    Regex::new(r"(?i)^content-length:\s+(\d+)").unwrap();
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LspResponseError {
  code: i32,
  message: String,
  data: Option<Value>,
}

#[derive(Debug)]
pub enum LspMessage {
  Notification(String, Option<Value>),
  Request(u64, String, Option<Value>),
  Response(u64, Option<Value>, Option<LspResponseError>),
}

impl<'a> From<&'a [u8]> for LspMessage {
  fn from(s: &'a [u8]) -> Self {
    let value: Value = serde_json::from_slice(s).unwrap();
    let obj = value.as_object().unwrap();
    if obj.contains_key("id") && obj.contains_key("method") {
      let id = obj.get("id").unwrap().as_u64().unwrap();
      let method = obj.get("method").unwrap().as_str().unwrap().to_string();
      Self::Request(id, method, obj.get("params").cloned())
    } else if obj.contains_key("id") {
      let id = obj.get("id").unwrap().as_u64().unwrap();
      let maybe_error: Option<LspResponseError> = obj
        .get("error")
        .map(|v| serde_json::from_value(v.clone()).unwrap());
      Self::Response(id, obj.get("result").cloned(), maybe_error)
    } else {
      assert!(obj.contains_key("method"));
      let method = obj.get("method").unwrap().as_str().unwrap().to_string();
      Self::Notification(method, obj.get("params").cloned())
    }
  }
}

fn read_message<R>(reader: &mut R) -> Result<Vec<u8>>
where
  R: io::Read + io::BufRead,
{
  let mut content_length = 0_usize;
  loop {
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    if let Some(captures) = CONTENT_TYPE_REG.captures(&buf) {
      let content_length_match = captures
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing capture"))?;
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

pub struct LspClient {
  child: Child,
  reader: io::BufReader<ChildStdout>,
  /// Used to hold pending messages that have come out of the expected sequence
  /// by the harness user which will be sent first when trying to consume a
  /// message before attempting to read a new message.
  msg_queue: VecDeque<LspMessage>,
  request_id: u64,
  start: Instant,
  writer: io::BufWriter<ChildStdin>,
  _temp_deno_dir: TempDir, // directory will be deleted on drop
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

fn notification_result<R>(
  method: String,
  maybe_params: Option<Value>,
) -> Result<(String, Option<R>)>
where
  R: de::DeserializeOwned,
{
  let maybe_params = match maybe_params {
    Some(params) => Some(serde_json::from_value(params)?),
    None => None,
  };
  Ok((method, maybe_params))
}

fn request_result<R>(
  id: u64,
  method: String,
  maybe_params: Option<Value>,
) -> Result<(u64, String, Option<R>)>
where
  R: de::DeserializeOwned,
{
  let maybe_params = match maybe_params {
    Some(params) => Some(serde_json::from_value(params)?),
    None => None,
  };
  Ok((id, method, maybe_params))
}

fn response_result<R>(
  maybe_result: Option<Value>,
  maybe_error: Option<LspResponseError>,
) -> Result<(Option<R>, Option<LspResponseError>)>
where
  R: de::DeserializeOwned,
{
  let maybe_result = match maybe_result {
    Some(result) => Some(serde_json::from_value(result)?),
    None => None,
  };
  Ok((maybe_result, maybe_error))
}

impl LspClient {
  pub fn new(deno_exe: &Path) -> Result<Self> {
    let deno_dir = new_deno_dir();
    let mut child = Command::new(deno_exe)
      .env("DENO_DIR", deno_dir.path())
      .arg("lsp")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = io::BufReader::new(stdout);

    let stdin = child.stdin.take().unwrap();
    let writer = io::BufWriter::new(stdin);

    Ok(Self {
      child,
      msg_queue: VecDeque::new(),
      reader,
      request_id: 1,
      start: Instant::now(),
      writer,
      _temp_deno_dir: deno_dir,
    })
  }

  pub fn duration(&self) -> Duration {
    self.start.elapsed()
  }

  pub fn queue_is_empty(&self) -> bool {
    self.msg_queue.is_empty()
  }

  pub fn queue_len(&self) -> usize {
    self.msg_queue.len()
  }

  fn read(&mut self) -> Result<LspMessage> {
    let msg_buf = read_message(&mut self.reader)?;
    let msg = LspMessage::from(msg_buf.as_slice());
    Ok(msg)
  }

  pub fn read_notification<R>(&mut self) -> Result<(String, Option<R>)>
  where
    R: de::DeserializeOwned,
  {
    if !self.msg_queue.is_empty() {
      let mut msg_queue = VecDeque::new();
      loop {
        match self.msg_queue.pop_front() {
          Some(LspMessage::Notification(method, maybe_params)) => {
            return notification_result(method, maybe_params)
          }
          Some(msg) => msg_queue.push_back(msg),
          _ => break,
        }
      }
      self.msg_queue = msg_queue;
    }

    loop {
      match self.read() {
        Ok(LspMessage::Notification(method, maybe_params)) => {
          return notification_result(method, maybe_params)
        }
        Ok(msg) => self.msg_queue.push_back(msg),
        Err(err) => return Err(err),
      }
    }
  }

  pub fn read_request<R>(&mut self) -> Result<(u64, String, Option<R>)>
  where
    R: de::DeserializeOwned,
  {
    if !self.msg_queue.is_empty() {
      let mut msg_queue = VecDeque::new();
      loop {
        match self.msg_queue.pop_front() {
          Some(LspMessage::Request(id, method, maybe_params)) => {
            return request_result(id, method, maybe_params)
          }
          Some(msg) => msg_queue.push_back(msg),
          _ => break,
        }
      }
      self.msg_queue = msg_queue;
    }

    loop {
      match self.read() {
        Ok(LspMessage::Request(id, method, maybe_params)) => {
          return request_result(id, method, maybe_params)
        }
        Ok(msg) => self.msg_queue.push_back(msg),
        Err(err) => return Err(err),
      }
    }
  }

  fn write(&mut self, value: Value) -> Result<()> {
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

  pub fn write_request<S, V, R>(
    &mut self,
    method: S,
    params: V,
  ) -> Result<(Option<R>, Option<LspResponseError>)>
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

    loop {
      match self.read() {
        Ok(LspMessage::Response(id, maybe_result, maybe_error)) => {
          assert_eq!(id, self.request_id);
          self.request_id += 1;
          return response_result(maybe_result, maybe_error);
        }
        Ok(msg) => self.msg_queue.push_back(msg),
        Err(err) => return Err(err),
      }
    }
  }

  pub fn write_response<V>(&mut self, id: u64, result: V) -> Result<()>
  where
    V: Serialize,
  {
    let value = json!({
      "jsonrpc": "2.0",
      "id": id,
      "result": result
    });
    self.write(value)
  }

  pub fn write_notification<S, V>(&mut self, method: S, params: V) -> Result<()>
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_read_message() {
    let msg = b"content-length: 11\r\n\r\nhello world";
    let mut reader = std::io::Cursor::new(msg);
    assert_eq!(read_message(&mut reader).unwrap(), b"hello world");
  }
}
