// Copyright 2018-2025 the Deno authors. MIT license.

use std::future::poll_fn;
use std::future::Future;
use std::pin::Pin;
use std::result::Result;

use anyhow::anyhow;
use bytes::Bytes;
use fastwebsockets::FragmentCollector;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::Role;
use fastwebsockets::WebSocket;
use futures::future::join3;
use futures::StreamExt;
use h2::server::Handshake;
use h2::server::SendResponse;
use h2::Reason;
use h2::RecvStream;
use hyper::upgrade::Upgraded;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper_util::rt::TokioIo;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use super::get_tcp_listener_stream;
use super::get_tls_listener_stream;
use super::SupportedHttpVersions;

pub async fn run_ws_server(port: u16) {
  let mut tcp = get_tcp_listener_stream("ws", port).await;
  while let Some(Ok(stream)) = tcp.next().await {
    spawn_ws_server(stream, |ws| Box::pin(echo_websocket_handler(ws)));
  }
}

pub async fn run_ws_ping_server(port: u16) {
  let mut tcp = get_tcp_listener_stream("ws (ping)", port).await;
  while let Some(Ok(stream)) = tcp.next().await {
    spawn_ws_server(stream, |ws| Box::pin(ping_websocket_handler(ws)));
  }
}

pub async fn run_wss_server(port: u16) {
  let mut tls = get_tls_listener_stream("wss", port, Default::default()).await;
  while let Some(Ok(tls_stream)) = tls.next().await {
    tokio::spawn(async move {
      spawn_ws_server(tls_stream, |ws| Box::pin(echo_websocket_handler(ws)));
    });
  }
}

pub async fn run_ws_close_server(port: u16) {
  let mut tcp = get_tcp_listener_stream("ws (close)", port).await;
  while let Some(Ok(stream)) = tcp.next().await {
    spawn_ws_server(stream, |ws| Box::pin(close_websocket_handler(ws)));
  }
}

pub async fn run_ws_hang_handshake(port: u16) {
  let mut tcp = get_tcp_listener_stream("ws (hang handshake)", port).await;
  while let Some(Ok(mut stream)) = tcp.next().await {
    loop {
      let mut buf = [0; 1024];
      let n = stream.read(&mut buf).await;

      if n.is_err() {
        break;
      }

      if n.unwrap() == 0 {
        break;
      }
    }
  }
}

pub async fn run_wss2_server(port: u16) {
  let mut tls = get_tls_listener_stream(
    "wss2 (tls)",
    port,
    SupportedHttpVersions::Http2Only,
  )
  .await;
  while let Some(Ok(tls)) = tls.next().await {
    tokio::spawn(async move {
      let mut h2 = h2::server::Builder::new();
      h2.enable_connect_protocol();
      // Using Bytes is pretty alloc-heavy but this is a test server
      let server: Handshake<_, Bytes> = h2.handshake(tls);
      let mut server = match server.await {
        Ok(server) => server,
        #[allow(clippy::print_stdout)]
        Err(e) => {
          println!("Failed to handshake h2: {e:?}");
          return;
        }
      };
      loop {
        let Some(conn) = server.accept().await else {
          break;
        };
        let (recv, send) = match conn {
          Ok(conn) => conn,
          #[allow(clippy::print_stdout)]
          Err(e) => {
            println!("Failed to accept a connection: {e:?}");
            break;
          }
        };
        tokio::spawn(handle_wss_stream(recv, send));
      }
    });
  }
}

async fn echo_websocket_handler(
  ws: fastwebsockets::WebSocket<TokioIo<Upgraded>>,
) -> Result<(), anyhow::Error> {
  let mut ws = FragmentCollector::new(ws);

  loop {
    let frame = ws.read_frame().await.unwrap();
    match frame.opcode {
      OpCode::Close => break,
      OpCode::Text | OpCode::Binary => {
        ws.write_frame(frame).await.unwrap();
      }
      _ => {}
    }
  }

  Ok(())
}

type WsHandler =
  fn(
    fastwebsockets::WebSocket<TokioIo<Upgraded>>,
  ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;

fn spawn_ws_server<S>(stream: S, handler: WsHandler)
where
  S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
  let service = hyper::service::service_fn(
    move |mut req: http::Request<hyper::body::Incoming>| async move {
      let (response, upgrade_fut) = fastwebsockets::upgrade::upgrade(&mut req)
        .map_err(|e| anyhow!("Error upgrading websocket connection: {}", e))?;

      tokio::spawn(async move {
        let ws = upgrade_fut
          .await
          .map_err(|e| anyhow!("Error upgrading websocket connection: {}", e))
          .unwrap();

        #[allow(clippy::print_stderr)]
        if let Err(e) = handler(ws).await {
          eprintln!("Error in websocket connection: {}", e);
        }
      });

      Ok::<_, anyhow::Error>(response)
    },
  );

  let io = TokioIo::new(stream);
  tokio::spawn(async move {
    let conn = hyper::server::conn::http1::Builder::new()
      .serve_connection(io, service)
      .with_upgrades();

    #[allow(clippy::print_stderr)]
    if let Err(e) = conn.await {
      eprintln!("websocket server error: {e:?}");
    }
  });
}

