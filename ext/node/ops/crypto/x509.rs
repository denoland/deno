// Copyright 2018-2025 the Deno authors. MIT license.

use std::ops::Deref;

use deno_core::ToJsBuffer;
use deno_core::op2;
use digest::Digest;
use x509_parser::der_parser::asn1_rs::Any;
use x509_parser::der_parser::asn1_rs::Tag;
use x509_parser::der_parser::oid::Oid;
pub use x509_parser::error::X509Error;
use x509_parser::extensions;
use x509_parser::pem;
use x509_parser::prelude::*;
use yoke::Yoke;
use yoke::Yokeable;

use super::KeyObjectHandle;

enum CertificateSources {
  Der(Box<[u8]>),
  Pem(pem::Pem),
}

#[derive(serde::Serialize, Default)]
#[serde(rename_all = "UPPERCASE")]
struct SubjectOrIssuer {
  #[serde(skip_serializing_if = "Option::is_none")]
  c: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  st: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  l: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  o: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  ou: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  cn: Option<String>,
}

#[derive(serde::Serialize)]
pub struct CertificateObject {
  ca: bool,
  raw: ToJsBuffer,
  subject: SubjectOrIssuer,
  issuer: SubjectOrIssuer,
  valid_from: String,
  valid_to: String,
  #[serde(rename = "serialNumber")]
  serial_number: String,
  fingerprint: String,
  fingerprint256: String,
  fingerprint512: String,
  subjectaltname: String,
  // RSA key fields
  #[serde(skip_serializing_if = "Option::is_none")]
  bits: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  exponent: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  modulus: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pubkey: Option<ToJsBuffer>,
  // EC key fields
  #[serde(skip_serializing_if = "Option::is_none")]
  #[serde(rename = "asn1Curve")]
  asn1_curve: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[serde(rename = "nistCurve")]
  nist_curve: Option<String>,
}

#[derive(Yokeable)]
struct CertificateView<'a> {
  cert: X509Certificate<'a>,
}

pub(crate) struct Certificate {
  inner: Yoke<CertificateView<'static>, Box<CertificateSources>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Certificate {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Certificate"
  }
}

impl Certificate {
  pub fn from_der(der: &[u8]) -> Result<Certificate, X509Error> {
    let source = CertificateSources::Der(der.to_vec().into_boxed_slice());

    let inner =
      Yoke::<CertificateView<'static>, Box<CertificateSources>>::try_attach_to_cart(
        Box::new(source),
        |source| {
          let cert = match source {
            CertificateSources::Der(buf) => {
              X509Certificate::from_der(buf).map(|(_, cert)| cert)?
            }
            _ => unreachable!(),
          };
          Ok::<_, X509Error>(CertificateView { cert })
        },
      )?;

