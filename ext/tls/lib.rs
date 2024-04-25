// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use deno_native_certs;
use deno_native_certs::load_native_certs;
pub use rustls;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::Der;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::PrivatePkcs1KeyDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use rustls::pki_types::PrivateSec1KeyDer;
use rustls::pki_types::ServerName;
use rustls::pki_types::TrustAnchor;
pub use rustls_pemfile;
pub use rustls_tokio_stream::*;
pub use webpki;
pub use webpki_roots;

use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;

use rustls::client::danger::HandshakeSignatureValid;
use rustls::client::danger::ServerCertVerified;
use rustls::client::danger::ServerCertVerifier;
use rustls::client::WebPkiServerVerifier;
use rustls::ClientConfig;
use rustls::DigitallySignedStruct;
use rustls::Error;
use rustls_pemfile::certs;
use rustls_pemfile::ec_private_keys;
use rustls_pemfile::pkcs8_private_keys;
use rustls_pemfile::rsa_private_keys;
use serde::Deserialize;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::net::IpAddr;
use std::sync::Arc;

pub type Certificate = rustls::pki_types::CertificateDer<'static>;
pub type PrivateKey = rustls::pki_types::PrivateKeyDer<'static>;
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

#[derive(Debug)]
struct DefaultSignatureVerification;

impl ServerCertVerifier for DefaultSignatureVerification {
  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    vec![]
  }
  fn verify_server_cert(
    &self,
    _end_entity: &rustls::pki_types::CertificateDer<'_>,
    _intermediates: &[rustls::pki_types::CertificateDer<'_>],
    _server_name: &rustls::pki_types::ServerName<'_>,
    _ocsp_response: &[u8],
    _now: rustls::pki_types::UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    Err(Error::General("Should not be used".to_string()))
  }
  fn verify_tls12_signature(
    &self,
    _message: &[u8],
    _cert: &rustls::pki_types::CertificateDer<'_>,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    Err(Error::General("Should not be used".to_string()))
  }
  fn verify_tls13_signature(
    &self,
    _message: &[u8],
    _cert: &rustls::pki_types::CertificateDer<'_>,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    Err(Error::General("Should not be used".to_string()))
  }
}

#[derive(Debug)]
pub struct NoCertificateVerification(pub Vec<String>);

impl ServerCertVerifier for NoCertificateVerification {
  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    let root_store = create_default_root_cert_store();
    let verifier = WebPkiServerVerifier::builder(root_store.into())
      .build()
      .unwrap();
    verifier.supported_verify_schemes()
  }
  fn verify_server_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    server_name: &rustls::pki_types::ServerName<'_>,
    ocsp_response: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    if self.0.is_empty() {
      return Ok(ServerCertVerified::assertion());
    }
    let dns_name_or_ip_address = match server_name {
      ServerName::DnsName(dns_name) => dns_name.as_ref().to_owned(),
      ServerName::IpAddress(ip_address) => {
        Into::<IpAddr>::into(*ip_address).to_string()
      }
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
      let verifier = WebPkiServerVerifier::builder(root_store.into())
        .build()
        .unwrap();
      verifier.verify_server_cert(
        end_entity,
        intermediates,
        server_name,
        ocsp_response,
        now,
      )
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer,
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
    cert: &rustls::pki_types::CertificateDer,
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
  for ta in webpki_roots::TLS_SERVER_ROOTS {
    root_cert_store.roots.push(TrustAnchor {
      name_constraints: ta.name_constraints.map(|x| Der::from(x).to_owned()),
      subject: ta.subject.into(),
      subject_public_key_info: ta.spki.into(),
    });
  }
  debug_assert!(!root_cert_store.is_empty());
  root_cert_store
}

pub fn create_platform_cert_store() -> RootCertStore {
  let mut root_cert_store = RootCertStore::empty();
  let roots: Vec<deno_native_certs::Certificate> =
    load_native_certs().expect("could not load platform certs");
  for root in roots {
    root_cert_store
      .add(Certificate::from(root.0))
      .expect("Failed to add platform cert to root cert store");
  }
  debug_assert!(!root_cert_store.is_empty());
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
  maybe_cert_chain_and_key: Option<TlsKey>,
  socket_use: SocketUse,
) -> Result<ClientConfig, AnyError> {
  if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    let client_config = ClientConfig::builder()
      .dangerous()
      .with_custom_certificate_verifier(Arc::new(NoCertificateVerification(
        ic_allowlist,
      )));

    // NOTE(bartlomieju): this if/else is duplicated at the end of the body of this function.
    // However it's not really feasible to deduplicate it as the `client_config` instances
    // are not type-compatible - one wants "client cert", the other wants "transparency policy
    // or client cert".
    let mut client =
      if let Some(TlsKey(cert_chain, private_key)) = maybe_cert_chain_and_key {
        client_config
          .with_client_auth_cert(cert_chain, private_key)
          .expect("invalid client key or certificate")
      } else {
        client_config.with_no_client_auth()
      };

    add_alpn(&mut client, socket_use);
    return Ok(client);
  }

  let client_config = ClientConfig::builder().with_root_certificates({
    let mut root_cert_store =
      root_cert_store.unwrap_or_else(create_default_root_cert_store);
    // If custom certs are specified, add them to the store
    for cert in ca_certs {
      let reader = &mut BufReader::new(Cursor::new(cert));
      // This function does not return specific errors, if it fails give a generic message.
      match rustls_pemfile::certs(reader) {
        Ok(certs) => {
          for cert in certs {
            root_cert_store.add(CertificateDer::from(cert))?;
          }
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

  let mut client =
    if let Some(TlsKey(cert_chain, private_key)) = maybe_cert_chain_and_key {
      client_config
        .with_client_auth_cert(cert_chain, private_key)
        .expect("invalid client key or certificate")
    } else {
      client_config.with_no_client_auth()
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

  Ok(certs.into_iter().map(CertificateDer::from).collect())
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
  Ok(
    keys
      .into_iter()
      .map(|x| PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(x)))
      .collect(),
  )
}

/// Starts with -----BEGIN EC PRIVATE KEY-----
fn load_ec_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = ec_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(
    keys
      .into_iter()
      .map(|x| PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(x)))
      .collect(),
  )
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = pkcs8_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(
    keys
      .into_iter()
      .map(|x| PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(x)))
      .collect(),
  )
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

pub fn load_private_keys(
  bytes: &[u8],
) -> Result<Vec<PrivateKeyDer<'static>>, AnyError> {
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

/// A loaded key.
// FUTURE(mmastrac): add resolver enum value to support dynamic SNI
pub enum TlsKeys {
  // TODO(mmastrac): We need Option<&T> for cppgc -- this is a workaround
  Null,
  Static(TlsKey),
}

/// A TLS certificate/private key pair.
#[derive(Debug)]
pub struct TlsKey(pub Vec<CertificateDer<'static>>, pub PrivateKeyDer<'static>);

impl Clone for TlsKey {
  fn clone(&self) -> Self {
    Self(self.0.clone(), self.1.clone_key())
  }
}
