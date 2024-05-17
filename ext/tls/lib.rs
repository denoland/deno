// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use deno_native_certs;
pub use rustls;
pub use rustls_pemfile;
pub use rustls_tokio_stream::*;
pub use webpki;
pub use webpki_roots;

use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;

use rustls::client::HandshakeSignatureValid;
use rustls::client::ServerCertVerified;
use rustls::client::ServerCertVerifier;
use rustls::client::WebPkiVerifier;
use rustls::ClientConfig;
use rustls::DigitallySignedStruct;
use rustls::Error;
use rustls::ServerName;
use rustls_pemfile::certs;
use rustls_pemfile::ec_private_keys;
use rustls_pemfile::pkcs8_private_keys;
use rustls_pemfile::rsa_private_keys;
use serde::Deserialize;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Arc;
use std::time::SystemTime;

mod tls_key;
pub use tls_key::*;

pub type Certificate = rustls::Certificate;
pub type PrivateKey = rustls::PrivateKey;
pub type RootCertStore = rustls::RootCertStore;

/// Lazily resolves the root cert store.
///
/// This was done because the root cert store is not needed in all cases
/// and takes a bit of time to initialize.
pub trait RootCertStoreProvider: Send + Sync {
  fn get_or_try_init(&self) -> Result<&RootCertStore, AnyError>;
}

// This extension has no runtime apis, it only exports some shared native functions.
deno_core::extension!(deno_tls);

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

pub fn create_default_root_cert_store() -> RootCertStore {
  let mut root_cert_store = RootCertStore::empty();
  // TODO(@justinmchase): Consider also loading the system keychain here
  root_cert_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(
    |ta| {
      rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
        ta.subject,
        ta.spki,
        ta.name_constraints,
      )
    },
  ));
  root_cert_store
}

pub enum SocketUse {
  /// General SSL: No ALPN
  GeneralSsl,
  /// HTTP: h1 and h2
  Http,
  /// http/1.1 only
  Http1Only,
  /// http/2 only
  Http2Only,
}

pub fn create_client_config(
  root_cert_store: Option<RootCertStore>,
  ca_certs: Vec<Vec<u8>>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  maybe_cert_chain_and_key: TlsKeys,
  socket_use: SocketUse,
) -> Result<ClientConfig, AnyError> {
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
    let mut client = match maybe_cert_chain_and_key {
      TlsKeys::Static(TlsKey(cert_chain, private_key)) => client_config
        .with_client_auth_cert(cert_chain, private_key)
        .expect("invalid client key or certificate"),
      TlsKeys::Null => client_config.with_no_client_auth(),
      TlsKeys::Resolver(_) => unimplemented!(),
    };

    add_alpn(&mut client, socket_use);
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

  let mut client = match maybe_cert_chain_and_key {
    TlsKeys::Static(TlsKey(cert_chain, private_key)) => client_config
      .with_client_auth_cert(cert_chain, private_key)
      .expect("invalid client key or certificate"),
    TlsKeys::Null => client_config.with_no_client_auth(),
    TlsKeys::Resolver(_) => unimplemented!(),
  };

  add_alpn(&mut client, socket_use);
  Ok(client)
}

fn add_alpn(client: &mut ClientConfig, socket_use: SocketUse) {
  match socket_use {
    SocketUse::Http1Only => {
      client.alpn_protocols = vec!["http/1.1".into()];
    }
    SocketUse::Http2Only => {
      client.alpn_protocols = vec!["h2".into()];
    }
    SocketUse::Http => {
      client.alpn_protocols = vec!["h2".into(), "http/1.1".into()];
    }
    SocketUse::GeneralSsl => {}
  };
}

pub fn load_certs(
  reader: &mut dyn BufRead,
) -> Result<Vec<Certificate>, AnyError> {
  let certs = certs(reader)
    .map_err(|_| custom_error("InvalidData", "Unable to decode certificate"))?;

  if certs.is_empty() {
    return Err(cert_not_found_err());
  }

  Ok(certs.into_iter().map(rustls::Certificate).collect())
}

fn key_decode_err() -> AnyError {
  custom_error("InvalidData", "Unable to decode key")
}

fn key_not_found_err() -> AnyError {
  custom_error("InvalidData", "No keys found in key data")
}

fn cert_not_found_err() -> AnyError {
  custom_error("InvalidData", "No certificates found in certificate data")
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = rsa_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys.into_iter().map(rustls::PrivateKey).collect())
}

/// Starts with -----BEGIN EC PRIVATE KEY-----
fn load_ec_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = ec_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys.into_iter().map(rustls::PrivateKey).collect())
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = pkcs8_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys.into_iter().map(rustls::PrivateKey).collect())
}

fn filter_invalid_encoding_err(
  to_be_filtered: Result<HandshakeSignatureValid, Error>,
) -> Result<HandshakeSignatureValid, Error> {
  match to_be_filtered {
    Err(Error::InvalidCertificate(rustls::CertificateError::BadEncoding)) => {
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
    keys = load_ec_keys(bytes)?;
  }

  if keys.is_empty() {
    return Err(key_not_found_err());
  }

  Ok(keys)
}
