// Copyright 2018-2025 the Deno authors. MIT license.

use bencher::*;
use futures::StreamExt;
use futures::stream::FuturesOrdered;
use futures::stream::FuturesUnordered;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use tokio::net::TcpListener;
use tokio::task::JoinSet;

const LOOPS: usize = 10;
const COUNT: usize = 10;

async fn task() {
  tokio::task::yield_now().await;
  let mut v = vec![];
  for _ in 0..3 {
    v.push(TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::LOCALHOST,
      0,
    ))));
  }
  drop(v);
}

fn bench_futures_unordered(b: &mut Bencher) {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();
  let mut futures = FuturesUnordered::default();
  b.iter(|| {
    runtime.block_on(async {
      for _ in 0..LOOPS {
        for _ in 0..COUNT {
          futures.push(task());
        }
        for _ in 0..COUNT {
          futures.next().await;
        }
      }
    });
  });
}

fn bench_futures_ordered(b: &mut Bencher) {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();
  let mut futures = FuturesOrdered::default();
  b.iter(|| {
    runtime.block_on(async {
      for _ in 0..LOOPS {
        for _ in 0..COUNT {
          futures.push_back(task());
        }
        for _ in 0..COUNT {
          futures.next().await;
        }
      }
    });
  });
}

fn bench_joinset(b: &mut Bencher) {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();
  let mut futures = JoinSet::default();
  b.iter(|| {
    runtime.block_on(async {
      for _ in 0..LOOPS {
        for _ in 0..COUNT {
          futures.spawn(task());
        }
        for _ in 0..COUNT {
          futures.join_next().await;
        }
      }
    });
  });
}

fn bench_unicycle(b: &mut Bencher) {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();
  let mut futures = unicycle::FuturesUnordered::new();
  b.iter(|| {
    runtime.block_on(async {
      for _ in 0..LOOPS {
        for _ in 0..COUNT {
          futures.push(task());
        }
        for _ in 0..COUNT {
          futures.next().await;
        }
      }
    });
  });
}

benchmark_main!(benches);

benchmark_group!(
  benches,
  bench_futures_ordered,
  bench_futures_unordered,
  bench_joinset,
  bench_unicycle,
);
