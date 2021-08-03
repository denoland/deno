// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rustls::internal::msgs::handshake::DigitallySignedStruct;
use rustls::Certificate;
use rustls::HandshakeSignatureValid;
use rustls::RootCertStore;
use rustls::ServerCertVerified;
use rustls::ServerCertVerifier;
use rustls::TLSError;
use webpki::DNSNameRef;

pub struct NoCertificateVerification(pub Vec<String>);

impl ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    _roots: &RootCertStore,
    _presented_certs: &[Certificate],
    dns_name: DNSNameRef<'_>,
    _ocsp: &[u8],
  ) -> Result<ServerCertVerified, TLSError> {
    let dns_name: &str = dns_name.into();
    let dns_name: String = dns_name.to_owned();
    if self.0.is_empty() || self.0.contains(&dns_name) {
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

pub fn combine_allow_insecure_certificates(
  lhs: Option<Vec<String>>,
  rhs: Option<Vec<String>>,
) -> Option<Vec<String>> {
  if lhs.is_some() || rhs.is_some() {
    let mut r = {
      let size = lhs.as_ref().map_or(0, |v| v.len())
        + rhs.as_ref().map_or(0, |v| v.len());
      Vec::<String>::with_capacity(size)
    };
    if let Some(gl) = lhs {
      r.extend(gl)
    }
    if let Some(al) = rhs {
      r.extend(al)
    }
    Some(r)
  } else {
    None
  }
}