    Ok(Certificate { inner })
  }

  fn fingerprint<D: Digest>(&self) -> Option<String> {
    let data = match self.inner.backing_cart().as_ref() {
      CertificateSources::Pem(pem) => &pem.contents,
      CertificateSources::Der(der) => der.as_ref(),
    };

    let mut hasher = D::new();
    hasher.update(data);
    let bytes = hasher.finalize();
    // OpenSSL returns colon separated upper case hex values.
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
      hex.push_str(&format!("{:02X}:", byte));
    }
    hex.pop();
    Some(hex)
  }

  pub fn to_object(
    &self,
    _detailed: bool,
  ) -> Result<CertificateObject, X509Error> {
    let cert = self.inner.get().deref();

    let raw = match self.inner.backing_cart().as_ref() {
      CertificateSources::Pem(pem) => pem.contents.clone(),
      CertificateSources::Der(der) => der.to_vec(),
    };

    let valid_from = cert.validity().not_before.to_string();
    let valid_to = cert.validity().not_after.to_string();

    let mut serial_number = cert.serial.to_str_radix(16);
    serial_number.make_ascii_uppercase();

    let fingerprint = self.fingerprint::<sha1::Sha1>().unwrap_or_default();
    let fingerprint256 = self.fingerprint::<sha2::Sha256>().unwrap_or_default();
    let fingerprint512 = self.fingerprint::<sha2::Sha512>().unwrap_or_default();

    let mut subjectaltname = String::new();
    if let Some(subject_alt) = cert
      .extensions()
      .iter()
      .find(|e| {
        e.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME
      })
      .and_then(|e| match e.parsed_extension() {
        extensions::ParsedExtension::SubjectAlternativeName(s) => Some(s),
        _ => None,
      })
    {
      let mut alt_names = Vec::new();
      for name in &subject_alt.general_names {
        match name {
          extensions::GeneralName::DNSName(dns) => {
            alt_names.push(format!("DNS:{}", dns));
          }
          extensions::GeneralName::RFC822Name(email) => {
            alt_names.push(format!("email:{}", email));
          }
          extensions::GeneralName::IPAddress(ip) => {
            alt_names.push(format!(
              "IP Address:{}",
              data_encoding::HEXUPPER.encode(ip)
            ));
          }
          _ => {}
        }
      }
      subjectaltname = alt_names.join(", ");
    }

    let subject = extract_subject_or_issuer(cert.subject());
    let issuer = extract_subject_or_issuer(cert.issuer());

    let KeyInfo {
      bits,
      exponent,
      modulus,
      pubkey,
      asn1_curve,
      nist_curve,
    } = extract_key_info(&cert.tbs_certificate.subject_pki);

    Ok(CertificateObject {
      ca: cert.is_ca(),
      raw: raw.into(),
      subject,
      issuer,
      valid_from,
      valid_to,
      serial_number,
      fingerprint,
      fingerprint256,
      fingerprint512,
      subjectaltname,
      bits,
      exponent,
      modulus,
      pubkey: pubkey.map(|p| p.into()),
      asn1_curve,
      nist_curve,
    })
  }
}

impl<'a> Deref for CertificateView<'a> {
  type Target = X509Certificate<'a>;

  fn deref(&self) -> &Self::Target {
    &self.cert
  }
}

deno_error::js_error_wrapper!(X509Error, JsX509Error, "Error");

#[op2]
#[cppgc]
pub fn op_node_x509_parse(
  #[buffer] buf: &[u8],
) -> Result<Certificate, JsX509Error> {
  let source = match pem::parse_x509_pem(buf) {
    Ok((_, pem)) => CertificateSources::Pem(pem),
    Err(_) => CertificateSources::Der(buf.to_vec().into_boxed_slice()),
  };

  let inner =
    Yoke::<CertificateView<'static>, Box<CertificateSources>>::try_attach_to_cart(
      Box::new(source),
      |source| {
        let cert = match source {
          CertificateSources::Pem(pem) => pem.parse_x509()?,
          CertificateSources::Der(buf) => {
            X509Certificate::from_der(buf).map(|(_, cert)| cert)?
          }
        };
        Ok::<_, X509Error>(CertificateView { cert })
      },
    )?;

  Ok(Certificate { inner })
}

#[op2(fast)]
pub fn op_node_x509_ca(#[cppgc] cert: &Certificate) -> bool {
  let cert = cert.inner.get().deref();
  cert.is_ca()
}

#[op2(fast)]
pub fn op_node_x509_check_email(
  #[cppgc] cert: &Certificate,
  #[string] email: &str,
) -> bool {
  let cert = cert.inner.get().deref();
  let subject = cert.subject();
  if subject
    .iter_email()
    .any(|e| e.as_str().unwrap_or("") == email)
  {
    return true;
  }

  let subject_alt = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::SubjectAlternativeName(s) => Some(s),
      _ => None,
    });

  if let Some(subject_alt) = subject_alt {
    for name in &subject_alt.general_names {
      if let extensions::GeneralName::RFC822Name(n) = name
        && *n == email
      {
        return true;
      }
    }
  }

  false
}

