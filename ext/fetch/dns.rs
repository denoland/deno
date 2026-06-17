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
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use tokio::task::JoinHandle;
use tower::Service;

tokio::task_local! {
  /// The destination port for the in-flight HTTP/WS connection. Set by
  /// [`PermissionedHttpConnector`] before invoking the inner connector so
  /// that the DNS resolver below can run the post-resolution deny check
  /// against the real request port instead of the placeholder `0` that
  /// hyper-util's `HttpConnector` carries through `Service<Name>`.
  static REQUEST_PORT: u16;
}

#[derive(Clone, Debug)]
pub struct Resolver {
  kind: ResolverKind,
  permissions: Option<PermissionsContainer>,
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
      permissions: None,
    }
  }

  /// Create a [`AsyncResolver`] from system conf.
  pub fn hickory() -> Result<Self, hickory_resolver::ResolveError> {
    Ok(Self {
      kind: ResolverKind::Hickory(
        hickory_resolver::Resolver::builder_tokio()?.build(),
      ),
      permissions: None,
    })
  }

  pub fn hickory_from_resolver(
    resolver: hickory_resolver::Resolver<TokioConnectionProvider>,
  ) -> Self {
    Self {
      kind: ResolverKind::Hickory(resolver),
      permissions: None,
    }
  }

  pub fn custom(resolver: Arc<dyn Resolve>) -> Self {
    Self {
      kind: ResolverKind::Custom(resolver),
      permissions: None,
    }
  }

  /// Attach a permissions container so that resolved addresses are checked
  /// against the net deny list before being returned to the HTTP connector.
  /// This mirrors the post-resolution check that `Deno.connect` performs and
  /// prevents bypassing IP-literal deny rules via attacker-controlled DNS.
  pub fn with_permissions(mut self, permissions: PermissionsContainer) -> Self {
    self.permissions = Some(permissions);
    self
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
    let permissions = self.permissions.clone();
    // Capture the request port *before* spawning, since `tokio::spawn`
    // detaches from the parent task and would lose the task-local.
    let port = REQUEST_PORT.try_with(|p| *p).ok();
    let task = match &mut self.kind {
      ResolverKind::Gai(gai_resolver) => {
        let mut resolver = gai_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.call(name).await?;
          let addrs: Vec<_> = result.into_iter().collect();
          check_resolved_permissions(&addrs, port, permissions.as_ref())?;
          Ok(addrs.into_iter())
        })
      }
      ResolverKind::Hickory(async_resolver) => {
        let resolver = async_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.lookup_ip(name.as_str()).await?;
          let addrs: Vec<_> =
            result.into_iter().map(|x| SocketAddr::new(x, 0)).collect();
          check_resolved_permissions(&addrs, port, permissions.as_ref())?;
          Ok(addrs.into_iter())
        })
      }
      ResolverKind::Custom(resolver) => {
        let resolver = resolver.clone();
        tokio::spawn(async move {
          let result = resolver.resolve(name).await?;
          let addrs: Vec<_> = result.into_iter().collect();
          check_resolved_permissions(&addrs, port, permissions.as_ref())?;
          Ok(addrs.into_iter())
        })
      }
    };
    ResolveFut { inner: task }
  }
}

fn check_resolved_permissions(
  addrs: &[SocketAddr],
  request_port: Option<u16>,
  permissions: Option<&PermissionsContainer>,
) -> io::Result<()> {
  let Some(permissions) = permissions else {
    return Ok(());
  };
  for addr in addrs {
    // `addr.port()` is `0` for the GAI and Hickory paths because hyper-util's
    // `HttpConnector` sets the destination port *after* DNS resolution. Prefer
    // the real request port carried via [`REQUEST_PORT`].
    let port = request_port.unwrap_or_else(|| addr.port());
    permissions
      .clone()
      .check_net_connect_resolved(&addr.ip(), port, "fetch()")
      .map_err(|e| {
        io::Error::new(io::ErrorKind::PermissionDenied, e.to_string())
      })?;
  }
  Ok(())
}

/// `Service<Uri>` adapter that runs ahead of hyper-util's `HttpConnector`.
///
/// `HttpConnector` only forwards the hostname to its `Service<Name>` resolver
/// and applies the destination port afterwards, which means the resolver
/// cannot check post-resolution permissions against the real request port.
/// This wrapper extracts the port from the `Uri`, scopes it into the
/// [`REQUEST_PORT`] task-local, and then delegates to the inner connector.
#[derive(Clone, Debug)]
pub struct PermissionedHttpConnector<C> {
  inner: C,
}

impl<C> PermissionedHttpConnector<C> {
  pub fn new(inner: C) -> Self {
    Self { inner }
  }
}

impl<C> Service<Uri> for PermissionedHttpConnector<C>
where
  C: Service<Uri>,
  C::Future: Send + 'static,
{
  type Response = C::Response;
  type Error = C::Error;
  type Future =
    Pin<Box<dyn Future<Output = Result<C::Response, C::Error>> + Send>>;

  fn poll_ready(
    &mut self,
    cx: &mut task::Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(cx)
  }

  fn call(&mut self, uri: Uri) -> Self::Future {
    let port = uri.port_u16().unwrap_or_else(|| {
      if uri.scheme() == Some(&Scheme::HTTPS) {
        443
      } else {
        80
      }
    });
    let fut = self.inner.call(uri);
    Box::pin(REQUEST_PORT.scope(port, fut))
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
