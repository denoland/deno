// Copyright 2018-2026 the Deno authors. MIT license.
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::task::{self};
use std::vec;

use deno_permissions::PermissionsContainer;
use hickory_resolver::name_server::TokioConnectionProvider;
use http::Uri;
use http::uri::Scheme;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tower::Service;

#[derive(Clone, Debug)]
pub struct Resolver {
  kind: ResolverKind,
}

#[allow(clippy::large_enum_variant, reason = "TODO: investigate")]
#[derive(Clone, Debug)]
enum ResolverKind {
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
    Self {
      kind: ResolverKind::Gai(GaiResolver::new()),
    }
  }

  /// Create a [`AsyncResolver`] from system conf.
  pub fn hickory() -> Result<Self, hickory_resolver::ResolveError> {
    Ok(Self {
      kind: ResolverKind::Hickory(
        hickory_resolver::Resolver::builder_tokio()?.build(),
      ),
    })
  }

  pub fn hickory_from_resolver(
    resolver: hickory_resolver::Resolver<TokioConnectionProvider>,
  ) -> Self {
    Self {
      kind: ResolverKind::Hickory(resolver),
    }
  }

  pub fn custom(resolver: Arc<dyn Resolve>) -> Self {
    Self {
      kind: ResolverKind::Custom(resolver),
    }
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
    let task = match &mut self.kind {
      ResolverKind::Gai(gai_resolver) => {
        let mut resolver = gai_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.call(name).await?;
          Ok(result.collect::<Vec<_>>().into_iter())
        })
      }
      ResolverKind::Hickory(async_resolver) => {
        let resolver = async_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.lookup_ip(name.as_str()).await?;
          let addrs: Vec<_> =
            result.into_iter().map(|x| SocketAddr::new(x, 0)).collect();
          Ok(addrs.into_iter())
        })
      }
      ResolverKind::Custom(resolver) => {
        let resolver = resolver.clone();
        tokio::spawn(async move { resolver.resolve(name).await })
      }
    };
    ResolveFut { inner: task }
  }
}

/// `Service<Uri>` adapter that runs the net-deny permission check against the
/// IP we actually connected to.
///
/// hyper-util's `HttpConnector` resolves DNS and then picks one address to
/// connect to, applying the destination port itself. By wrapping it we can
/// read the real peer address off the resulting [`TcpStream`] and run the
/// same post-resolution check that `Deno.connect` performs in
/// `ext/net/ops.rs`, without smuggling the port to the DNS resolver out of
/// band.
#[derive(Clone, Debug)]
pub struct PermissionedHttpConnector {
  inner: HttpConnector<Resolver>,
  permissions: Option<PermissionsContainer>,
}

impl PermissionedHttpConnector {
  pub fn new(
    inner: HttpConnector<Resolver>,
    permissions: Option<PermissionsContainer>,
  ) -> Self {
    Self { inner, permissions }
  }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

impl Service<Uri> for PermissionedHttpConnector {
  type Response = TokioIo<TcpStream>;
  type Error = BoxError;
  type Future =
    Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(
    &mut self,
    cx: &mut task::Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(cx).map_err(Into::into)
  }

  fn call(&mut self, uri: Uri) -> Self::Future {
    let port = uri.port_u16().unwrap_or_else(|| {
      if uri.scheme() == Some(&Scheme::HTTPS) {
        443
      } else {
        80
      }
    });
    let permissions = self.permissions.clone();
    let fut = self.inner.call(uri);
    Box::pin(async move {
      let stream = fut.await?;
      if let Some(mut permissions) = permissions {
        // Fail closed: if we can't determine the peer we connected to, deny
        // rather than letting the connection through unchecked.
        let peer = stream.inner().peer_addr().map_err(|e| -> BoxError {
          io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()).into()
        })?;
        permissions
          .check_net_resolved(&peer.ip(), port, "fetch()")
          .map_err(|e| -> BoxError {
            io::Error::new(io::ErrorKind::PermissionDenied, e.to_string())
              .into()
          })?;
      }
      Ok(stream)
    })
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
    let mut resolver = Resolver::custom(Arc::new(DebugResolver(
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
