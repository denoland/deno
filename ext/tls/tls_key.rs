// Copyright 2018-2026 the Deno authors. MIT license.

//! These represent the various types of TLS keys we support for both client and server
//! connections.
//!
//! A TLS key will most often be static, and will loaded from a certificate and key file
//! or string. These are represented by `TlsKey`, which is stored in `TlsKeys::Static`.
//!
//! In more complex cases, you may need a `TlsKeyResolver`/`TlsKeyLookup` pair, which
//! requires polling of the `TlsKeyLookup` lookup queue. The underlying channels that used for
//! key lookup can handle closing one end of the pair, in which case they will just
//! attempt to clean up the associated resources.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::future::poll_fn;
use std::future::ready;
use std::io::ErrorKind;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::futures::FutureExt;
use deno_core::futures::future::Either;
use deno_core::unsync::spawn;
use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls_tokio_stream::ServerConfigProvider;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::get_ssl_key_log;

#[derive(Debug, thiserror::Error)]
pub enum TlsKeyError {
  #[error(transparent)]
  Rustls(#[from] rustls::Error),
  #[error("Failed: {0}")]
  Failed(ErrorType),
  #[error(transparent)]
  JoinError(#[from] tokio::task::JoinError),
  #[error(transparent)]
  RecvError(#[from] tokio::sync::broadcast::error::RecvError),
}

type ErrorType = Arc<Box<str>>;

/// A TLS certificate/private key pair.
/// see https://docs.rs/rustls-pki-types/latest/rustls_pki_types/#cloning-private-keys
#[derive(Debug, PartialEq, Eq)]
pub struct TlsKey(pub Vec<CertificateDer<'static>>, pub PrivateKeyDer<'static>);

impl Clone for TlsKey {
  fn clone(&self) -> Self {
    Self(self.0.clone(), self.1.clone_key())
  }
}

/// The relevant parts of a TLS ClientHello, passed to a `TlsKeyResolver`
/// callback so it can pick a certificate per connection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlsClientHelloInfo {
  pub sni: String,
  /// The ALPN protocols offered by the client, in order of preference.
  pub alpn: Vec<Vec<u8>>,
}

impl TlsClientHelloInfo {
  /// Key used for caching resolutions. Connections with the same SNI but a
  /// different ALPN offer resolve independently, since the resolver may
  /// answer them differently (eg. TLS-ALPN-01 challenge handshakes).
  ///
  /// The ALPN values are client-controlled arbitrary bytes, so the key keeps
  /// them as-is (a `Vec<Vec<u8>>`) rather than lossily decoding to a string,
  /// which would let two distinct offers collide.
  fn cache_key(&self) -> CacheKey {
    (self.sni.clone(), self.alpn.clone())
  }
}

/// The result of a `TlsKeyResolver` lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTlsKey {
  pub key: TlsKey,
  /// When set, overrides the listener's ALPN protocols for this
  /// resolution (eg. `acme-tls/1` for a TLS-ALPN-01 challenge handshake).
  pub alpn: Option<Vec<Vec<u8>>>,
  /// When `false`, this resolution is not cached and the next handshake
  /// with the same ClientHello triggers a fresh lookup.
  pub cache: bool,
}

#[derive(Clone, Debug, Default)]
pub enum TlsKeys {
  // TODO(mmastrac): We need Option<&T> for cppgc -- this is a workaround
  #[default]
  Null,
  Static(TlsKey),
  Resolver(TlsKeyResolver),
}

pub struct TlsKeysHolder(RefCell<TlsKeys>);

// SAFETY: we're sure `TlsKeysHolder` can be GCed
unsafe impl deno_core::GarbageCollected for TlsKeysHolder {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TlsKeyHolder"
  }
}

impl TlsKeysHolder {
  pub fn take(&self) -> TlsKeys {
    std::mem::take(&mut *self.0.borrow_mut())
  }
}

impl From<TlsKeys> for TlsKeysHolder {
  fn from(value: TlsKeys) -> Self {
    TlsKeysHolder(RefCell::new(value))
  }
}

impl TryInto<Option<TlsKey>> for TlsKeys {
  type Error = Self;
  fn try_into(self) -> Result<Option<TlsKey>, Self::Error> {
    match self {
      Self::Null => Ok(None),
      Self::Static(key) => Ok(Some(key)),
      Self::Resolver(_) => Err(self),
    }
  }
}

