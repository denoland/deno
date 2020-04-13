// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use crate::tokio_util;
use deno_core::ErrBox;
use std;
use tokio_tungstenite;
use url::Url;
use crate::futures::StreamExt;
use crate::futures::SinkExt;

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

    let mut collector = Self {
        msg_id: 1,
        socket,
    };

    eprintln!("start");
    collector.socket.send(r#"{"id":1,"method":"Runtime.enable"}"#.into()).await.unwrap();
    collector.socket.send(r#"{"id":2,"method":"Profiler.enable"}"#.into()).await.unwrap();
    collector.socket.send(r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": false, "detailed": true } }"#.into()).await.unwrap();
    collector.socket.send(r#"{"id":4,"method":"Runtime.runIfWaitingForDebugger" }"#.into()).await.unwrap();
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

    self.socket.send(r#"{"id":5,"method":"Profiler.takePreciseCoverage" }"#.into()).await.unwrap();
    self.socket.send(r#"{"id":6,"method":"Profiler.stopPreciseCoverage" }"#.into()).await.unwrap();
    Ok(())
  }

  pub async fn get_report(&mut self) -> Result<String, ErrBox> {
    dbg!("before recv");
    let msg = self.socket.next().await.unwrap();
    dbg!("after recv");
    dbg!(msg);
    let msg = self.socket.next().await.unwrap();
    dbg!(msg);

    todo!()
  }
}
