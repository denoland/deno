// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub use rustls;
pub use rustls_native_certs;
pub use rustls_pemfile;
pub use webpki;
pub use webpki_roots;

use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::Extension;

use rustls::client::HandshakeSignatureValid;
use rustls::client::ServerCertVerified;
use rustls::client::ServerCertVerifier;
use rustls::client::StoresClientSessions;
use rustls::client::WebPkiVerifier;
use rustls::internal::msgs::handshake::DigitallySignedStruct;
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
  Extension::builder(env!("CARGO_PKG_NAME")).build()
}

struct DefaultSignatureVerification;

impl ServerCertVerifier for DefaultSignatureVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &Certificate,
    _intermediates: &[Certificate],
    _server_name: &ServerName,
    _scts: &mut dyn Iterator<Item = &[u8]>,
    _ocsp_response: &[u8],
    _now: SystemTime,
  ) -> Result<ServerCertVerified, Error> {
    Err(Error::General("Should not be used".to_string()))
  }
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
    if self.0.is_empty() {
      return Ok(ServerCertVerified::assertion());
    }
    let dns_name_or_ip_address = match server_name {
      ServerName::DnsName(dns_name) => dns_name.as_ref().to_owned(),
      ServerName::IpAddress(ip_address) => ip_address.to_string(),
      _ => {
        // NOTE(bartlomieju): `ServerName` is a non-exhaustive enum
        // so we have this catch all errors here.
        return Err(Error::General("Unknown `ServerName` variant".to_string()));
      }
    };
    if self.0.contains(&dns_name_or_ip_address) {
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
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &rustls::Certificate,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    if self.0.is_empty() {
      return Ok(HandshakeSignatureValid::assertion());
    }
    filter_invalid_encoding_err(
      DefaultSignatureVerification.verify_tls12_signature(message, cert, dss),
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::Certificate,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    if self.0.is_empty() {
      return Ok(HandshakeSignatureValid::assertion());
    }
    filter_invalid_encoding_err(
      DefaultSignatureVerification.verify_tls13_signature(message, cert, dss),
    )
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
  let maybe_cert_chain_and_key =
    if let Some((cert_chain, private_key)) = client_cert_chain_and_key {
      // The `remove` is safe because load_private_keys checks that there is at least one key.
      let private_key = load_private_keys(private_key.as_bytes())?.remove(0);
      let cert_chain = load_certs(&mut cert_chain.as_bytes())?;
      Some((cert_chain, private_key))
    } else {
      None
    };

  if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    let client_config = ClientConfig::builder()
      .with_safe_defaults()
      .with_custom_certificate_verifier(Arc::new(NoCertificateVerification(
        ic_allowlist,
      )));

    // NOTE(bartlomieju): this if/else is duplicated at the end of the body of this function.
    // However it's not really feasible to deduplicate it as the `client_config` instances
    // are not type-compatible - one wants "client cert", the other wants "transparency policy
    // or client cert".
    let client =
      if let Some((cert_chain, private_key)) = maybe_cert_chain_and_key {
        client_config
          .with_single_cert(cert_chain, private_key)
          .expect("invalid client key or certificate")
      } else {
        client_config.with_no_client_auth()
      };

    return Ok(client);
  }

  let client_config = ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates({
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

  let client = if let Some((cert_chain, private_key)) = maybe_cert_chain_and_key
  {
    client_config
      .with_single_cert(cert_chain, private_key)
      .expect("invalid client key or certificate")
  } else {
    client_config.with_no_client_auth()
  };

  Ok(client)
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

fn filter_invalid_encoding_err(
  to_be_filtered: Result<HandshakeSignatureValid, Error>,
) -> Result<HandshakeSignatureValid, Error> {
  match to_be_filtered {
    Err(Error::InvalidCertificateEncoding) => {
      Ok(HandshakeSignatureValid::assertion())
    }
    res => res,
  }
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