impl From<Option<TlsKey>> for TlsKeys {
  fn from(value: Option<TlsKey>) -> Self {
    match value {
      None => TlsKeys::Null,
      Some(key) => TlsKeys::Static(key),
    }
  }
}

enum TlsKeyState {
  Resolving(broadcast::Receiver<Result<ResolvedTlsKey, ErrorType>>),
  Resolved(Result<ResolvedTlsKey, ErrorType>),
}

/// Cache key for a resolution: the SNI hostname together with the client's
/// ALPN offer. The ALPN values are kept as raw bytes so distinct offers
/// never collide.
type CacheKey = (String, Vec<Vec<u8>>);

type TlsKeyCache = Rc<RefCell<HashMap<CacheKey, TlsKeyState>>>;

type ResolutionRequest = (
  String,
  TlsClientHelloInfo,
  broadcast::Sender<Result<ResolvedTlsKey, ErrorType>>,
);

struct TlsKeyResolverInner {
  resolution_tx: mpsc::UnboundedSender<ResolutionRequest>,
  cache: TlsKeyCache,
  // Monotonic counter for the opaque resolution id handed to the lookup side
  // (and round-tripped through JS). Decoupled from the cache key so the id
  // stays a collision-free string regardless of the (arbitrary-byte) ALPN.
  next_id: Cell<u64>,
}

#[derive(Clone)]
pub struct TlsKeyResolver {
  inner: Rc<TlsKeyResolverInner>,
}

impl TlsKeyResolver {
  async fn resolve_internal(
    &self,
    hello: TlsClientHelloInfo,
    alpn: Vec<Vec<u8>>,
  ) -> Result<Arc<ServerConfig>, TlsKeyError> {
    let resolved = self.resolve(hello).await?;

    let mut tls_config = ServerConfig::builder()
      .with_no_client_auth()
      .with_single_cert(resolved.key.0, resolved.key.1.clone_key())?;
    tls_config.key_log = get_ssl_key_log();
    tls_config.alpn_protocols = resolved.alpn.unwrap_or(alpn);
    Ok(tls_config.into())
  }

  pub fn into_server_config_provider(
    self,
    alpn: Vec<Vec<u8>>,
  ) -> ServerConfigProvider {
    let (tx, mut rx) = mpsc::unbounded_channel::<(_, oneshot::Sender<_>)>();

    // We don't want to make the resolver multi-threaded, but the `ServerConfigProvider` is
    // required to be wrapped in an Arc. To fix this, we spawn a task in our current runtime
    // to respond to the requests.
    spawn(async move {
      while let Some((hello, txr)) = rx.recv().await {
        _ = txr.send(self.resolve_internal(hello, alpn.clone()).await);
      }
    });

    Arc::new(move |hello| {
      // Take ownership of the ClientHello information
      let info = TlsClientHelloInfo {
        sni: hello.server_name().unwrap_or_default().to_owned(),
        alpn: hello
          .alpn()
          .map(|protos| protos.map(|p| p.to_vec()).collect())
          .unwrap_or_default(),
      };
      let (txr, rxr) = tokio::sync::oneshot::channel::<_>();
      _ = tx.send((info, txr));
      rxr
        .map(|res| match res {
          Err(e) => Err(std::io::Error::new(ErrorKind::InvalidData, e)),
          Ok(Err(e)) => Err(std::io::Error::new(ErrorKind::InvalidData, e)),
          Ok(Ok(res)) => Ok(res),
        })
        .boxed()
    })
  }
}

impl Debug for TlsKeyResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TlsKeyResolver").finish()
  }
}

pub fn new_resolver() -> (TlsKeyResolver, TlsKeyLookup) {
  let (resolution_tx, resolution_rx) = mpsc::unbounded_channel();
  let cache = TlsKeyCache::default();
  (
    TlsKeyResolver {
      inner: Rc::new(TlsKeyResolverInner {
        resolution_tx,
        cache: cache.clone(),
        next_id: Cell::new(0),
      }),
    },
    TlsKeyLookup {
      resolution_rx: RefCell::new(resolution_rx),
      pending: Default::default(),
      cache,
    },
  )
}

