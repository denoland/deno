// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rustls::internal::msgs::handshake::DigitallySignedStruct;
use rustls::Certificate;
use rustls::HandshakeSignatureValid;
use rustls::RootCertStore;
use rustls::ServerCertVerified;
use rustls::ServerCertVerifier;
use rustls::TLSError;
use webpki::DNSNameRef;

pub struct NoCertificateVerification {
  excluded: Vec<String>,
}

impl NoCertificateVerification {
  pub fn new(excluded: Vec<String>) -> Self {
    Self { excluded }
  }
}

impl ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    _roots: &RootCertStore,
    _presented_certs: &[Certificate],
    dns_name: DNSNameRef<'_>,
    _ocsp: &[u8],
  ) -> Result<ServerCertVerified, TLSError> {
    fn convert_to_string<T: AsRef<str>>(s: T) -> String {
      s.as_ref().to_owned()
    }
    let dns_name: String = convert_to_string(dns_name.to_owned());
    if self.excluded.is_empty() || self.excluded.contains(&dns_name) {
      Ok(ServerCertVerified::assertion())
    } else {
      Err(TLSError::General(dns_name))
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
