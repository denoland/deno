// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub use reqwest;
pub use rustls;
pub use rustls_native_certs;
pub use webpki;
pub use webpki_roots;

use deno_core::error::anyhow;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::Extension;

use reqwest::header::HeaderMap;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Client;
use rustls::internal::msgs::handshake::DigitallySignedStruct;
use rustls::Certificate;
use rustls::ClientConfig;
use rustls::HandshakeSignatureValid;
use rustls::RootCertStore;
use rustls::ServerCertVerified;
use rustls::ServerCertVerifier;
use rustls::StoresClientSessions;
use rustls::TLSError;
use rustls::WebPKIVerifier;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Arc;
use webpki::DNSNameRef;

/// This extension has no runtime apis, it only exports some shared native functions.
pub fn init() -> Extension {
  Extension::builder().build()
}

pub struct NoCertificateVerification(pub Vec<String>);

impl ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    roots: &RootCertStore,
    presented_certs: &[Certificate],
    dns_name_ref: DNSNameRef<'_>,
    ocsp: &[u8],
  ) -> Result<ServerCertVerified, TLSError> {
    let dns_name: &str = dns_name_ref.into();
    let dns_name: String = dns_name.to_owned();
    if self.0.is_empty() || self.0.contains(&dns_name) {
      Ok(ServerCertVerified::assertion())
    } else {
      WebPKIVerifier::new().verify_server_cert(
        roots,
        presented_certs,
        dns_name_ref,
        ocsp,
      )
    }
  }

  fn verify_tls12_signature(
    &self,
    _message: &[u8],
    _cert: &Certificate,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, TLSError> {
    Ok(HandshakeSignatureValid::assertion())
  }

  fn verify_tls13_signature(
    &self,
    _message: &[u8],
    _cert: &Certificate,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, TLSError> {
    Ok(HandshakeSignatureValid::assertion())
  }
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Proxy {
  pub url: String,
  pub basic_auth: Option<BasicAuth>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct BasicAuth {
  pub username: String,
  pub password: String,
}

lazy_static::lazy_static! {
  static ref CLIENT_SESSION_MEMORY_CACHE: Arc<ClientSessionMemoryCache> =
    Arc::new(ClientSessionMemoryCache::default());
}

#[derive(Default)]
struct ClientSessionMemoryCache(Mutex<HashMap<Vec<u8>, Vec<u8>>>);

impl StoresClientSessions for ClientSessionMemoryCache {
  fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
    self.0.lock().get(key).cloned()
  }

  fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
    let mut sessions = self.0.lock();
    // TODO(bnoordhuis) Evict sessions LRU-style instead of arbitrarily.
    while sessions.len() >= 1024 {
      let key = sessions.keys().next().unwrap().clone();
      sessions.remove(&key);
    }
    sessions.insert(key, value);
    true
  }
}

pub fn create_default_root_cert_store() -> RootCertStore {
  let mut root_cert_store = RootCertStore::empty();
  // TODO(@justinmchase): Consider also loading the system keychain here
  root_cert_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
  root_cert_store
}

pub fn create_client_config(
  root_cert_store: Option<RootCertStore>,
  ca_data: Option<Vec<u8>>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
) -> Result<ClientConfig, AnyError> {
  let mut tls_config = ClientConfig::new();
  tls_config.set_persistence(CLIENT_SESSION_MEMORY_CACHE.clone());
  tls_config.root_store =
    root_cert_store.unwrap_or_else(create_default_root_cert_store);

  // If a custom cert is specified, add it to the store
  if let Some(cert) = ca_data {
    let reader = &mut BufReader::new(Cursor::new(cert));
    // This function does not return specific errors, if it fails give a generic message.
    if let Err(_err) = tls_config.root_store.add_pem_file(reader) {
      return Err(anyhow!("Unable to add pem file to certificate store"));
    }
  }

  if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    tls_config.dangerous().set_certificate_verifier(Arc::new(
      NoCertificateVerification(ic_allowlist),
    ));
  }

  Ok(tls_config)
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: String,
  root_cert_store: Option<RootCertStore>,
  ca_data: Option<Vec<u8>>,
  proxy: Option<Proxy>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
) -> Result<Client, AnyError> {
  let tls_config = create_client_config(
    root_cert_store,
    ca_data,
    unsafely_ignore_certificate_errors,
  )?;
  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, user_agent.parse().unwrap());
  let mut builder = Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_preconfigured_tls(tls_config);

  if let Some(proxy) = proxy {
    let mut reqwest_proxy = reqwest::Proxy::all(&proxy.url)?;
    if let Some(basic_auth) = &proxy.basic_auth {
      reqwest_proxy =
        reqwest_proxy.basic_auth(&basic_auth.username, &basic_auth.password);
    }
    builder = builder.proxy(reqwest_proxy);
  }

  builder
    .build()
    .map_err(|e| generic_error(format!("Unable to build http client: {}", e)))
}