impl TlsKeyResolver {
  /// Resolve the certificate and key for a given ClientHello. This immediately spawns a task
  /// in the background and is therefore cancellation-safe.
  pub fn resolve(
    &self,
    hello: TlsClientHelloInfo,
  ) -> impl Future<Output = Result<ResolvedTlsKey, TlsKeyError>> + use<> {
    let cache_key = hello.cache_key();
    let mut cache = self.inner.cache.borrow_mut();
    let mut recv = match cache.get(&cache_key) {
      None => {
        let (tx, rx) = broadcast::channel(1);
        cache
          .insert(cache_key.clone(), TlsKeyState::Resolving(rx.resubscribe()));
        let id = self.inner.next_id.get();
        self.inner.next_id.set(id.wrapping_add(1));
        _ = self.inner.resolution_tx.send((id.to_string(), hello, tx));
        rx
      }
      Some(TlsKeyState::Resolving(recv)) => recv.resubscribe(),
      Some(TlsKeyState::Resolved(res)) => {
        return Either::Left(ready(res.clone().map_err(TlsKeyError::Failed)));
      }
    };
    drop(cache);

    // Make this cancellation safe
    let inner = self.inner.clone();
    let handle = spawn(async move {
      let res = recv.recv().await?;
      let mut cache = inner.cache.borrow_mut();
      let cacheable = match &res {
        Ok(resolved) => resolved.cache,
        // Never cache errors: otherwise a peer varying SNI/ALPN could grow
        // the cache without bound with failed resolutions, and a transient
        // failure would be sticky. The next handshake retries instead.
        Err(_) => false,
      };
      match cache.get(&cache_key) {
        None | Some(TlsKeyState::Resolving(..)) => {
          if cacheable {
            cache.insert(cache_key, TlsKeyState::Resolved(res.clone()));
          } else {
            // The resolver opted out of caching this result: forget the
            // in-flight state so the next handshake resolves freshly.
            cache.remove(&cache_key);
          }
        }
        Some(TlsKeyState::Resolved(..)) => {
          // Someone beat us to it
        }
      }
      res.map_err(TlsKeyError::Failed)
    });
    Either::Right(async move { handle.await? })
  }
}

pub struct TlsKeyLookup {
  resolution_rx: RefCell<mpsc::UnboundedReceiver<ResolutionRequest>>,
  pending: RefCell<
    HashMap<String, broadcast::Sender<Result<ResolvedTlsKey, ErrorType>>>,
  >,
  cache: TlsKeyCache,
}

// SAFETY: we're sure `TlsKeyLookup` can be GCed
unsafe impl deno_core::GarbageCollected for TlsKeyLookup {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TlsKeyLookup"
  }
}

impl TlsKeyLookup {
  /// Multiple `poll` calls are safe, but this method is not starvation-safe. Generally
  /// only one `poll`er should be active at any time.
  ///
  /// Returns an opaque resolution id (pass it back to [`Self::resolve`])
  /// and the ClientHello information.
  pub async fn poll(&self) -> Option<(String, TlsClientHelloInfo)> {
    match poll_fn(|cx| self.resolution_rx.borrow_mut().poll_recv(cx)).await {
      Some((id, hello, sender)) => {
        self.pending.borrow_mut().insert(id.clone(), sender);
        Some((id, hello))
      }
      _ => None,
    }
  }

  /// Resolve a previously polled item.
  pub fn resolve(&self, id: String, res: Result<ResolvedTlsKey, String>) {
    _ = self
      .pending
      .borrow_mut()
      .remove(&id)
      .unwrap()
      .send(res.map_err(|e| Arc::new(e.into_boxed_str())));
  }

  /// Remove all cached resolutions for the given SNI hostname (regardless
  /// of the ALPN offer), causing the next TLS handshake for that hostname
  /// to trigger a fresh lookup. Used to swap certificates without
  /// restarting the listener (eg. ACME renewal).
  pub fn invalidate(&self, sni: &str) {
    self
      .cache
      .borrow_mut()
      .retain(|(key_sni, _), _| key_sni != sni);
  }
}

#[cfg(test)]
pub mod tests {
  #![allow(clippy::disallowed_methods, reason = "tests")]

  use deno_core::unsync::spawn;

  use super::*;

  fn tls_key_for_test(sni: &str) -> TlsKey {
    let manifest_dir =
      std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let sni = sni.replace(".com", "");
    let cert_file = manifest_dir.join(format!("testdata/{}_cert.der", sni));
    let prikey_file = manifest_dir.join(format!("testdata/{}_prikey.der", sni));
    let cert = std::fs::read(cert_file).unwrap();
    let prikey = std::fs::read(prikey_file).unwrap();
    let cert = CertificateDer::from(cert);
    let prikey = PrivateKeyDer::try_from(prikey).unwrap();
    TlsKey(vec![cert], prikey)
  }