#[op2(fast)]
pub fn op_node_x509_check_host(
  #[cppgc] cert: &Certificate,
  #[string] host: &str,
) -> bool {
  let cert = cert.inner.get().deref();

  let subject = cert.subject();
  if subject
    .iter_common_name()
    .any(|e| e.as_str().unwrap_or("") == host)
  {
    return true;
  }

  let subject_alt = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::SubjectAlternativeName(s) => Some(s),
      _ => None,
    });

  if let Some(subject_alt) = subject_alt {
    for name in &subject_alt.general_names {
      if let extensions::GeneralName::DNSName(n) = name
        && *n == host
      {
        return true;
      }
    }
  }

  false
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint(#[cppgc] cert: &Certificate) -> Option<String> {
  cert.fingerprint::<sha1::Sha1>()
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint256(
  #[cppgc] cert: &Certificate,
) -> Option<String> {
  cert.fingerprint::<sha2::Sha256>()
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint512(
  #[cppgc] cert: &Certificate,
) -> Option<String> {
  cert.fingerprint::<sha2::Sha512>()
}

#[op2]
#[string]
pub fn op_node_x509_get_issuer(
  #[cppgc] cert: &Certificate,
) -> Result<String, JsX509Error> {
  let cert = cert.inner.get().deref();
  x509name_to_string(cert.issuer(), oid_registry()).map_err(Into::into)
}

#[op2]
#[string]
pub fn op_node_x509_get_subject(
  #[cppgc] cert: &Certificate,
) -> Result<String, JsX509Error> {
  let cert = cert.inner.get().deref();
  x509name_to_string(cert.subject(), oid_registry()).map_err(Into::into)
}

#[op2]
#[cppgc]
pub fn op_node_x509_public_key(
  #[cppgc] cert: &Certificate,
) -> Result<KeyObjectHandle, super::keys::X509PublicKeyError> {
  let cert = cert.inner.get().deref();
  let public_key = &cert.tbs_certificate.subject_pki;

  KeyObjectHandle::new_x509_public_key(public_key)
}

fn extract_subject_or_issuer(name: &X509Name) -> SubjectOrIssuer {
  let mut result = SubjectOrIssuer::default();

  for rdn in name.iter_rdn() {
    for attr in rdn.iter() {
      if let Ok(value_str) =
        attribute_value_to_string(attr.attr_value(), attr.attr_type())
      {
        match attr.attr_type() {
          oid if oid == &x509_parser::oid_registry::OID_X509_COUNTRY_NAME => {
            result.c = Some(value_str);
          }
          oid if oid == &x509_parser::oid_registry::OID_X509_STATE_OR_PROVINCE_NAME => {
            result.st = Some(value_str);
          }
          oid if oid == &x509_parser::oid_registry::OID_X509_LOCALITY_NAME => {
            result.l = Some(value_str);
          }
          oid if oid == &x509_parser::oid_registry::OID_X509_ORGANIZATION_NAME => {
            result.o = Some(value_str);
          }
          oid if oid == &x509_parser::oid_registry::OID_X509_ORGANIZATIONAL_UNIT => {
            result.ou = Some(value_str);
          }
          oid if oid == &x509_parser::oid_registry::OID_X509_COMMON_NAME => {
            result.cn = Some(value_str);
          }
          _ => {}
        }
      }
    }
  }

  result
}

// Attempt to convert attribute to string. If type is not a string, return value is the hex
// encoding of the attribute value
fn attribute_value_to_string(
  attr: &Any,
  _attr_type: &Oid,
) -> Result<String, X509Error> {
  // TODO: replace this with helper function, when it is added to asn1-rs
  match attr.tag() {
    Tag::NumericString
    | Tag::BmpString
    | Tag::VisibleString
    | Tag::PrintableString
    | Tag::GeneralString
    | Tag::ObjectDescriptor
    | Tag::GraphicString
    | Tag::T61String
    | Tag::VideotexString
    | Tag::Utf8String
    | Tag::Ia5String => {
      let s = core::str::from_utf8(attr.data)
        .map_err(|_| X509Error::InvalidAttributes)?;
      Ok(s.to_owned())
    }
    _ => {
      // type is not a string, get slice and convert it to base64
      Ok(data_encoding::HEXUPPER.encode(attr.as_bytes()))
    }
  }
}

fn x509name_to_string(
  name: &X509Name,
  oid_registry: &oid_registry::OidRegistry,
) -> Result<String, x509_parser::error::X509Error> {
  // Lifted from https://github.com/rusticata/x509-parser/blob/4d618c2ed6b1fc102df16797545895f7c67ee0fe/src/x509.rs#L543-L566
  // since it's a private function (Copyright 2017 Pierre Chifflier)
  name.iter_rdn().try_fold(String::new(), |acc, rdn| {
    rdn
      .iter()
      .try_fold(String::new(), |acc2, attr| {
        let val_str =
          attribute_value_to_string(attr.attr_value(), attr.attr_type())?;
        // look ABBREV, and if not found, use shortname
        let abbrev = match oid2abbrev(attr.attr_type(), oid_registry) {
          Ok(s) => String::from(s),
          _ => format!("{:?}", attr.attr_type()),
        };
        let rdn = format!("{}={}", abbrev, val_str);
        match acc2.len() {
          0 => Ok(rdn),
          _ => Ok(acc2 + " + " + rdn.as_str()),
        }
      })
      .map(|v| match acc.len() {
        0 => v,
        _ => acc + "\n" + v.as_str(),
      })
  })
}

#[op2]
#[string]
pub fn op_node_x509_get_valid_from(#[cppgc] cert: &Certificate) -> String {
  let cert = cert.inner.get().deref();
  cert.validity().not_before.to_string()
}

#[op2]
#[string]
pub fn op_node_x509_get_valid_to(#[cppgc] cert: &Certificate) -> String {
  let cert = cert.inner.get().deref();
  cert.validity().not_after.to_string()
}

#[op2]
#[string]
pub fn op_node_x509_get_serial_number(#[cppgc] cert: &Certificate) -> String {
  let cert = cert.inner.get().deref();
  let mut s = cert.serial.to_str_radix(16);
  s.make_ascii_uppercase();
  s
}

#[op2(fast)]
pub fn op_node_x509_key_usage(#[cppgc] cert: &Certificate) -> u16 {
  let cert = cert.inner.get().deref();
  let key_usage = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_KEY_USAGE)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::KeyUsage(k) => Some(k),
      _ => None,
    });

  key_usage.map(|k| k.flags).unwrap_or(0)
}

