// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use futures;
use futures::Future;
use tokio;
use tokio_executor;

pub fn block_on<F, R, E>(future: F) -> Result<R, E>
where
  F: Send + 'static + Future<Item = R, Error = E>,
  R: Send + 'static,
  E: Send + 'static,
{
  let (tx, rx) = futures::sync::oneshot::channel();
  tokio::spawn(future.then(move |r| tx.send(r).map_err(|_| unreachable!())));
  rx.wait().unwrap()
}

// Set the default executor so we can use tokio::spawn(). It's difficult to
// pass around mut references to the runtime, so using with_default is
// preferable. Ideally Tokio would provide this function.
pub fn init<F>(f: F)
where
  F: FnOnce(),
{
  let rt = tokio::runtime::Runtime::new().unwrap();
  let mut executor = rt.executor();
  let mut enter = tokio_executor::enter().expect("Multiple executors at once");
  tokio_executor::with_default(&mut executor, &mut enter, move |_enter| f());
}