  fn resolved_key_for_test(sni: &str) -> ResolvedTlsKey {
    ResolvedTlsKey {
      key: tls_key_for_test(sni),
      alpn: None,
      cache: true,
    }
  }

  fn hello(sni: &str) -> TlsClientHelloInfo {
    TlsClientHelloInfo {
      sni: sni.to_owned(),
      alpn: vec![],
    }
  }

  #[tokio::test]
  async fn test_resolve_once() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(resolved_key_for_test(&hello.sni)));
      }
    });

    let key = resolver.resolve(hello("example1.com")).await.unwrap();
    assert_eq!(resolved_key_for_test("example1.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_concurrent() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(resolved_key_for_test(&hello.sni)));
      }
    });

    let f1 = resolver.resolve(hello("example1.com"));
    let f2 = resolver.resolve(hello("example1.com"));

    let key = f1.await.unwrap();
    assert_eq!(resolved_key_for_test("example1.com"), key);
    let key = f2.await.unwrap();
    assert_eq!(resolved_key_for_test("example1.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_multiple_concurrent() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(resolved_key_for_test(&hello.sni)));
      }
    });

    let f1 = resolver.resolve(hello("example1.com"));
    let f2 = resolver.resolve(hello("example2.com"));

    let key = f1.await.unwrap();
    assert_eq!(resolved_key_for_test("example1.com"), key);
    let key = f2.await.unwrap();
    assert_eq!(resolved_key_for_test("example2.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_alpn_no_cache() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      let mut count = 0;
      while let Some((id, hello)) = lookup.poll().await {
        count += 1;
        let acme = hello.alpn == vec![b"acme-tls/1".to_vec()];
        lookup.resolve(
          id,
          Ok(ResolvedTlsKey {
            key: tls_key_for_test(&hello.sni),
            alpn: acme.then(|| vec![b"acme-tls/1".to_vec()]),
            cache: !acme,
          }),
        );
      }
      count
    });

    let acme_hello = TlsClientHelloInfo {
      sni: "example1.com".to_owned(),
      alpn: vec![b"acme-tls/1".to_vec()],
    };

    // A regular handshake is cached; the second resolve doesn't hit the
    // lookup queue.
    let key = resolver.resolve(hello("example1.com")).await.unwrap();
    assert_eq!(key.alpn, None);
    resolver.resolve(hello("example1.com")).await.unwrap();

    // `cache: false` resolutions resolve freshly every time.
    let key = resolver.resolve(acme_hello.clone()).await.unwrap();
    assert_eq!(key.alpn, Some(vec![b"acme-tls/1".to_vec()]));
    resolver.resolve(acme_hello).await.unwrap();

    drop(resolver);
    assert_eq!(task.await.unwrap(), 3);
  }

  #[tokio::test]
  async fn test_errors_are_not_cached() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      let mut count = 0;
      while let Some((id, hello)) = lookup.poll().await {
        count += 1;
        // Fail the first resolution, succeed afterwards.
        if count == 1 {
          lookup.resolve(id, Err("boom".to_owned()));
        } else {
          lookup.resolve(id, Ok(resolved_key_for_test(&hello.sni)));
        }
      }
      count
    });

    // The first resolution errors and must not be cached.
    assert!(resolver.resolve(hello("example1.com")).await.is_err());
    // The next handshake hits the lookup queue again and succeeds.
    let key = resolver.resolve(hello("example1.com")).await.unwrap();
    assert_eq!(resolved_key_for_test("example1.com"), key);

    drop(resolver);
    assert_eq!(task.await.unwrap(), 2);
  }

  #[tokio::test]
  async fn test_invalidate() {
    let (resolver, lookup) = new_resolver();

    let f1 = resolver.resolve(hello("example1.com"));
    let (id, _) = lookup.poll().await.unwrap();
    lookup.resolve(id, Ok(resolved_key_for_test("example1.com")));
    f1.await.unwrap();

    lookup.invalidate("example1.com");

    // Invalidation dropped the cached resolution, so this hits the lookup
    // queue again.
    let f2 = resolver.resolve(hello("example1.com"));
    let (id, hello_info) = lookup.poll().await.unwrap();
    assert_eq!(hello_info.sni, "example1.com");
    lookup.resolve(id, Ok(resolved_key_for_test("example1.com")));
    f2.await.unwrap();
  }
}
