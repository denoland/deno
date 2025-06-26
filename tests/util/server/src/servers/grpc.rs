// Copyright 2018-2025 the Deno authors. MIT license.

use futures::StreamExt;
use h2;
use hyper::header::HeaderName;
use hyper::header::HeaderValue;
use rustls_tokio_stream::TlsStream;
use tokio::net::TcpStream;
use tokio::task::LocalSet;

use super::get_tcp_listener_stream;
use super::get_tls_listener_stream;
use super::SupportedHttpVersions;

pub async fn h2_grpc_server(h2_grpc_port: u16, h2s_grpc_port: u16) {
  let mut tcp = get_tcp_listener_stream("grpc", h2_grpc_port).await;
  let mut tls = get_tls_listener_stream(
    "grpc (tls)",
    h2s_grpc_port,
    SupportedHttpVersions::Http2Only,
  )
  .await;

  async fn serve(socket: TcpStream) -> Result<(), anyhow::Error> {
    let mut connection = h2::server::handshake(socket).await?;

    while let Some(result) = connection.accept().await {
      let (request, respond) = result?;
      tokio::spawn(async move {
        let _ = handle_request(request, respond).await;
      });
    }

    Ok(())
  }

  async fn serve_tls(
    socket: TlsStream<TcpStream>,
  ) -> Result<(), anyhow::Error> {
    let mut connection = h2::server::handshake(socket).await?;

    while let Some(result) = connection.accept().await {
      let (request, respond) = result?;
      tokio::spawn(async move {
        let _ = handle_request(request, respond).await;
      });
    }

    Ok(())
  }

  async fn handle_request(
    mut request: hyper::Request<h2::RecvStream>,
    mut respond: h2::server::SendResponse<bytes::Bytes>,
  ) -> Result<(), anyhow::Error> {
    let body = request.body_mut();
    while let Some(data) = body.data().await {
      let data = data?;
      let _ = body.flow_control().release_capacity(data.len());
    }

    let maybe_recv_trailers = body.trailers().await?;

    let response = hyper::Response::new(());
    let mut send = respond.send_response(response, false)?;
    send.send_data(bytes::Bytes::from_static(b"hello "), false)?;
    send.send_data(bytes::Bytes::from_static(b"world\n"), false)?;
    let mut trailers = hyper::HeaderMap::new();
    trailers.insert(
      HeaderName::from_static("abc"),
      HeaderValue::from_static("def"),
    );
    trailers.insert(
      HeaderName::from_static("opr"),
      HeaderValue::from_static("stv"),
    );
    if let Some(recv_trailers) = maybe_recv_trailers {
      for (key, value) in recv_trailers {
        trailers.insert(key.unwrap(), value);
      }
    }
    send.send_trailers(trailers)?;

    Ok(())
  }

  let local_set = LocalSet::new();
  local_set.spawn_local(async move {
    while let Some(Ok(tcp)) = tcp.next().await {
      tokio::spawn(async move {
        let _ = serve(tcp).await;
      });
    }
  });

  local_set.spawn_local(async move {
    while let Some(Ok(tls)) = tls.next().await {
      tokio::spawn(async move {
        let _ = serve_tls(tls).await;
      });
    }
  });

  local_set.await;
}
