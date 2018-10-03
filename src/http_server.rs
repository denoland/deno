// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use errors::DenoError;
use errors::DenoResult;

use futures::sync::mpsc;
use futures::sync::oneshot;
use futures::Sink;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server};
use std::net::SocketAddr;
use tokio;

pub type Res = Response<Body>;

// server -> loop
pub struct Transaction {
  pub req: Request<Body>,
  pub response_tx: Option<oneshot::Sender<Res>>,
}

// accept -> loop
pub type TransactionSender = oneshot::Sender<Transaction>;

pub struct HttpServer {
  sender_a: mpsc::Sender<TransactionSender>,
  complete_tx: Option<oneshot::Sender<()>>,
}

/// This ensures that when the HttpServer is dropped from the resources table
/// it will stop its pending future.
impl Drop for HttpServer {
  fn drop(&mut self) {
    let complete_tx = self.complete_tx.take().unwrap();
    complete_tx.send(()).unwrap();
  }
}

impl HttpServer {
  pub fn accept(&self) -> impl Future<Item = Transaction, Error = DenoError> {
    let (transaction_sender, transaction_receiver) =
      oneshot::channel::<Transaction>();
    let tx = self.sender_a.clone();
    tx.send(transaction_sender)
      .map_err(|e| DenoError::from(e))
      .and_then(|_| transaction_receiver.map_err(|e| DenoError::from(e)))
  }
}

pub fn create_and_bind(addr: &SocketAddr) -> DenoResult<HttpServer> {
  let (sender_a, loop_rx) = mpsc::channel::<TransactionSender>(1);
  let (sender_b, loop2_rx) = mpsc::channel::<Transaction>(1);

  let sender_b2 = sender_b.clone();

  let loop_fut =
    loop_rx
      .zip(loop2_rx)
      .for_each(|(transaction_sender, transaction)| {
        let r = transaction_sender.send(transaction);
        assert!(r.is_ok());
        Ok(())
      });

  let new_service = move || {
    // Yes, this is oddly necessary. Attempts to remove it end in tears.
    let sender_b3 = sender_b2.clone();

    service_fn(move |req: Request<Body>| {
      let (response_tx, response_rx) = oneshot::channel::<Res>();
      //
      let transaction = Transaction {
        req,
        response_tx: Some(response_tx),
      };
      // Clone necessary here too.
      sender_b3
        .clone()
        .send(transaction)
        .map_err(|e| DenoError::from(e))
        .and_then(|_| response_rx.map_err(|e| DenoError::from(e)))
    })
  };

  let builder = Server::try_bind(&addr)?;
  let fut = builder.serve(new_service);
  let fut = fut.map_err(|err| panic!(err));

  let (complete_tx, complete_rx) = oneshot::channel::<()>();

  tokio::spawn(loop_fut);
  tokio::spawn(
    complete_rx.select(fut)
    .map_err(|err| panic!(err)) // TODO properly handle error here.
    .and_then(|_| {
      Ok(())
    }),
  );

  let http_server = HttpServer {
    sender_a,
    complete_tx: Some(complete_tx),
  };

  Ok(http_server)
}

#[cfg(test)]
mod test {
  use futures::Future;
  use http_util;
  use hyper::{Body, Response};
  use std::net::SocketAddr;
  use std::str::FromStr;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;
  use tokio;
  use tokio_util;

  #[test]
  fn test_http_server_create() {
    let req_counter = Arc::new(AtomicUsize::new(0));
    // Clone the counter, so we can access it in the closure (which may happen on
    // another thread.
    let req_counter_ = req_counter.clone();
    let addr = SocketAddr::from_str("127.0.0.1:4500").unwrap();
    tokio_util::init(|| {
      let http_server = super::create_and_bind(&addr).unwrap();

      let accept_fut = http_server
        .accept()
        .map(move |mut transaction| {
          assert_eq!(transaction.req.uri(), "/foo");
          let response_tx = transaction.response_tx.take().unwrap();
          assert!(response_tx.is_canceled() == false);
          let r = response_tx.send(Response::new(Body::from("hi")));
          assert!(r.is_ok());
          req_counter_.fetch_add(1, Ordering::SeqCst);
          ()
        }).map_err(|e| panic!(e));
      tokio::spawn(accept_fut);

      let r = http_util::fetch_sync_string("http://127.0.0.1:4500/foo");
      assert!(r.is_ok());
      let (res_body, _res_content_type) = r.unwrap();
      assert_eq!(res_body, "hi");
    });
    assert_eq!(req_counter.load(Ordering::SeqCst), 1);
  }
}
