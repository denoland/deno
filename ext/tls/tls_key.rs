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
/// callback so it can pick a certificate for the requested server name.
///
/// The numeric fields are raw IANA TLS code points (the same values used by
/// fingerprinting schemes such as JA4), exposed for observability. Note that
/// resolutions are cached per server name, so for a given name these reflect
/// the ClientHello of the connection that first triggered a lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlsClientHelloInfo {
  /// The server name requested via SNI (empty if the client sent none).
  pub sni: String,
  /// The ALPN protocols offered by the client, in order of preference.
  pub alpn: Vec<Vec<u8>>,
  /// The cipher suites offered by the client, as IANA code points.
  pub cipher_suites: Vec<u16>,
  /// The signature schemes offered by the client, as IANA code points.
  pub signature_schemes: Vec<u16>,
  /// The named groups ("supported groups") offered by the client, as IANA
  /// code points.
  pub supported_groups: Vec<u16>,
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
  Resolving(broadcast::Receiver<Result<TlsKey, ErrorType>>),
  Resolved(Result<TlsKey, ErrorType>),
}

/// Cache key for a resolution: the requested server name (SNI). Resolutions
/// are cached per name, matching the documented `resolveCertificate`
/// contract.
type CacheKey = String;

/// Upper bound on the number of cached resolutions. The cache key (SNI) is
/// client-controlled, so without a bound a peer varying it could grow the
/// cache without limit. Errors are never cached (see
/// [`TlsKeyResolver::resolve`]), so only successful resolutions count toward
/// this; when the cache is full the oldest entry is evicted and a later
/// handshake for it simply re-resolves.
const MAX_CACHED_RESOLUTIONS: usize = 1024;

struct CacheEntry {
  state: TlsKeyState,
  /// Insertion-order stamp, used to evict the oldest entry when the cache is
  /// full.
  seq: u64,
}

/// A size-bounded cache of TLS key resolutions, keyed by [`CacheKey`].
#[derive(Default)]
struct BoundedTlsKeyCache {
  entries: HashMap<CacheKey, CacheEntry>,
  next_seq: u64,
}

impl BoundedTlsKeyCache {
  fn get(&self, key: &CacheKey) -> Option<&TlsKeyState> {
    self.entries.get(key).map(|entry| &entry.state)
  }

  fn insert(&mut self, key: CacheKey, state: TlsKeyState) {
    // Evict the oldest entry if inserting a new key would exceed the bound.
    if self.entries.len() >= MAX_CACHED_RESOLUTIONS
      && !self.entries.contains_key(&key)
      && let Some(oldest) = self
        .entries
        .iter()
        .min_by_key(|(_, entry)| entry.seq)
        .map(|(oldest_key, _)| oldest_key.clone())
    {
      self.entries.remove(&oldest);
    }
    let seq = self.next_seq;
    self.next_seq = self.next_seq.wrapping_add(1);
    self.entries.insert(key, CacheEntry { state, seq });
  }

  fn remove(&mut self, key: &CacheKey) {
    self.entries.remove(key);
  }
}

type TlsKeyCache = Rc<RefCell<BoundedTlsKeyCache>>;

type ResolutionRequest = (
  String,
  TlsClientHelloInfo,
  broadcast::Sender<Result<TlsKey, ErrorType>>,
);

struct TlsKeyResolverInner {
  resolution_tx: mpsc::UnboundedSender<ResolutionRequest>,
  cache: TlsKeyCache,
  // Monotonic counter for the opaque resolution id handed to the lookup side
  // (and round-tripped through JS), decoupled from the cache key.
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
    let key = self.resolve(hello).await?;

    let mut tls_config = ServerConfig::builder()
      .with_no_client_auth()
      .with_single_cert(key.0, key.1.clone_key())?;
    tls_config.key_log = get_ssl_key_log();
    tls_config.alpn_protocols = alpn;
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
        cipher_suites: hello
          .cipher_suites()
          .iter()
          .map(|cs| u16::from(*cs))
          .collect(),
        signature_schemes: hello
          .signature_schemes()
          .iter()
          .map(|ss| u16::from(*ss))
          .collect(),
        supported_groups: hello
          .named_groups()
          .unwrap_or_default()
          .iter()
          .map(|g| u16::from(*g))
          .collect(),
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
        cache,
        next_id: Cell::new(0),
      }),
    },
    TlsKeyLookup {
      resolution_rx: RefCell::new(resolution_rx),
      pending: Default::default(),
    },
  )
}

