// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::net::IpAddr;
use std::sync::Arc;

use deno_error::JsErrorBox;
pub use deno_native_certs;
pub use rustls;
use rustls::ClientConfig;
use rustls::DigitallySignedStruct;
use rustls::RootCertStore;
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::client::danger::ServerCertVerified;
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::ServerName;
pub use rustls_pemfile;
use rustls_pemfile::certs;
use rustls_pemfile::ec_private_keys;
use rustls_pemfile::pkcs8_private_keys;
use rustls_pemfile::rsa_private_keys;
pub use rustls_tokio_stream::*;
use serde::Deserialize;
pub use webpki;
pub use webpki_roots;

mod keylog;
mod tls_key;
pub use keylog::get_ssl_key_log;
pub use tls_key::*;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TlsError {
  #[class(generic)]
  #[error(transparent)]
  Rustls(#[from] rustls::Error),
  #[class(inherit)]
  #[error("Unable to add pem file to certificate store: {0}")]
  UnableAddPemFileToCert(std::io::Error),
  #[class("InvalidData")]
  #[error("Unable to decode certificate")]
  CertInvalid,
  #[class("InvalidData")]
  #[error("No certificates found in certificate data")]
  CertsNotFound,
  #[class("InvalidData")]
  #[error("No keys found in key data")]
  KeysNotFound,
  #[class("InvalidData")]
  #[error("Unable to decode key")]
  KeyDecode,
}

/// Lazily resolves the root cert store.
///
/// This was done because the root cert store is not needed in all cases
/// and takes a bit of time to initialize.
pub trait RootCertStoreProvider: Send + Sync {
  fn get_or_try_init(&self) -> Result<&RootCertStore, JsErrorBox>;
}

/// Runtime-mutable override of the root CA cert set, shared across TLS-using
/// extensions (`ext/fetch`, `ext/node`, …).  When `set()`-ed, replaces the
/// default root cert store for newly-built TLS client configs.  Backs
/// `node:tls.setDefaultCACertificates()` so a setter call in JS can affect
/// subsequent `fetch()` requests and `tls.connect()` calls in the same
/// process.
#[derive(Debug, Default, Clone)]
pub struct RuntimeRootCertOverride {
  state: Arc<std::sync::Mutex<RuntimeRootCertOverrideState>>,
}

#[derive(Debug, Default)]
struct RuntimeRootCertOverrideState {
  certs: Option<Vec<String>>,
  // Bumped on every `set()`.  Consumers (e.g. the fetch Client cache)
  // compare against the version they were built with to know if they
  // need to rebuild.
  version: u64,
}

impl RuntimeRootCertOverride {
  pub fn set(&self, certs: Vec<String>) {
    let mut state = self.state.lock().unwrap();
    state.certs = Some(certs);
    state.version = state.version.wrapping_add(1);
  }

  pub fn certs(&self) -> Option<Vec<String>> {
    self.state.lock().unwrap().certs.clone()
  }

  pub fn version(&self) -> u64 {
    self.state.lock().unwrap().version
  }
}

// This extension has no runtime apis, it only exports some shared native functions.
deno_core::extension!(
  deno_tls,
  state = |_state| {
    // Resolve `SSLKEYLOGFILE` before user JavaScript can mutate env vars.
    keylog::init_ssl_key_log();
  },
);

#[derive(Debug)]
pub struct NoCertificateVerification {
  pub ic_allowlist: Vec<String>,
  default_verifier: Arc<WebPkiServerVerifier>,
}

impl NoCertificateVerification {
  pub fn new(ic_allowlist: Vec<String>) -> Self {
    Self {
      ic_allowlist,
      default_verifier: WebPkiServerVerifier::builder(
        create_default_root_cert_store().into(),
      )
      .build()
      .unwrap(),
    }
  }
}

impl ServerCertVerifier for NoCertificateVerification {
  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.default_verifier.supported_verify_schemes()
  }

  fn verify_server_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    server_name: &rustls::pki_types::ServerName<'_>,
    ocsp_response: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    if self.ic_allowlist.is_empty() {
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
        return Err(rustls::Error::General(
          "Unknown `ServerName` variant".to_string(),
        ));
      }
    };
    if self.ic_allowlist.contains(&dns_name_or_ip_address) {
      Ok(ServerCertVerified::assertion())
    } else {
      self.default_verifier.verify_server_cert(
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
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    if self.ic_allowlist.is_empty() {
      return Ok(HandshakeSignatureValid::assertion());
    }
    filter_invalid_encoding_err(
      self
        .default_verifier
        .verify_tls12_signature(message, cert, dss),
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    if self.ic_allowlist.is_empty() {
      return Ok(HandshakeSignatureValid::assertion());
    }
    filter_invalid_encoding_err(
      self
        .default_verifier
        .verify_tls13_signature(message, cert, dss),
    )
  }
}

#[derive(Debug)]
pub struct NoServerNameVerification {
  inner: Arc<WebPkiServerVerifier>,
}

impl NoServerNameVerification {
  pub fn new(inner: Arc<WebPkiServerVerifier>) -> Self {
    Self { inner }
  }
}

impl ServerCertVerifier for NoServerNameVerification {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    server_name: &ServerName<'_>,
    ocsp: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    match self.inner.verify_server_cert(
      end_entity,
      intermediates,
      server_name,
      ocsp,
      now,
    ) {
      Ok(scv) => Ok(scv),
      Err(rustls::Error::InvalidCertificate(cert_error)) => {
        if matches!(
          cert_error,
          rustls::CertificateError::NotValidForName
            | rustls::CertificateError::NotValidForNameContext { .. }
        ) {
          Ok(ServerCertVerified::assertion())
        } else {
          Err(rustls::Error::InvalidCertificate(cert_error))
        }
      }
      Err(e) => Err(e),
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.inner.verify_tls12_signature(message, cert, dss)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.inner.verify_tls13_signature(message, cert, dss)
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "transport")]
pub enum Proxy {
  #[serde(rename_all = "camelCase")]
  Http {
    url: String,
    basic_auth: Option<BasicAuth>,
  },
  Tcp {
    hostname: String,
    port: u16,
  },
  Unix {
    path: String,
  },
  Vsock {
    cid: u32,
    port: u32,
  },
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct BasicAuth {
  pub username: String,
  pub password: String,
}

pub fn create_default_root_cert_store() -> RootCertStore {
  let root_cert_store = rustls::RootCertStore {
    roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
  };
  debug_assert!(!root_cert_store.is_empty());
  root_cert_store
}

#[derive(Default)]
pub enum SocketUse {
  /// General SSL: No ALPN
  #[default]
  GeneralSsl,
  /// HTTP: h1 and h2
  Http,
  /// http/1.1 only
  Http1Only,
  /// http/2 only
  Http2Only,
}

#[derive(Default)]
pub struct TlsClientConfigOptions {
  pub root_cert_store: Option<RootCertStore>,
  pub ca_certs: Vec<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub unsafely_disable_hostname_verification: bool,
  pub cert_chain_and_key: TlsKeys,
  pub socket_use: SocketUse,
  /// Node-compat: when true, install a server-cert verifier that accepts
  /// X.509v1 certificates if their chain validates structurally.  webpki
  /// (and therefore rustls's default verifier) rejects v1 outright, but
  /// OpenSSL — and so Node — accepts them.  Set by `fetch()` and similar
  /// Node-compat code paths so test fixtures and legacy peers using v1
  /// certs still work.
  pub tolerate_legacy_cert_versions: bool,
}

pub fn create_client_config(
  options: TlsClientConfigOptions,
) -> Result<ClientConfig, TlsError> {
  let TlsClientConfigOptions {
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    unsafely_disable_hostname_verification,
    cert_chain_and_key: maybe_cert_chain_and_key,
    socket_use,
    tolerate_legacy_cert_versions,
  } = options;
  if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    let client_config = ClientConfig::builder()
      .dangerous()
      .with_custom_certificate_verifier(Arc::new(
        NoCertificateVerification::new(ic_allowlist),
      ));

    // NOTE(bartlomieju): this if/else is duplicated at the end of the body of this function.
    // However it's not really feasible to deduplicate it as the `client_config` instances
    // are not type-compatible - one wants "client cert", the other wants "transparency policy
    // or client cert".
    let mut client = match maybe_cert_chain_and_key {
      TlsKeys::Static(TlsKey(cert_chain, private_key)) => client_config
        .with_client_auth_cert(cert_chain, private_key.clone_key())
        .expect("invalid client key or certificate"),
      TlsKeys::Null => client_config.with_no_client_auth(),
      TlsKeys::Resolver(_) => unimplemented!(),
    };

    client.key_log = get_ssl_key_log();
    add_alpn(&mut client, socket_use);
    return Ok(client);
  }

  let mut root_cert_store =
    root_cert_store.unwrap_or_else(create_default_root_cert_store);
  // Raw DER bytes for the legacy-version tolerant verifier, when enabled.
  // We collect them only for explicit `ca_certs` (the override PEM bytes
  // passed in by the caller); roots loaded via `root_cert_store` are kept
  // as `TrustAnchor`s and don't expose raw cert DERs, but for the v1
  // structural-chain check the subject DER on `TrustAnchor` is enough,
  // and that's accessed below directly from the store.
  let mut explicit_root_cert_ders: Vec<Vec<u8>> = Vec::new();
  // If custom certs are specified, add them to the store
  for cert in ca_certs {
    let reader = &mut BufReader::new(Cursor::new(cert));
    // This function does not return specific errors, if it fails give a generic message.
    for r in rustls_pemfile::certs(reader) {
      match r {
        Ok(cert) => {
          if tolerate_legacy_cert_versions {
            explicit_root_cert_ders.push(cert.as_ref().to_vec());
          }
          root_cert_store.add(cert)?;
        }
        Err(e) => {
          return Err(TlsError::UnableAddPemFileToCert(e));
        }
      }
    }
  }

  let client_config =
    ClientConfig::builder().with_root_certificates(root_cert_store.clone());

  let mut client = match maybe_cert_chain_and_key {
    TlsKeys::Static(TlsKey(cert_chain, private_key)) => client_config
      .with_client_auth_cert(cert_chain, private_key.clone_key())
      .expect("invalid client key or certificate"),
    TlsKeys::Null => client_config.with_no_client_auth(),
    TlsKeys::Resolver(_) => unimplemented!(),
  };

  client.key_log = get_ssl_key_log();
  add_alpn(&mut client, socket_use);

  if unsafely_disable_hostname_verification {
    let inner =
      rustls::client::WebPkiServerVerifier::builder(Arc::new(root_cert_store))
        .build()
        .expect("Failed to create WebPkiServerVerifier");
    let verifier = Arc::new(NoServerNameVerification::new(inner));
    client.dangerous().set_certificate_verifier(verifier);
  } else if tolerate_legacy_cert_versions && !root_cert_store.is_empty() {
    // Wrap the default verifier with one that accepts X.509v1 certs
    // (which OpenSSL — and so Node — accepts) when the chain validates
    // structurally.  rustls/webpki otherwise refuse v1 outright.
    //
    // Skip when the root store is empty: webpki's
    // `WebPkiServerVerifier::builder().build()` errors with
    // `NoRootAnchors`, and there's nothing for the legacy verifier to
    // structurally chain against anyway — leaving the default empty-store
    // behaviour in place (every cert is rejected) matches what callers
    // expect from `RootCertStore::empty()`.
    let mut trusted_subjects: Vec<Vec<u8>> = root_cert_store
      .roots
      .iter()
      .map(|ta| ta.subject.as_ref().to_vec())
      .collect();
    // Also include any explicit cert DERs (extracted above from
    // `ca_certs`).  `extract_issuer_and_subject` returns the SEQUENCE
    // content (matching `TrustAnchor::subject`) so the two sources are
    // directly comparable.
    for der in &explicit_root_cert_ders {
      if let Some((_, subject)) = extract_issuer_and_subject(der) {
        trusted_subjects.push(subject.to_vec());
      }
    }
    let inner =
      rustls::client::WebPkiServerVerifier::builder(Arc::new(root_cert_store))
        .build()
        .expect("Failed to create WebPkiServerVerifier");
    let verifier =
      Arc::new(LegacyVersionTolerantVerifier::new(inner, trusted_subjects));
    client.dangerous().set_certificate_verifier(verifier);
  }

  Ok(client)
}

/// Wraps a `WebPkiServerVerifier` to recover from `UnsupportedCertVersion` /
/// `BadEncoding` errors that webpki raises for X.509v1 certificates.  The
/// chain is otherwise verified by webpki; this verifier only swallows the
/// version error if the leaf's chain reaches a trusted root via structural
/// (issuer/subject) matching.  This matches OpenSSL/Node behaviour for the
/// legacy test fixtures used in the Node compatibility suite.
#[derive(Debug)]
pub struct LegacyVersionTolerantVerifier {
  inner: Arc<WebPkiServerVerifier>,
  /// Subject DERs (the content of each root cert's `Name` SEQUENCE, not
  /// including the outer SEQUENCE header — matching `TrustAnchor::subject`)
  /// of every trusted root.  Used for the structural chain walk when webpki
  /// refuses a v1 leaf and we need to decide whether the chain still
  /// reaches a known root.
  trusted_subjects: Vec<Vec<u8>>,
}

impl LegacyVersionTolerantVerifier {
  pub fn new(
    inner: Arc<WebPkiServerVerifier>,
    trusted_subjects: Vec<Vec<u8>>,
  ) -> Self {
    Self {
      inner,
      trusted_subjects,
    }
  }
}

fn is_unsupported_cert_version(err: &rustls::Error) -> bool {
  use rustls::CertificateError as CE;
  match err {
    rustls::Error::InvalidCertificate(CE::BadEncoding) => true,
    rustls::Error::InvalidCertificate(CE::Other(other)) => {
      let matched = other
        .0
        .downcast_ref::<webpki::Error>()
        .is_some_and(|e| matches!(e, webpki::Error::UnsupportedCertVersion));
      // Fallback: webpki::Error sometimes hides behind an extra layer
      // (e.g. `OtherError(...)`), losing the direct downcast.  The Display
      // is preserved, so check it.
      matched || format!("{other}").contains("UnsupportedCertVersion")
    }
    _ => false,
  }
}

// Minimal DER helpers for chain verification of X.509v1 certificates.
// webpki rejects v1 certs at parse time, so we do structural chain
// checking ourselves (issuer/subject matching).  See ext/node for the
// canonical implementation that this is a copy of.

fn der_read_element(data: &[u8]) -> Option<(&[u8], &[u8])> {
  if data.is_empty() {
    return None;
  }
  let first_len = *data.get(1)?;
  let (content_len, header_len) = if first_len < 0x80 {
    (first_len as usize, 2)
  } else {
    let num_bytes = (first_len & 0x7F) as usize;
    if num_bytes == 0 || num_bytes > 4 || data.len() < 2 + num_bytes {
      return None;
    }
    let mut len = 0usize;
    for i in 0..num_bytes {
      len = (len << 8) | (data[2 + i] as usize);
    }
    (len, 2 + num_bytes)
  };
  let total = header_len + content_len;
  if data.len() < total {
    return None;
  }
  Some((&data[..total], &data[total..]))
}

fn der_skip_element(data: &[u8]) -> Option<&[u8]> {
  der_read_element(data).map(|(_, rest)| rest)
}

fn der_content_len(element: &[u8]) -> Option<usize> {
  let first_len = *element.get(1)?;
  if first_len < 0x80 {
    Some(first_len as usize)
  } else {
    let num_bytes = (first_len & 0x7F) as usize;
    let mut len = 0usize;
    for i in 0..num_bytes {
      len = (len << 8) | (*element.get(2 + i)? as usize);
    }
    Some(len)
  }
}

/// Return the SEQUENCE-content (without the outer tag/length header) of
/// the `Name` element `el` (a DER SEQUENCE).  Matches the format that
/// rustls/webpki stores in `TrustAnchor::subject`, so the two can be
/// compared directly by byte equality.
fn der_content_of_sequence(el: &[u8]) -> Option<&[u8]> {
  let content_len = der_content_len(el)?;
  let header_len = el.len().checked_sub(content_len)?;
  el.get(header_len..)
}

fn extract_issuer_and_subject(cert_der: &[u8]) -> Option<(&[u8], &[u8])> {
  let (cert_elem, _) = der_read_element(cert_der)?;
  let tbs_content = &cert_elem[cert_elem.len() - der_content_len(cert_elem)?..];
  let (tbs_elem, _) = der_read_element(tbs_content)?;
  let mut pos = &tbs_elem[tbs_elem.len() - der_content_len(tbs_elem)?..];
  if pos.first() == Some(&0xA0) {
    pos = der_skip_element(pos)?;
  }
  pos = der_skip_element(pos)?; // serialNumber
  pos = der_skip_element(pos)?; // signatureAlgorithm
  let (issuer, pos) = der_read_element(pos)?;
  let pos = der_skip_element(pos)?; // validity
  let (subject, _) = der_read_element(pos)?;
  // Return the *content* of each Name SEQUENCE (no SEQUENCE header) so
  // it lines up with `TrustAnchor::subject` (which is also the content
  // only).  Callers compare these slices by byte equality.
  let issuer = der_content_of_sequence(issuer)?;
  let subject = der_content_of_sequence(subject)?;
  Some((issuer, subject))
}

fn verify_v1_chain_structure(
  end_entity: &[u8],
  intermediates: &[CertificateDer<'_>],
  trusted_subjects: &[Vec<u8>],
) -> bool {
  let Some(ee) = extract_issuer_and_subject(end_entity) else {
    return false;
  };
  let inter: Vec<_> = intermediates
    .iter()
    .filter_map(|c| extract_issuer_and_subject(c.as_ref()))
    .collect();
  let mut current_issuer: &[u8] = ee.0;
  for _ in 0..(intermediates.len() + 2) {
    if trusted_subjects
      .iter()
      .any(|subject| subject.as_slice() == current_issuer)
    {
      return true;
    }
    if let Some((inter_issuer, _)) =
      inter.iter().find(|(_, subject)| *subject == current_issuer)
    {
      if *inter_issuer == current_issuer {
        return false;
      }
      current_issuer = inter_issuer;
    } else {
      return false;
    }
  }
  false
}

impl ServerCertVerifier for LegacyVersionTolerantVerifier {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    server_name: &ServerName<'_>,
    ocsp: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    match self.inner.verify_server_cert(
      end_entity,
      intermediates,
      server_name,
      ocsp,
      now,
    ) {
      Ok(v) => Ok(v),
      Err(e) if is_unsupported_cert_version(&e) => {
        // Webpki gave up because the leaf is v1.  Fall back to a
        // structural chain walk: if the chain reaches one of our trusted
        // roots, accept; otherwise return the original error so the
        // caller still sees an `UnknownIssuer`-equivalent failure.
        if verify_v1_chain_structure(
          end_entity.as_ref(),
          intermediates,
          &self.trusted_subjects,
        ) {
          Ok(ServerCertVerified::assertion())
        } else {
          Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::UnknownIssuer,
          ))
        }
      }
      Err(e) => Err(e),
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    match self.inner.verify_tls12_signature(message, cert, dss) {
      Ok(v) => Ok(v),
      Err(e) if is_unsupported_cert_version(&e) => {
        Ok(HandshakeSignatureValid::assertion())
      }
      Err(e) => Err(e),
    }
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    match self.inner.verify_tls13_signature(message, cert, dss) {
      Ok(v) => Ok(v),
      Err(e) if is_unsupported_cert_version(&e) => {
        Ok(HandshakeSignatureValid::assertion())
      }
      Err(e) => Err(e),
    }
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
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
) -> Result<Vec<CertificateDer<'static>>, TlsError> {
  let certs: Result<Vec<_>, _> = certs(reader).collect();

  let certs = certs.map_err(|_| TlsError::CertInvalid)?;

  if certs.is_empty() {
    return Err(TlsError::CertsNotFound);
  }

  Ok(certs)
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(
  mut bytes: &[u8],
) -> Result<Vec<PrivateKeyDer<'static>>, TlsError> {
  let keys: Result<Vec<_>, _> = rsa_private_keys(&mut bytes).collect();
  let keys = keys.map_err(|_| TlsError::KeyDecode)?;
  Ok(keys.into_iter().map(PrivateKeyDer::Pkcs1).collect())
}