#[derive(Default)]
struct KeyInfo {
  bits: Option<u32>,
  exponent: Option<String>,
  modulus: Option<String>,
  pubkey: Option<Vec<u8>>,
  asn1_curve: Option<String>,
  nist_curve: Option<String>,
}

fn extract_key_info(spki: &x509_parser::x509::SubjectPublicKeyInfo) -> KeyInfo {
  use x509_parser::der_parser::asn1_rs::oid;
  use x509_parser::public_key::PublicKey;

  match spki.parsed() {
    Ok(PublicKey::RSA(key)) => {
      let modulus_bytes = key.modulus;
      let exponent_bytes = key.exponent;

      let bits = Some((modulus_bytes.len() * 8) as u32);
      let modulus = Some(data_encoding::HEXUPPER.encode(modulus_bytes));
      let exponent = Some(data_encoding::HEXUPPER.encode(exponent_bytes));
      let pubkey = Some(spki.raw.to_vec());

      KeyInfo {
        bits,
        exponent,
        modulus,
        pubkey,
        asn1_curve: None,
        nist_curve: None,
      }
    }
    Ok(PublicKey::EC(point)) => {
      let pubkey = Some(point.data().to_vec());
      let mut asn1_curve = None;
      let mut nist_curve = None;
      let mut bits = None;

      if let Some(params) = &spki.algorithm.parameters
        && let Ok(curve_oid) = params.as_oid()
      {
        const ID_SECP224R1: &[u8] = &oid!(raw 1.3.132.0.33);
        const ID_SECP256R1: &[u8] = &oid!(raw 1.2.840.10045.3.1.7);
        const ID_SECP384R1: &[u8] = &oid!(raw 1.3.132.0.34);

        match curve_oid.as_bytes() {
          ID_SECP224R1 => {
            asn1_curve = Some("1.3.132.0.33".to_string());
            nist_curve = Some("secp224r1".to_string());
            bits = Some(224);
          }
          ID_SECP256R1 => {
            asn1_curve = Some("1.2.840.10045.3.1.7".to_string());
            nist_curve = Some("secp256r1".to_string());
            bits = Some(256);
          }
          ID_SECP384R1 => {
            asn1_curve = Some("1.3.132.0.34".to_string());
            nist_curve = Some("secp384r1".to_string());
            bits = Some(384);
          }
          _ => {
            asn1_curve = Some(curve_oid.to_string());
          }
        }
      }

      KeyInfo {
        bits,
        exponent: None,
        modulus: None,
        pubkey,
        asn1_curve,
        nist_curve,
      }
    }
    _ => KeyInfo::default(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_subject_or_issuer() {
    let cert_pem = b"-----BEGIN CERTIFICATE-----
MIICljCCAX4CCQCKmSl7UdG4tjANBgkqhkiG9w0BAQsFADANMQswCQYDVQQDDAJD
TjAeFw0yNTAxMjIxNzQyNDFaFw0yNjAxMjIxNzQyNDFaMA0xCzAJBgNVBAMMAkNO
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA0K/qV+9PQH3Kg2g6tK6X
VxY7F8/2YKi8cKnX0YT5g9QnKjS1v8R9kKvR+LLx0Y1+pT8zFZr7BjU1cKxz8fmY
7P+vKH1R3O5p2qKvOxY4GlO6U3cQ1HtQ9TjIGiXn7T6v9BkKH6k8zL4m5W6Kp4s4
tR9J9n4rGY3j6TxC9h3W3d/dW9H6nF9r3oF9F5KvG0p8H0R7WXoO6h4J5m8J5k6b
5K3E7j9O9J1V9R8h4I5k8h0x7P2s1J8F1c5Z5T8l8e0K8N9J0x8Z8r9m0O0k6F0r
9B3G4e2j8d6F8r0t8I2W4K2v4g1g8N6f4j8c9w2r6m8O3J5I5h5E5i7n8d9v3QAU
lQIDAQABMA0GCSqGSIb3DQEBCwUAA4IBAQAWOKj9Z7ZuY8fz8N3bYh8G4kFh2J7R
B6QFzT4M6gF3jl6oJ5E3K0k5z7n9L9T5c4p8x5X8f2w8T2r4N8b4y2B8W6z4N5S8
y8M7R4H0t4R8y6S9c8o8r8g8Y8b8J8t6N8p4M3O4K8f8Z7w8P8T8G8N8q8b8H6H8
r6C3V5F4Z9y8o8i9E4j5V8O5Q7Y8Z4W8n7R8B8l8H8L4P4F8r8c8A4v3O4g8L8S6
8r8t3C6h8Y6k8b3F8w8z8H8g8k8m8B3R6K8C6P4R8f8M6g8Z2N8B8x8Z8F3A2N8R
8r8H8x2F8J2h8c8Y8x8H8g8n4l8x4E8r8p8j8S8m6F3k8L8S8z6A8F8k8B9U8L3R
-----END CERTIFICATE-----";

    let pem = pem::parse_x509_pem(cert_pem).unwrap().1;
    let cert = Certificate::from_der(&pem.contents).unwrap();
    let cert_inner = cert.inner.get().deref();

    let result = extract_subject_or_issuer(cert_inner.subject());

    assert_eq!(result.cn, Some("CN".to_string()));
    assert_eq!(result.c, None);
    assert_eq!(result.st, None);
    assert_eq!(result.l, None);
    assert_eq!(result.o, None);
    assert_eq!(result.ou, None);
  }
}
