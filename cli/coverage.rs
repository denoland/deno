// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use crate::colors;
use crate::file_fetcher::SourceFile;
use crate::futures::SinkExt;
use crate::futures::StreamExt;
use crate::tokio_util;
use deno_core::ErrBox;
use serde::Deserialize;
use serde::Serialize;
use sourcemap::SourceMap;
use url::Url;

pub struct CoverageCollector {
  socket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
}

// TODO(caspervonb) do not hard-code message ids.
// TODO yield then await each command response after sending the request.
impl CoverageCollector {
  pub async fn connect(url: Url) -> Result<Self, ErrBox> {
    let (socket, response) = tokio_tungstenite::connect_async(url)
      .await
      .expect("Can't connect");
    assert_eq!(response.status(), 101);

    Ok(Self { socket })
  }

  pub async fn start_collecting(&mut self) -> Result<(), ErrBox> {
    self
      .socket
      .send(r#"{"id":1,"method":"Runtime.enable"}"#.into())
      .await
      .unwrap();

    self
      .socket
      .send(r#"{"id":2,"method":"Profiler.enable"}"#.into())
      .await
      .unwrap();

    self
          .socket
          .send(r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": true, "detailed": true}}"#.into()).await.unwrap();

    self
      .socket
      .send(r#"{"id":4,"method":"Runtime.runIfWaitingForDebugger"}"#.into())
      .await
      .unwrap();

    Ok(())
  }

  pub async fn stop_collecting(&mut self) -> Result<(), ErrBox> {
    self.socket.next().await.unwrap();
    self.socket.next().await.unwrap();
    self.socket.next().await.unwrap();
    self.socket.next().await.unwrap();
    self.socket.next().await.unwrap();

    self
      .socket
      .send(r#"{"id":5,"method":"Profiler.takePreciseCoverage" }"#.into())
      .await
      .unwrap();

    self
      .socket
      .send(r#"{"id":6,"method":"Profiler.stopPreciseCoverage" }"#.into())
      .await
      .unwrap();

    Ok(())
  }

  pub async fn take_precise_coverage(
    &mut self,
  ) -> Result<(Vec<ScriptCoverage>), ErrBox> {
    let msg = self.socket.next().await.unwrap();
    let msg = msg.unwrap();
    let msg_text = msg.to_text()?;

    let coverage_result: TakePreciseCoverageResponse =
      serde_json::from_str(msg_text).unwrap();

    self.socket.next().await.unwrap();

    Ok(coverage_result.result.result)
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
  pub start_offset: usize,
  pub end_offset: usize,
  pub count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageResult {
  result: Vec<ScriptCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageResponse {
  id: usize,
  result: TakePreciseCoverageResult,
}

pub struct PrettyCoverageReporter {}

impl PrettyCoverageReporter {
  pub fn new() -> PrettyCoverageReporter {
    PrettyCoverageReporter {}
  }

  pub fn visit(
    &mut self,
    script_coverage: &ScriptCoverage,
    source_file: &SourceFile,
  ) {
    let mut total_lines = 0;
    let mut covered_lines = 0;

    let mut line_offset = 0;
    let source_string = source_file.source_code.to_string().unwrap();

    for line in source_string.lines() {
      let line_start_offset = line_offset;
      let line_end_offset = line_start_offset + line.len();

      let mut count = 1;
      for function in &script_coverage.functions {
        for range in &function.ranges {
          if range.start_offset <= line_start_offset
            && range.end_offset >= line_end_offset
          {
            count = range.count;
          }
        }
      }

      if count > 0 {
        covered_lines += 1;
      }

      total_lines += 1;
      line_offset += line.len();
    }

    let line_ratio = (covered_lines as f32 / total_lines as f32);
    let line_coverage = format!("{:.3}%", line_ratio * 100.0);

    if line_ratio >= 0.9 {
      println!(
        "{} {}",
        source_file.url.to_string(),
        colors::green(&line_coverage)
      );
    } else if line_ratio >= 0.75 {
      println!(
        "{} {}",
        source_file.url.to_string(),
        colors::gray(&line_coverage)
      );
    } else {
      println!(
        "{} {}",
        source_file.url.to_string(),
        colors::red(&line_coverage)
      );
    }
  }
}