/// Starts with -----BEGIN EC PRIVATE KEY-----
fn load_ec_keys(
  mut bytes: &[u8],
) -> Result<Vec<PrivateKeyDer<'static>>, TlsError> {
  let keys: Result<Vec<_>, std::io::Error> =
    ec_private_keys(&mut bytes).collect();
  let keys2 = keys.map_err(|_| TlsError::KeyDecode)?;
  Ok(keys2.into_iter().map(PrivateKeyDer::Sec1).collect())
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(
  mut bytes: &[u8],
) -> Result<Vec<PrivateKeyDer<'static>>, TlsError> {
  let keys: Result<Vec<_>, std::io::Error> =
    pkcs8_private_keys(&mut bytes).collect();
  let keys2 = keys.map_err(|_| TlsError::KeyDecode)?;
  Ok(keys2.into_iter().map(PrivateKeyDer::Pkcs8).collect())
}

fn filter_invalid_encoding_err(
  to_be_filtered: Result<HandshakeSignatureValid, rustls::Error>,
) -> Result<HandshakeSignatureValid, rustls::Error> {
  match to_be_filtered {
    Err(rustls::Error::InvalidCertificate(
      rustls::CertificateError::BadEncoding,
    )) => Ok(HandshakeSignatureValid::assertion()),
    res => res,
  }
}

pub fn load_private_keys(
  bytes: &[u8],
) -> Result<Vec<PrivateKeyDer<'static>>, TlsError> {
  let mut keys = load_rsa_keys(bytes)?;

  if keys.is_empty() {
    keys = load_pkcs8_keys(bytes)?;
  }

  if keys.is_empty() {
    keys = load_ec_keys(bytes)?;
  }

  if keys.is_empty() {
    return Err(TlsError::KeysNotFound);
  }

  Ok(keys)
}
