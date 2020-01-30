// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[cfg(test)]
pub fn run<F>(future: F)
where
  F: std::future::Future<Output = ()> + Send + 'static,
{
  let mut rt = tokio::runtime::Builder::new()
    .threaded_scheduler()
    .enable_all()
    .thread_name("deno")
    .build()
    .expect("Unable to create Tokio runtime");
  rt.block_on(future);
}