async fn handle_wss_stream(
  recv: Request<RecvStream>,
  mut send: SendResponse<Bytes>,
) -> Result<(), h2::Error> {
  #[allow(clippy::print_stderr)]
  if recv.method() != Method::CONNECT {
    eprintln!("wss2: refusing non-CONNECT stream");
    send.send_reset(Reason::REFUSED_STREAM);
    return Ok(());
  }
  #[allow(clippy::print_stderr)]
  let Some(protocol) = recv.extensions().get::<h2::ext::Protocol>() else {
    eprintln!("wss2: refusing no-:protocol stream");
    send.send_reset(Reason::REFUSED_STREAM);
    return Ok(());
  };
  #[allow(clippy::print_stderr)]
  if protocol.as_str() != "websocket" && protocol.as_str() != "WebSocket" {
    eprintln!("wss2: refusing non-websocket stream");
    send.send_reset(Reason::REFUSED_STREAM);
    return Ok(());
  }
  let mut body = recv.into_body();
  let mut response = Response::new(());
  *response.status_mut() = StatusCode::OK;
  let mut resp = send.send_response(response, false)?;
  // Use a duplex stream to talk to fastwebsockets because it's just faster to implement
  let (a, b) = tokio::io::duplex(65536);
  let f1 = tokio::spawn(tokio::task::unconstrained(async move {
    let ws = WebSocket::after_handshake(a, Role::Server);
    let mut ws = FragmentCollector::new(ws);
    loop {
      let frame = ws.read_frame().await.unwrap();
      if frame.opcode == OpCode::Close {
        break;
      }
      ws.write_frame(frame).await.unwrap();
    }
  }));
  let (mut br, mut bw) = tokio::io::split(b);
  let f2 = tokio::spawn(tokio::task::unconstrained(async move {
    loop {
      let Some(Ok(data)) = poll_fn(|cx| body.poll_data(cx)).await else {
        return;
      };
      body.flow_control().release_capacity(data.len()).unwrap();
      let Ok(_) = bw.write_all(&data).await else {
        break;
      };
    }
  }));
  let f3 = tokio::spawn(tokio::task::unconstrained(async move {
    loop {
      let mut buf = [0; 65536];
      let n = br.read(&mut buf).await.unwrap();
      if n == 0 {
        break;
      }
      resp.reserve_capacity(n);
      poll_fn(|cx| resp.poll_capacity(cx)).await;
      resp
        .send_data(Bytes::copy_from_slice(&buf[0..n]), false)
        .unwrap();
    }
    resp.send_data(Bytes::new(), true).unwrap();
  }));
  _ = join3(f1, f2, f3).await;
  Ok(())
}

async fn close_websocket_handler(
  ws: fastwebsockets::WebSocket<TokioIo<Upgraded>>,
) -> Result<(), anyhow::Error> {
  let mut ws = FragmentCollector::new(ws);

  ws.write_frame(Frame::close_raw(vec![].into()))
    .await
    .unwrap();

  Ok(())
}

async fn ping_websocket_handler(
  ws: fastwebsockets::WebSocket<TokioIo<Upgraded>>,
) -> Result<(), anyhow::Error> {
  let mut ws = FragmentCollector::new(ws);

  for i in 0..9 {
    ws.write_frame(Frame::new(true, OpCode::Ping, None, vec![].into()))
      .await
      .unwrap();

    let frame = ws.read_frame().await.unwrap();
    assert_eq!(frame.opcode, OpCode::Pong);
    assert!(frame.payload.is_empty());

    ws.write_frame(Frame::text(
      format!("hello {}", i).as_bytes().to_vec().into(),
    ))
    .await
    .unwrap();

    let frame = ws.read_frame().await.unwrap();
    assert_eq!(frame.opcode, OpCode::Text);
    assert_eq!(frame.payload, format!("hello {}", i).as_bytes());
  }

  ws.write_frame(Frame::close(1000, b"")).await.unwrap();

  Ok(())
}
