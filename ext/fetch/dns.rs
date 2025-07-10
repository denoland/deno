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

use crate::FetchPermissions;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum ResolverKind {
  Gai(GaiResolver),
  Hickory(hickory_resolver::Resolver<TokioConnectionProvider>),
  Custom(Arc<dyn Resolve>),
}

#[derive(Clone)]
pub struct Resolver {
  pub kind: ResolverKind,
  pub permissions: Arc<dyn FetchPermissions + 'static>,
}

impl std::fmt::Debug for Resolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Resolver")
      .field("kind", &self.kind)
      .field("permissions", &"...")
      .finish()
  }
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

impl Resolver {
  pub fn default(permissions: Arc<dyn FetchPermissions + 'static>) -> Self {
    Self::gai(permissions)
  }

  pub fn gai(permissions: Arc<dyn FetchPermissions + 'static>) -> Self {
    Self {
      kind: ResolverKind::Gai(GaiResolver::new()),
      permissions,
    }
  }

  pub fn custom(
    permissions: Arc<dyn FetchPermissions + 'static>,
    resolver: Arc<dyn Resolve>,
  ) -> Self {
    Self {
      kind: ResolverKind::Custom(resolver),
      permissions,
    }
  }

  /// Create a [`AsyncResolver`] from system conf.
  pub fn hickory(
    permissions: Arc<dyn FetchPermissions + 'static>,
  ) -> Result<Self, hickory_resolver::ResolveError> {
    Ok(Self {
      kind: ResolverKind::Hickory(
        hickory_resolver::Resolver::tokio_from_system_conf()?,
      ),
      permissions,
    })
  }

  pub fn hickory_from_resolver(
    permissions: Arc<dyn FetchPermissions + 'static>,
    resolver: hickory_resolver::Resolver<TokioConnectionProvider>,
  ) -> Self {
    Self {
      kind: ResolverKind::Hickory(resolver),
      permissions,
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
    let permissions = self.permissions.clone();
    let task = match &mut self.kind {
      ResolverKind::Gai(gai_resolver) => {
        let mut resolver = gai_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.call(name).await?;
          let x: Vec<_> = result.into_iter().collect();
          for addr in &x {
            permissions
              .check_net(
                &(&addr.ip().to_string(), Some(addr.port())),
                "Deno.fetch()",
              )
              .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
          }
          let iter: SocketAddrs = x.into_iter();
          Ok(iter)
        })
      }
      ResolverKind::Hickory(async_resolver) => {
        let resolver = async_resolver.clone();
        tokio::spawn(async move {
          let result = resolver.lookup_ip(name.as_str()).await?;

          let x: Vec<_> =
            result.into_iter().map(|x| SocketAddr::new(x, 0)).collect();

          for addr in &x {
            permissions
              .check_net(
                &(&addr.ip().to_string(), Some(addr.port())),
                "Deno.fetch()",
              )
              .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
          }
          let iter: SocketAddrs = x.into_iter();
          Ok(iter)
        })
      }
      ResolverKind::Custom(resolver) => {
        let resolver = resolver.clone();
        tokio::spawn(async move {
          let result = resolver.resolve(name).await?;
          let x: Vec<_> = result.into_iter().collect();
          for addr in &x {
            permissions
              .check_net(
                &(&addr.ip().to_string(), Some(addr.port())),
                "Deno.fetch()",
              )
              .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
          }
          let iter: SocketAddrs = x.into_iter();
          Ok(iter)
        })
      }
    };
    ResolveFut { inner: task }
  }
}

#[cfg(test)]
mod tests {
  use std::borrow::Cow;
  use std::path::Path;
  use std::str::FromStr;

  use deno_core::url::Url;
  use deno_permissions::PermissionCheckError;

  use super::*;

  impl FetchPermissions for () {
    fn check_net_url(
      &self,
      _url: &Url,
      _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
      Ok(())
    }

    fn check_net_vsock(
      &mut self,
      _cid: u32,
      _port: u32,
      _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
      Ok(())
    }

    fn check_open<'a>(
      &mut self,
      path: Cow<'a, Path>,
      _open_access: deno_permissions::OpenAccessKind,
      _api_name: &str,
    ) -> Result<deno_permissions::CheckedPath<'a>, PermissionCheckError> {
      Ok(deno_permissions::CheckedPath {
        path: deno_permissions::PathWithRequested {
          path,
          requested: None,
        },
        canonicalized: false,
      })
    }

    fn check_net(
      &self,
      _addr: &(&str, Option<u16>),
      _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
      Ok(())
    }
  }

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
    let mut resolver = Resolver::custom(
      Arc::new(()),
      Arc::new(DebugResolver("127.0.0.1:8080".parse().unwrap())),
    );
    let mut addr = resolver
      .call(Name::from_str("foo.com").unwrap())
      .await
      .unwrap();

    let addr = addr.next().unwrap();
    assert_eq!(addr, "127.0.0.1:8080".parse().unwrap());
  }
}
