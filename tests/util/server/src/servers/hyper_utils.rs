// Copyright 2018-2025 the Deno authors. MIT license.

use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::result::Result;

use bytes::Bytes;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use http;
use http::Request;
use http::Response;
use http_body_util::combinators::UnsyncBoxBody;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[derive(Debug, Clone, Copy)]
pub enum ServerKind {
  Auto,
  OnlyHttp1,
  OnlyHttp2,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerOptions {
  pub error_msg: &'static str,
  pub addr: SocketAddr,
  pub kind: ServerKind,
}

pub type HandlerOutput =
  Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error>;

pub async fn run_server<F, S>(options: ServerOptions, handler: F)
where
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  let fut: Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> =
    async move {
      let listener = TcpListener::bind(options.addr).await?;
      #[allow(clippy::print_stdout)]
      {
        println!("ready: {}", options.addr);
      }
      loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        deno_unsync::spawn(hyper_serve_connection(
          io,
          handler,
          options.error_msg,
          options.kind,
        ));
      }
    }
    .boxed_local();

  if let Err(e) = fut.await {
    let err_str = e.to_string();
    #[allow(clippy::print_stderr)]
    if !err_str.contains("early eof") {
      eprintln!("{}: {:?}", options.error_msg, e);
    }
  }
}

pub async fn run_server_with_acceptor<A, F, S>(
  mut acceptor: Pin<Box<A>>,
  handler: F,
  error_msg: &'static str,
  kind: ServerKind,
) where
  A: Stream<Item = io::Result<rustls_tokio_stream::TlsStream>> + ?Sized,
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  let fut: Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> =
    async move {
      while let Some(result) = acceptor.next().await {
        let stream = result?;
        let io = TokioIo::new(stream);
        deno_unsync::spawn(hyper_serve_connection(
          io, handler, error_msg, kind,
        ));
      }
      Ok(())
    }
    .boxed_local();

  if let Err(e) = fut.await {
    let err_str = e.to_string();
    #[allow(clippy::print_stderr)]
    if !err_str.contains("early eof") {
      eprintln!("{}: {:?}", error_msg, e);
    }
  }
}

async fn hyper_serve_connection<I, F, S>(
  io: I,
  handler: F,
  error_msg: &'static str,
  kind: ServerKind,
) where
  I: hyper::rt::Read + hyper::rt::Write + Unpin + 'static,
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  let service = hyper::service::service_fn(handler);

  let result: Result<(), anyhow::Error> = match kind {
    ServerKind::Auto => {
      let builder =
        hyper_util::server::conn::auto::Builder::new(DenoUnsyncExecutor);
      builder
        .serve_connection(io, service)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))
    }
    ServerKind::OnlyHttp1 => {
      let builder = hyper::server::conn::http1::Builder::new();
      builder
        .serve_connection(io, service)
        .await
        .map_err(|e| e.into())
    }
    ServerKind::OnlyHttp2 => {
      let builder =
        hyper::server::conn::http2::Builder::new(DenoUnsyncExecutor);
      builder
        .serve_connection(io, service)
        .await
        .map_err(|e| e.into())
    }
  };

  if let Err(e) = result {
    let err_str = e.to_string();
    #[allow(clippy::print_stderr)]
    if !err_str.contains("early eof") {
      eprintln!("{}: {:?}", error_msg, e);
    }
  }
}

#[derive(Clone)]
struct DenoUnsyncExecutor;

impl<Fut> hyper::rt::Executor<Fut> for DenoUnsyncExecutor
where
  Fut: Future + 'static,
  Fut::Output: 'static,
{
  fn execute(&self, fut: Fut) {
    deno_unsync::spawn(fut);
  }
}
