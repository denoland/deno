// Copyright 2018-2026 the Deno authors. MIT license.

use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
// re-export test_util so server source files can use crate::PathRef,
// crate::testdata_path, crate::consts::*, etc. without changes
pub use test_util::*;
use tokio::net::TcpStream;

pub mod https;
pub mod npm;
pub mod servers;

/// Returns a [`Stream`] of [`TcpStream`]s accepted from the given port.
async fn get_tcp_listener_stream(
  name: &'static str,
  port: u16,
) -> impl Stream<Item = Result<TcpStream, std::io::Error>> + Unpin + Send {
  let host_and_port = &format!("localhost:{port}");

  // Listen on ALL addresses that localhost can resolves to.
  let accept = |listener: tokio::net::TcpListener| {
    async {
      let result = listener.accept().await;
      Some((result.map(|r| r.0), listener))
    }
    .boxed()
  };

  let mut addresses = vec![];
  let listeners = tokio::net::lookup_host(host_and_port)
    .await
    .expect(host_and_port)
    .inspect(|address| addresses.push(*address))
    .map(tokio::net::TcpListener::bind)
    .collect::<futures::stream::FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(|s| s.unwrap())
    .map(|listener| futures::stream::unfold(listener, accept))
    .collect::<Vec<_>>();

  // Eye catcher for HttpServerCount
  test_util::println!("ready: {name} on {:?}", addresses);

  futures::stream::select_all(listeners)
}

/// Extension trait adding server-dependent methods to TestContext.
/// These methods require heavy dependencies (reqwest, tokio, sha2) that
/// are only available in the test_server crate.
pub trait TestContextServerExt {
  fn get_jsr_package_integrity(&self, sub_path: &str) -> String;
}

impl TestContextServerExt for test_util::TestContext {
  fn get_jsr_package_integrity(&self, sub_path: &str) -> String {
    fn get_checksum(bytes: &[u8]) -> String {
      use sha2::Digest;
      let mut hasher = sha2::Sha256::new();
      hasher.update(bytes);
      format!("{:x}", hasher.finalize())
    }

    let url = url::Url::parse(self.envs().get("JSR_URL").unwrap()).unwrap();
    let url = url.join(&format!("{}_meta.json", sub_path)).unwrap();
    let bytes = sync_fetch(url);
    get_checksum(&bytes)
  }
}

fn sync_fetch(url: url::Url) -> bytes::Bytes {
  std::thread::scope(move |s| {
    s.spawn(move || {
      let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
      runtime.block_on(async move {
        let client = reqwest::Client::new();
        let response = client.get(url).send().await.unwrap();
        assert!(response.status().is_success());
        response.bytes().await.unwrap()
      })
    })
    .join()
    .unwrap()
  })
}
