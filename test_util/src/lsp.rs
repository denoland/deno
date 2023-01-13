// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::npm_registry_url;
use crate::std_file_url;

use super::new_deno_dir;
use super::TempDir;

use anyhow::Result;
use lazy_static::lazy_static;
use parking_lot::Condvar;
use parking_lot::Mutex;
use regex::Regex;
use serde::de;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::{json, to_value};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

lazy_static! {
  static ref CONTENT_TYPE_REG: Regex =
    Regex::new(r"(?i)^content-length:\s+(\d+)").unwrap();
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LspResponseError {
  code: i32,
  message: String,
  data: Option<Value>,
}

#[derive(Clone, Debug)]
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

fn read_message<R>(reader: &mut R) -> Result<Option<Vec<u8>>>
where
  R: io::Read + io::BufRead,
{
  let mut content_length = 0_usize;
  loop {
    let mut buf = String::new();
    if reader.read_line(&mut buf)? == 0 {
      return Ok(None);
    }
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
  Ok(Some(msg_buf))
}

struct LspStdoutReader {
  pending_messages: Arc<(Mutex<Vec<LspMessage>>, Condvar)>,
  read_messages: Vec<LspMessage>,
}

impl LspStdoutReader {
  pub fn new(mut buf_reader: io::BufReader<ChildStdout>) -> Self {
    let messages: Arc<(Mutex<Vec<LspMessage>>, Condvar)> = Default::default();
    std::thread::spawn({
      let messages = messages.clone();
      move || {
        while let Ok(Some(msg_buf)) = read_message(&mut buf_reader) {
          let msg = LspMessage::from(msg_buf.as_slice());
          let cvar = &messages.1;
          {
            let mut messages = messages.0.lock();
            messages.push(msg);
          }
          cvar.notify_all();
        }
      }
    });

    LspStdoutReader {
      pending_messages: messages,
      read_messages: Vec::new(),
    }
  }

  pub fn pending_len(&self) -> usize {
    self.pending_messages.0.lock().len()
  }

  pub fn had_message(&self, is_match: impl Fn(&LspMessage) -> bool) -> bool {
    self.read_messages.iter().any(&is_match)
      || self.pending_messages.0.lock().iter().any(&is_match)
  }

  pub fn read_message<R>(
    &mut self,
    mut get_match: impl FnMut(&LspMessage) -> Option<R>,
  ) -> R {
    let (msg_queue, cvar) = &*self.pending_messages;
    let mut msg_queue = msg_queue.lock();
    loop {
      for i in 0..msg_queue.len() {
        let msg = &msg_queue[i];
        if let Some(result) = get_match(msg) {
          let msg = msg_queue.remove(i);
          self.read_messages.push(msg);
          return result;
        }
      }
      cvar.wait(&mut msg_queue);
    }
  }
}

pub struct LspClient {
  child: Child,
  reader: LspStdoutReader,
  request_id: u64,
  start: Instant,
  writer: io::BufWriter<ChildStdin>,
  deno_dir: TempDir,
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
    Some(params) => {
      Some(serde_json::from_value(params.clone()).map_err(|err| {
        anyhow::anyhow!(
          "Could not deserialize message '{}': {}\n\n{:?}",
          method,
          err,
          params
        )
      })?)
    }
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
  pub fn new(deno_exe: &Path, print_stderr: bool) -> Result<Self> {
    let deno_dir = new_deno_dir();
    let mut command = Command::new(deno_exe);
    command
      .env("DENO_DIR", deno_dir.path())
      .env("DENO_NODE_COMPAT_URL", std_file_url())
      .env("NPM_CONFIG_REGISTRY", npm_registry_url())
      .arg("lsp")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped());
    if !print_stderr {
      command.stderr(Stdio::null());
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let buf_reader = io::BufReader::new(stdout);
    let reader = LspStdoutReader::new(buf_reader);

    let stdin = child.stdin.take().unwrap();
    let writer = io::BufWriter::new(stdin);

    Ok(Self {
      child,
      reader,
      request_id: 1,
      start: Instant::now(),
      writer,
      deno_dir,
    })
  }

  pub fn deno_dir(&self) -> &TempDir {
    &self.deno_dir
  }

  pub fn duration(&self) -> Duration {
    self.start.elapsed()
  }

  pub fn queue_is_empty(&self) -> bool {
    self.reader.pending_len() == 0
  }

  pub fn queue_len(&self) -> usize {
    self.reader.pending_len()
  }

  // it's flaky to assert for a notification because a notification
  // might arrive a little later, so only provide a method for asserting
  // that there is no notification
  pub fn assert_no_notification(&mut self, searching_method: &str) {
    assert!(!self.reader.had_message(|message| match message {
      LspMessage::Notification(method, _) => method == searching_method,
      _ => false,
    }))
  }

  pub fn read_notification<R>(&mut self) -> Result<(String, Option<R>)>
  where
    R: de::DeserializeOwned,
  {
    self.reader.read_message(|msg| match msg {
      LspMessage::Notification(method, maybe_params) => Some(
        notification_result(method.to_owned(), maybe_params.to_owned()),
      ),
      _ => None,
    })
  }

  pub fn read_request<R>(&mut self) -> Result<(u64, String, Option<R>)>
  where
    R: de::DeserializeOwned,
  {
    self.reader.read_message(|msg| match msg {
      LspMessage::Request(id, method, maybe_params) => Some(request_result(
        *id,
        method.to_owned(),
        maybe_params.to_owned(),
      )),
      _ => None,
    })
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
    let value = if to_value(&params).unwrap().is_null() {
      json!({
        "jsonrpc": "2.0",
        "id": self.request_id,
        "method": method.as_ref(),
      })
    } else {
      json!({
        "jsonrpc": "2.0",
        "id": self.request_id,
        "method": method.as_ref(),
        "params": params,
      })
    };
    self.write(value)?;

    self.reader.read_message(|msg| match msg {
      LspMessage::Response(id, maybe_result, maybe_error) => {
        assert_eq!(*id, self.request_id);
        self.request_id += 1;
        Some(response_result(
          maybe_result.to_owned(),
          maybe_error.to_owned(),
        ))
      }
      _ => None,
    })
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
    let msg1 = b"content-length: 11\r\n\r\nhello world";
    let mut reader1 = std::io::Cursor::new(msg1);
    assert_eq!(read_message(&mut reader1).unwrap().unwrap(), b"hello world");

    let msg2 = b"content-length: 5\r\n\r\nhello world";
    let mut reader2 = std::io::Cursor::new(msg2);
    assert_eq!(read_message(&mut reader2).unwrap().unwrap(), b"hello");
  }

  #[test]
  #[should_panic(expected = "failed to fill whole buffer")]
  fn test_invalid_read_message() {
    let msg1 = b"content-length: 12\r\n\r\nhello world";
    let mut reader1 = std::io::Cursor::new(msg1);
    read_message(&mut reader1).unwrap();
  }
}
