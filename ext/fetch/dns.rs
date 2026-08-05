// Copyright 2018-2026 the Deno authors. MIT license.
use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
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

/// `Service<Uri>` adapter that runs the net-deny permission check on every
/// resolved address *before* a socket is opened.
///
/// hyper-util's `HttpConnector` only hands the hostname to its DNS resolver
/// and applies the destination port after resolution, so the resolver alone
/// can't run the port-aware deny check. This wrapper sees the full `Uri`
/// (and thus the real port) and owns the resolver: it resolves the hostname
/// itself, checks every resolved address (denying before any connection is
/// made, like `Deno.connect` in `ext/net/ops.rs`), and then hands the vetted
/// addresses to the inner connector through a pre-resolved resolver, so the
/// connection goes to exactly the addresses that were checked and no second
/// DNS query can race with a record change.
#[derive(Clone, Debug)]
pub struct PermissionedHttpConnector {
  resolver: Resolver,
  local_address: Option<IpAddr>,
  permissions: Option<PermissionsContainer>,
  no_delay: bool,
}

impl PermissionedHttpConnector {
  pub fn new(
    resolver: Resolver,
    local_address: Option<IpAddr>,
    permissions: Option<PermissionsContainer>,
    no_delay: bool,
  ) -> Self {
    Self {
      resolver,
      local_address,
      permissions,
      no_delay,
    }
  }

  fn http_connector(&self, resolver: Resolver) -> HttpConnector<Resolver> {
    let mut connector = HttpConnector::new_with_resolver(resolver);
    connector.enforce_http(false);
    connector.set_local_address(self.local_address);
    connector.set_nodelay(self.no_delay);
    connector
  }
}

/// Resolver handing out a fixed, already-permission-checked set of addresses.
#[derive(Debug)]
struct PreResolved(Vec<SocketAddr>);

impl Resolve for PreResolved {
  fn resolve(&self, _name: Name) -> Resolving {
    let addrs = self.0.clone();
    Box::pin(async move { Ok(addrs.into_iter()) })
  }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Mirrors the `ConnectError` shape hyper-util's `HttpConnector` uses for
/// resolution failures, so error messages keep the same
/// `client error (Connect): dns error: ...` format now that resolution
/// happens in [`PermissionedHttpConnector`] instead.
#[derive(Debug)]
struct DnsError(io::Error);

impl std::fmt::Display for DnsError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("dns error")
  }
}

impl std::error::Error for DnsError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.0)
  }
}

fn check_resolved(
  permissions: &PermissionsContainer,
  ip: &IpAddr,
  port: u16,
) -> Result<(), BoxError> {
  permissions
    .clone()
    .check_net_resolved(ip, port, "fetch()")
    .map_err(|e| {
      io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()).into()
    })
}

/// Extracts the connection host (with IPv6 brackets stripped) and the
/// effective destination port from a `Uri`, defaulting the port from the
/// scheme. Returns `None` if the `Uri` has no host.
fn bare_host_and_port(uri: &Uri) -> Option<(&str, u16)> {
  let host = uri.host()?;
  let port = uri.port_u16().unwrap_or_else(|| {
    if uri.scheme() == Some(&Scheme::HTTPS) {
      443
    } else {
      80
    }
  });
  let bare_host = host
    .strip_prefix('[')
    .and_then(|h| h.strip_suffix(']'))
    .unwrap_or(host);
  Some((bare_host, port))
}

impl Service<Uri> for PermissionedHttpConnector {
  type Response = TokioIo<TcpStream>;
  type Error = BoxError;
  type Future =
    Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(
    &mut self,
    _cx: &mut task::Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, uri: Uri) -> Self::Future {
    let this = self.clone();
    Box::pin(async move {
      let Some(permissions) = &this.permissions else {
        let mut connector = this.http_connector(this.resolver.clone());
        return connector.call(uri).await.map_err(Into::into);
      };

      let Some((bare_host, port)) = bare_host_and_port(&uri) else {
        return Err(
          io::Error::new(io::ErrorKind::InvalidInput, "missing host in URI")
            .into(),
        );
      };
      if let Ok(ip) = bare_host.parse::<IpAddr>() {
        // IP literal: `HttpConnector` connects to it directly without
        // consulting the resolver.
        check_resolved(permissions, &ip, port)?;
        let mut connector = this.http_connector(this.resolver.clone());
        return connector.call(uri).await.map_err(Into::into);
      }

      let name = Name::from_str(bare_host).map_err(|e| -> BoxError {
        io::Error::new(io::ErrorKind::InvalidInput, e.to_string()).into()
      })?;
      let addrs: Vec<SocketAddr> = this
        .resolver
        .clone()
        .call(name)
        .await
        .map_err(|e| -> BoxError { DnsError(e).into() })?
        .collect();
      for addr in &addrs {
        check_resolved(permissions, &addr.ip(), port)?;
      }

      let mut connector =
        this.http_connector(Resolver::custom(Arc::new(PreResolved(addrs))));
      connector.call(uri).await.map_err(Into::into)
    })
  }
}

/// Lets a connector run a best-effort net-deny check against a destination
/// `Uri` without opening a connection to it.
///
/// Used by the proxy connector: when a request is routed through a proxy, the
/// deny check the connector would otherwise run sees the connection to the
/// *proxy*, not the destination, so an IP-level `--deny-net` rule would only be
/// enforced against the destination's pre-resolution hostname. This resolves
/// the destination and checks every resolved address too.
///
/// It is best-effort: a proxy may be able to reach a host this process cannot
/// resolve locally, so a resolution failure here is not fatal. The connection
/// to the proxy itself is still checked separately.
pub trait CheckDst {
  fn check_dst(
    &self,
    uri: Uri,
  ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send>>;
}

impl CheckDst for PermissionedHttpConnector {
  fn check_dst(
    &self,
    uri: Uri,
  ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send>> {
    let this = self.clone();
    Box::pin(async move {
      let Some(permissions) = &this.permissions else {
        return Ok(());
      };
      let Some((bare_host, port)) = bare_host_and_port(&uri) else {
        return Ok(());
      };
      if let Ok(ip) = bare_host.parse::<IpAddr>() {
        return check_resolved(permissions, &ip, port);
      }
      let Ok(name) = Name::from_str(bare_host) else {
        return Ok(());
      };
      // Best-effort: if the destination can't be resolved locally, let the
      // proxy handle it rather than failing the request.
      let Ok(addrs) = this.resolver.clone().call(name).await else {
        return Ok(());
      };
      for addr in addrs {
        check_resolved(permissions, &addr.ip(), port)?;
      }
      Ok(())
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