impl TlsKeyResolver {
  /// Resolve the certificate and key for a given ClientHello. This immediately spawns a task
  /// in the background and is therefore cancellation-safe.
  pub fn resolve(
    &self,
    hello: TlsClientHelloInfo,
  ) -> impl Future<Output = Result<TlsKey, TlsKeyError>> + use<> {
    let cache_key = hello.sni.clone();
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
      // Never cache errors: otherwise a peer varying the SNI could grow the
      // cache without bound with failed resolutions, and a transient failure
      // would be sticky. The next handshake retries instead.
      let cacheable = res.is_ok();
      match cache.get(&cache_key) {
        None | Some(TlsKeyState::Resolving(..)) => {
          if cacheable {
            cache.insert(cache_key, TlsKeyState::Resolved(res.clone()));
          } else {
            // The resolution failed: forget the in-flight state so the next
            // handshake resolves freshly.
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
  pending:
    RefCell<HashMap<String, broadcast::Sender<Result<TlsKey, ErrorType>>>>,
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
  pub fn resolve(&self, id: String, res: Result<TlsKey, String>) {
    _ = self
      .pending
      .borrow_mut()
      .remove(&id)
      .unwrap()
      .send(res.map_err(|e| Arc::new(e.into_boxed_str())));
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

  fn hello(sni: &str) -> TlsClientHelloInfo {
    TlsClientHelloInfo {
      sni: sni.to_owned(),
      ..Default::default()
    }
  }

  #[tokio::test]
  async fn test_resolve_once() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(tls_key_for_test(&hello.sni)));
      }
    });

    let key = resolver.resolve(hello("example1.com")).await.unwrap();
    assert_eq!(tls_key_for_test("example1.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_concurrent() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(tls_key_for_test(&hello.sni)));
      }
    });

    let f1 = resolver.resolve(hello("example1.com"));
    let f2 = resolver.resolve(hello("example1.com"));

    let key = f1.await.unwrap();
    assert_eq!(tls_key_for_test("example1.com"), key);
    let key = f2.await.unwrap();
    assert_eq!(tls_key_for_test("example1.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_multiple_concurrent() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      while let Some((id, hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(tls_key_for_test(&hello.sni)));
      }
    });

    let f1 = resolver.resolve(hello("example1.com"));
    let f2 = resolver.resolve(hello("example2.com"));

    let key = f1.await.unwrap();
    assert_eq!(tls_key_for_test("example1.com"), key);
    let key = f2.await.unwrap();
    assert_eq!(tls_key_for_test("example2.com"), key);
    drop(resolver);

    task.await.unwrap();
  }

  #[tokio::test]
  async fn test_resolve_cached_by_name() {
    let (resolver, lookup) = new_resolver();
    let task = spawn(async move {
      let mut count = 0;
      while let Some((id, hello)) = lookup.poll().await {
        count += 1;
        lookup.resolve(id, Ok(tls_key_for_test(&hello.sni)));
      }
      count
    });

    // The same name resolves once and is then served from cache, even when
    // the ALPN offer differs between connections.
    resolver.resolve(hello("example1.com")).await.unwrap();
    let with_alpn = TlsClientHelloInfo {
      sni: "example1.com".to_owned(),
      alpn: vec![b"h2".to_vec()],
      ..Default::default()
    };
    resolver.resolve(with_alpn).await.unwrap();

    drop(resolver);
    assert_eq!(task.await.unwrap(), 1);
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
          lookup.resolve(id, Ok(tls_key_for_test(&hello.sni)));
        }
      }
      count
    });

    // The first resolution errors and must not be cached.
    assert!(resolver.resolve(hello("example1.com")).await.is_err());
    // The next handshake hits the lookup queue again and succeeds.
    let key = resolver.resolve(hello("example1.com")).await.unwrap();
    assert_eq!(tls_key_for_test("example1.com"), key);

    drop(resolver);
    assert_eq!(task.await.unwrap(), 2);
  }

  #[tokio::test]
  async fn test_cache_is_bounded() {
    let (resolver, lookup) = new_resolver();
    // Reuse a single loaded cert for every resolution; only the cache key
    // (the SNI) varies, so each resolve adds a distinct cache entry.
    let key = tls_key_for_test("example1.com");
    let task = spawn(async move {
      while let Some((id, _hello)) = lookup.poll().await {
        lookup.resolve(id, Ok(key.clone()));
      }
    });

    // Resolve more distinct SNIs than the cache can hold.
    for i in 0..(MAX_CACHED_RESOLUTIONS + 50) {
      resolver
        .resolve(hello(&format!("example{i}.com")))
        .await
        .unwrap();
    }

    // The cache stays bounded; older entries were evicted.
    assert_eq!(
      resolver.inner.cache.borrow().entries.len(),
      MAX_CACHED_RESOLUTIONS
    );

    drop(resolver);
    task.await.unwrap();
  }
}
