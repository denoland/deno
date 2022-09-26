// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use futures_util::SinkExt;
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let url = url::Url::parse("ws://localhost:8000").unwrap();
  let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

  let start = std::time::Instant::now();
  let mut count = 0;
  let mut bytes = 0;
  loop {
    ws_stream
      .send(Message::Text("hello".to_string()))
      .await
      .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    if let Message::Text(data) = msg {
      count += 1;
      bytes += data.len();
      if start.elapsed().as_secs() > 1 {
        println!(
          "Sent {} messages in 1 sec, throughput: {} bytes/sec",
          count, bytes
        );
        count = 0;
        bytes = 0;
        break;
      }
    }
  }
}
