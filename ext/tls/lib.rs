// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub use reqwest;
pub use rustls;
pub use rustls_native_certs;
pub use rustls_pemfile;
pub use webpki;
pub use webpki_roots;

use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::Extension;

use reqwest::header::HeaderMap;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Client;
use rustls::client::ServerCertVerified;
use rustls::client::ServerCertVerifier;
use rustls::client::StoresClientSessions;
use rustls::client::WebPkiVerifier;
use rustls::Certificate;
use rustls::ClientConfig;
use rustls::Error;
use rustls::PrivateKey;
use rustls::RootCertStore;
use rustls::ServerName;
use rustls_pemfile::certs;
use rustls_pemfile::pkcs8_private_keys;
use rustls_pemfile::rsa_private_keys;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Arc;
use std::time::SystemTime;

/// This extension has no runtime apis, it only exports some shared native functions.
pub fn init() -> Extension {
  Extension::builder().build()
}

pub struct NoCertificateVerification(pub Vec<String>);

impl ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    end_entity: &Certificate,
    intermediates: &[Certificate],
    server_name: &ServerName,
    scts: &mut dyn Iterator<Item = &[u8]>,
    ocsp_response: &[u8],
    now: SystemTime,
  ) -> Result<ServerCertVerified, Error> {
    if let ServerName::DnsName(dns_name) = server_name {
      let dns_name = dns_name.as_ref().to_owned();
      if self.0.is_empty() || self.0.contains(&dns_name) {
        Ok(ServerCertVerified::assertion())
      } else {
        let root_store = create_default_root_cert_store();
        let verifier = WebPkiVerifier::new(root_store, None);
        verifier.verify_server_cert(
          end_entity,
          intermediates,
          server_name,
          scts,
          ocsp_response,
          now,
        )
      }
    } else {
      todo!()
    }
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
  root_cert_store.add_server_trust_anchors(
    webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
      rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
        ta.subject,
        ta.spki,
        ta.name_constraints,
      )
    }),
  );
  root_cert_store
}

pub fn create_client_config(
  root_cert_store: Option<RootCertStore>,
  ca_certs: Vec<Vec<u8>>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  client_cert_chain_and_key: Option<(String, String)>,
) -> Result<ClientConfig, AnyError> {
  let b = ClientConfig::builder().with_safe_defaults();

  if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    let b = b.with_custom_certificate_verifier(Arc::new(
      NoCertificateVerification(ic_allowlist),
    ));

    // TODO(ry) DUPLICATED CODE HERE.
    Ok(
      if let Some((cert_chain, private_key)) = client_cert_chain_and_key {
        // The `remove` is safe because load_private_keys checks that there is at least one key.
        let private_key = load_private_keys(private_key.as_bytes())?.remove(0);

        b.with_single_cert(load_certs(&mut cert_chain.as_bytes())?, private_key)
          .expect("invalid client key or certificate")
      } else {
        b.with_no_client_auth()
      },
    )
  } else {
    let b = b.with_root_certificates({
      let mut root_cert_store =
        root_cert_store.unwrap_or_else(create_default_root_cert_store);
      // If custom certs are specified, add them to the store
      for cert in ca_certs {
        let reader = &mut BufReader::new(Cursor::new(cert));
        // This function does not return specific errors, if it fails give a generic message.
        match rustls_pemfile::certs(reader) {
          Ok(certs) => {
            root_cert_store.add_parsable_certificates(&certs);
          }
          Err(e) => {
            return Err(anyhow!(
              "Unable to add pem file to certificate store: {}",
              e
            ));
          }
        }
      }
      root_cert_store
    });

    // TODO(ry) DUPLICATED CODE HERE.
    Ok(
      if let Some((cert_chain, private_key)) = client_cert_chain_and_key {
        // The `remove` is safe because load_private_keys checks that there is at least one key.
        let private_key = load_private_keys(private_key.as_bytes())?.remove(0);

        b.with_single_cert(load_certs(&mut cert_chain.as_bytes())?, private_key)
          .expect("invalid client key or certificate")
      } else {
        b.with_no_client_auth()
      },
    )
  }
}

pub fn load_certs(
  reader: &mut dyn BufRead,
) -> Result<Vec<Certificate>, AnyError> {
  let certs = certs(reader)
    .map_err(|_| custom_error("InvalidData", "Unable to decode certificate"))?;

  if certs.is_empty() {
    let e = custom_error("InvalidData", "No certificates found in cert file");
    return Err(e);
  }

  Ok(certs.into_iter().map(Certificate).collect())
}

fn key_decode_err() -> AnyError {
  custom_error("InvalidData", "Unable to decode key")
}

fn key_not_found_err() -> AnyError {
  custom_error("InvalidData", "No keys found in key file")
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = rsa_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys.into_iter().map(PrivateKey).collect())
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = pkcs8_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys.into_iter().map(PrivateKey).collect())
}

pub fn load_private_keys(bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let mut keys = load_rsa_keys(bytes)?;

  if keys.is_empty() {
    keys = load_pkcs8_keys(bytes)?;
  }

  if keys.is_empty() {
    return Err(key_not_found_err());
  }

  Ok(keys)
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: String,
  root_cert_store: Option<RootCertStore>,
  ca_certs: Vec<Vec<u8>>,
  proxy: Option<Proxy>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  client_cert_chain_and_key: Option<(String, String)>,
) -> Result<Client, AnyError> {
  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    client_cert_chain_and_key,
  )?;

  tls_config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];

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
