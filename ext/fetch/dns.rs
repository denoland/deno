// Copyright 2018-2025 the Deno authors. MIT license.
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::task::{self};
use std::vec;

use hickory_resolver::name_server::TokioConnectionProvider;
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use tokio::task::JoinHandle;
use tower::Service;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Resolver {
  /// A resolver using blocking `getaddrinfo` calls in a threadpool.
  Gai(GaiResolver),
  /// hickory-resolver's userspace resolver.
  Hickory(hickory_resolver::Resolver<TokioConnectionProvider>),
  /// A custom resolver that implements `Resolve`.
  Custom(Arc<dyn Resolve>),
}

/// Alias for the `Future` type returned by a custom DNS resolver.
// The future has to be `Send` as `tokio::spawn` is used to execute the future.
pub type Resolving =
  Pin<Box<dyn Future<Output = Result<SocketAddrs, io::Error>> + Send>>;

/// A trait for customizing DNS resolution in ext/fetch.
// The resolver needs to be `Send` and `Sync` for two reasons. One is it is
// wrapped inside an `Arc` and will be cloned and moved to an async block to
// perfrom DNS resolution. That async block will be executed by `tokio::spawn`,
// so to make that async block `Send`, `Arc<dyn Resolve>` needs to be
// `Send`. The other is `Resolver` needs to be `Send` to make the wrapping
// `HttpConnector` `Send`.
pub trait Resolve: Send + Sync + std::fmt::Debug {
  fn resolve(&self, name: Name) -> Resolving;
}

impl Default for Resolver {
  fn default() -> Self {
    Self::gai()
  }
}

impl Resolver {
  pub fn gai() -> Self {
    Self::Gai(GaiResolver::new())
  }

  /// Create a [`AsyncResolver`] from system conf.
  pub fn hickory() -> Result<Self, hickory_resolver::ResolveError> {
    Ok(Self::Hickory(
      hickory_resolver::Resolver::builder_tokio()?.build(),
    ))
  }

  pub fn hickory_from_resolver(
    resolver: hickory_resolver::Resolver<TokioConnectionProvider>,
  ) -> Self {
    Self::Hickory(resolver)
  }
}

type SocketAddrs = vec::IntoIter<SocketAddr>;

pub struct ResolveFut {
  inner: JoinHandle<Result<SocketAddrs, io::Error>>,
}

impl Future for ResolveFut {
  type Output = Result<SocketAddrs, io::Error>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut task::Context<'_>,
  ) -> Poll<Self::Output> {
    Pin::new(&mut self.inner).poll(cx).map(|res| match res {
      Ok(Ok(addrs)) => Ok(addrs),
      Ok(Err(e)) => Err(e),
      Err(join_err) => {
        if join_err.is_cancelled() {
          Err(io::Error::new(io::ErrorKind::Interrupted, join_err))
        } else {
          Err(io::Error::other(join_err))
        }
      }
    })
  }
}

impl Service<Name> for Resolver {
  type Response = SocketAddrs;
  type Error = io::Error;
  type Future = ResolveFut;

  fn poll_ready(
    &mut self,
    _cx: &mut task::Context<'_>,
  ) -> Poll<Result<(), io::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, name: Name) -> Self::Future {
    let task = match self {
      Resolver::Gai(gai_resolver) => {
        let mut resolver = gai_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.call(name).await?;
          let x: Vec<_> = result.into_iter().collect();
          let iter: SocketAddrs = x.into_iter();
          Ok(iter)
        })
      }
      Resolver::Hickory(async_resolver) => {
        let resolver = async_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.lookup_ip(name.as_str()).await?;

          let x: Vec<_> =
            result.into_iter().map(|x| SocketAddr::new(x, 0)).collect();
          let iter: SocketAddrs = x.into_iter();
          Ok(iter)
        })
      }
      Resolver::Custom(resolver) => {
        let resolver = resolver.clone();
        tokio::spawn(async move { resolver.resolve(name).await })
      }
    };
    ResolveFut { inner: task }
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;

  // A resolver that resolves any name into the same address.
  #[derive(Debug)]
  struct DebugResolver(SocketAddr);

  impl Resolve for DebugResolver {
    fn resolve(&self, _name: Name) -> Resolving {
      let addr = self.0;
      Box::pin(async move { Ok(vec![addr].into_iter()) })
    }
  }

  #[tokio::test]
  async fn custom_dns_resolver() {
    let mut resolver = Resolver::Custom(Arc::new(DebugResolver(
      "127.0.0.1:8080".parse().unwrap(),
    )));
    let mut addr = resolver
      .call(Name::from_str("foo.com").unwrap())
      .await
      .unwrap();

    let addr = addr.next().unwrap();
    assert_eq!(addr, "127.0.0.1:8080".parse().unwrap());
  }
}
