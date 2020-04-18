// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use crate::futures::SinkExt;
use crate::futures::StreamExt;
use crate::tokio_util;
use deno_core::ErrBox;
use serde::Deserialize;
use std;
use tokio_tungstenite;
use url::Url;

pub struct CoverageCollector {
  msg_id: usize,
  socket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
}

impl CoverageCollector {
  pub async fn connect(url: Url) -> Result<Self, ErrBox> {
    let (socket, response) = tokio_tungstenite::connect_async(url)
      .await
      .expect("Can't connect");
    assert_eq!(response.status(), 101);

    let mut collector = Self { msg_id: 1, socket };

    eprintln!("start");
    collector
      .socket
      .send(r#"{"id":1,"method":"Runtime.enable"}"#.into())
      .await
      .unwrap();
    collector
      .socket
      .send(r#"{"id":2,"method":"Profiler.enable"}"#.into())
      .await
      .unwrap();
    collector.socket.send(r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": false, "detailed": true } }"#.into()).await.unwrap();
    collector
      .socket
      .send(r#"{"id":4,"method":"Runtime.runIfWaitingForDebugger" }"#.into())
      .await
      .unwrap();
    eprintln!("start1");

    Ok(collector)
  }

  pub async fn stop_collecting(&mut self) -> Result<(), ErrBox> {
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    eprintln!("start2");
    dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);

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

  pub async fn get_report(&mut self) -> Result<Vec<CoverageResult>, ErrBox> {
    dbg!("before recv");
    let msg = self.socket.next().await.unwrap();
    dbg!("after recv");
    let msg = msg.unwrap();
    let msg_text = msg.to_text()?;

    let coverage_result: CoverageResultMsg =
      serde_json::from_str(msg_text).unwrap();
    // eprintln!("cover result {:#?}", coverage_result);

    // dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);

    Ok(coverage_result.result.result)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
  pub start_offset: usize,
  pub end_offset: usize,
  pub count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverageResult {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageResult {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverageResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Res {
  result: Vec<CoverageResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoverageResultMsg {
  id: usize,
  result: Res,
}
