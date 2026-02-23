// Copyright 2018-2026 the Deno authors. MIT license.

use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::ops::Deref;

use base64::Engine;
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

use crate::keys::KeyObjectHandle;

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

pub struct Certificate {
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

    let subjectaltname = get_subject_alt_name(cert).unwrap_or_default();

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

fn format_ip_address(ip_bytes: &[u8]) -> Option<String> {
  match ip_bytes.len() {
    4 => {
      let addr =
        Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
      Some(addr.to_string())
    }
    16 => {
      let mut segments = [0u16; 8];
      for i in 0..8 {
        segments[i] =
          u16::from_be_bytes([ip_bytes[i * 2], ip_bytes[i * 2 + 1]]);
      }
      let addr = Ipv6Addr::from(segments);
      Some(addr.to_string())
    }
    _ => None,
  }
}

fn get_subject_alt_name(cert: &X509Certificate) -> Option<String> {
  let subject_alt = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::SubjectAlternativeName(s) => Some(s),
      _ => None,
    })?;

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
        if let Some(formatted) = format_ip_address(ip) {
          alt_names.push(format!("IP Address:{}", formatted));
        } else {
          alt_names
            .push(format!("IP Address:{}", data_encoding::HEXUPPER.encode(ip)));
        }
      }
      extensions::GeneralName::URI(uri) => {
        alt_names.push(format!("URI:{}", uri));
      }
      extensions::GeneralName::DirectoryName(dn) => {
        if let Ok(s) = x509name_to_string(dn, oid_registry()) {
          alt_names.push(format!("DirName:{}", s));
        }
      }
      _ => {}
    }
  }

  if alt_names.is_empty() {
    None
  } else {
    Some(alt_names.join(", "))
  }
}

#[op2]
#[string]
pub fn op_node_x509_to_string(#[cppgc] cert: &Certificate) -> String {
  let der_bytes = match cert.inner.backing_cart().as_ref() {
    CertificateSources::Pem(pem) => &pem.contents,
    CertificateSources::Der(der) => der.as_ref(),
  };

  let b64 = base64::engine::general_purpose::STANDARD.encode(der_bytes);
  let mut pem_str = String::from("-----BEGIN CERTIFICATE-----\n");
  for chunk in b64.as_bytes().chunks(64) {
    pem_str.push_str(std::str::from_utf8(chunk).unwrap());
    pem_str.push('\n');
  }
  pem_str.push_str("-----END CERTIFICATE-----\n");
  pem_str
}

#[op2]
#[buffer]
pub fn op_node_x509_get_raw(#[cppgc] cert: &Certificate) -> Box<[u8]> {
  match cert.inner.backing_cart().as_ref() {
    CertificateSources::Pem(pem) => pem.contents.clone().into_boxed_slice(),
    CertificateSources::Der(der) => der.clone(),
  }
}

#[op2]
#[string]
pub fn op_node_x509_get_subject_alt_name(
  #[cppgc] cert: &Certificate,
) -> Option<String> {
  let cert = cert.inner.get().deref();
  get_subject_alt_name(cert)
}

#[op2]
#[string]
pub fn op_node_x509_check_ip(
  #[cppgc] cert: &Certificate,
  #[string] ip: &str,
) -> Option<String> {
  let target_ip: IpAddr = ip.parse().ok()?;

  let cert = cert.inner.get().deref();
  let subject_alt = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::SubjectAlternativeName(s) => Some(s),
      _ => None,
    })?;

  for name in &subject_alt.general_names {
    if let extensions::GeneralName::IPAddress(ip_bytes) = name {
      let san_ip = match ip_bytes.len() {
        4 => IpAddr::V4(Ipv4Addr::new(
          ip_bytes[0],
          ip_bytes[1],
          ip_bytes[2],
          ip_bytes[3],
        )),
        16 => {
          let mut segments = [0u16; 8];
          for i in 0..8 {
            segments[i] =
              u16::from_be_bytes([ip_bytes[i * 2], ip_bytes[i * 2 + 1]]);
          }
          IpAddr::V6(Ipv6Addr::from(segments))
        }
        _ => continue,
      };
      if san_ip == target_ip {
        return Some(ip.to_string());
      }
    }
  }

  None
}

#[op2(fast)]
pub fn op_node_x509_check_issued(
  #[cppgc] cert: &Certificate,
  #[cppgc] other: &Certificate,
) -> bool {
  let cert = cert.inner.get().deref();
  let other = other.inner.get().deref();

  // 1. Check if other's subject matches cert's issuer (name comparison)
  if cert.issuer().as_raw() != other.subject().as_raw() {
    return false;
  }

  // 2. If cert has an Authority Key Identifier extension with a key_identifier,
  //    it must match the issuer's Subject Key Identifier.
  let cert_aki = cert
    .extensions()
    .iter()
    .find(|e| {
      e.oid == x509_parser::oid_registry::OID_X509_EXT_AUTHORITY_KEY_IDENTIFIER
    })
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::AuthorityKeyIdentifier(aki) => Some(aki),
      _ => None,
    });

  if let Some(aki) = cert_aki {
    if let Some(aki_key_id) = &aki.key_identifier {
      let other_ski = other
        .extensions()
        .iter()
        .find(|e| {
          e.oid
            == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_KEY_IDENTIFIER
        })
        .and_then(|e| match e.parsed_extension() {
          extensions::ParsedExtension::SubjectKeyIdentifier(ski) => Some(ski),
          _ => None,
        });

      match other_ski {
        Some(ski) => {
          if aki_key_id.0 != ski.0 {
            return false;
          }
        }
        None => return false,
      }
    }
  }

  // 3. If issuer has KeyUsage extension, keyCertSign bit must be set.
  let other_key_usage = other
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_KEY_USAGE)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::KeyUsage(k) => Some(k),
      _ => None,
    });

  if let Some(key_usage) = other_key_usage {
    if !key_usage.key_cert_sign() {
      return false;
    }
  }

  true
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X509CheckPrivateKeyError {
  #[class(generic)]
  #[error(transparent)]
  X509(#[from] X509Error),
  #[class(generic)]
  #[error("Failed to export public key")]
  ExportFailed,
}

#[op2(fast)]
pub fn op_node_x509_check_private_key(
  #[cppgc] cert: &Certificate,
  #[cppgc] key: &KeyObjectHandle,
) -> Result<bool, X509CheckPrivateKeyError> {
  let private_key = match key.as_private_key() {
    Some(k) => k,
    None => return Ok(false),
  };

  let derived_public_key = private_key.to_public_key();
  let derived_spki_der = derived_public_key
    .export_der("spki")
    .map_err(|_| X509CheckPrivateKeyError::ExportFailed)?;

  let cert = cert.inner.get().deref();
  let cert_spki_raw = cert.tbs_certificate.subject_pki.raw;

  // Both `subject_pki.raw` and `export_der("spki")` produce the full
  // SubjectPublicKeyInfo DER SEQUENCE (tag + length + contents). DER
  // encoding is canonical, so a byte comparison is sufficient.
  Ok(cert_spki_raw == derived_spki_der.as_ref())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X509VerifyError {
  #[class(generic)]
  #[error(transparent)]
  X509(#[from] X509Error),
  #[class(generic)]
  #[error("Failed to export public key")]
  ExportFailed,
  #[class(generic)]
  #[error("Failed to parse public key")]
  ParseFailed,
  #[class(generic)]
  #[error("Unsupported EC curve for X509 verification")]
  UnsupportedEcCurve,
}

/// Verify an RSA-PSS signature. Parses the hash algorithm from the
/// RSASSA-PSS-params in the certificate's signature algorithm.
fn verify_rsa_pss(
  rsa_key: &rsa::RsaPublicKey,
  sig_alg: &x509_parser::x509::AlgorithmIdentifier,
  tbs_raw: &[u8],
  sig_value: &[u8],
) -> Result<bool, X509VerifyError> {
  use rsa::signature::Verifier;

  // Parse the hash algorithm OID from the RSA-PSS parameters.
  // RSASSA-PSS-params ::= SEQUENCE {
  //   hashAlgorithm [0] AlgorithmIdentifier DEFAULT sha1,
  //   ...
  // }
  // Default hash algorithm is SHA-1 if parameters are absent.
  let hash_oid = sig_alg.parameters.as_ref().and_then(|params| {
    let (_, seq) =
      x509_parser::der_parser::asn1_rs::Sequence::from_der(params.as_bytes())
        .ok()?;
    let mut remaining = seq.content.as_ref();
    while !remaining.is_empty() {
      let (rest, any) =
        x509_parser::der_parser::asn1_rs::Any::from_der(remaining).ok()?;
      remaining = rest;
      // [0] EXPLICIT tag for hashAlgorithm
      if any.tag().0 == 0 {
        // The content is an AlgorithmIdentifier SEQUENCE containing the OID
        let (_, inner_seq) =
          x509_parser::der_parser::asn1_rs::Sequence::from_der(any.data)
            .ok()?;
        let (_, oid) = Oid::from_der(inner_seq.content.as_ref()).ok()?;
        return Some(oid.to_id_string());
      }
    }
    None
  });

  let hash_alg = hash_oid.as_deref().unwrap_or("1.3.14.3.2.26"); // SHA-1 default

  let sig = rsa::pss::Signature::try_from(sig_value)
    .map_err(|_| X509VerifyError::ParseFailed)?;

  let result = match hash_alg {
    // id-sha1
    "1.3.14.3.2.26" => {
      let verifier = rsa::pss::VerifyingKey::<sha1::Sha1>::new(rsa_key.clone());
      verifier.verify(tbs_raw, &sig).is_ok()
    }
    // id-sha256
    "2.16.840.1.101.3.4.2.1" => {
      let verifier =
        rsa::pss::VerifyingKey::<sha2::Sha256>::new(rsa_key.clone());
      verifier.verify(tbs_raw, &sig).is_ok()
    }
    // id-sha384
    "2.16.840.1.101.3.4.2.2" => {
      let verifier =
        rsa::pss::VerifyingKey::<sha2::Sha384>::new(rsa_key.clone());
      verifier.verify(tbs_raw, &sig).is_ok()
    }
    // id-sha512
    "2.16.840.1.101.3.4.2.3" => {
      let verifier =
        rsa::pss::VerifyingKey::<sha2::Sha512>::new(rsa_key.clone());
      verifier.verify(tbs_raw, &sig).is_ok()
    }
    _ => false,
  };

  Ok(result)
}

#[op2(fast)]
pub fn op_node_x509_verify(
  #[cppgc] cert: &Certificate,
  #[cppgc] key: &KeyObjectHandle,
) -> Result<bool, X509VerifyError> {
  use crate::keys::AsymmetricPublicKey;

  let public_key = match key.as_public_key() {
    Some(k) => k,
    None => return Ok(false),
  };

  let cert_inner = cert.inner.get().deref();

  // Get the raw TBS (to-be-signed) certificate bytes and signature.
  // `as_ref()` returns the raw DER bytes of the TBSCertificate including
  // the SEQUENCE header (tag + length), which is what the signature covers.
  // See: https://github.com/rusticata/x509-parser/blob/b7dcc9397b596cf9fa3df65115c3f405f1748b2a/src/certificate.rs#L770-L773
  let tbs_raw = cert_inner.tbs_certificate.as_ref();
  let sig_value = cert_inner.signature_value.as_ref();
  let sig_alg_oid = cert_inner.signature_algorithm.algorithm.to_id_string();

  // Verify based on key type and signature algorithm
  match &*public_key {
    AsymmetricPublicKey::Rsa(rsa_key) => {
      use rsa::signature::Verifier;

      let result = match sig_alg_oid.as_str() {
        // sha1WithRSAEncryption
        "1.2.840.113549.1.1.5" => {
          let verifier =
            rsa::pkcs1v15::VerifyingKey::<sha1::Sha1>::new(rsa_key.clone());
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha256WithRSAEncryption
        "1.2.840.113549.1.1.11" => {
          let verifier =
            rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(rsa_key.clone());
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha384WithRSAEncryption
        "1.2.840.113549.1.1.12" => {
          let verifier =
            rsa::pkcs1v15::VerifyingKey::<sha2::Sha384>::new(rsa_key.clone());
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha512WithRSAEncryption
        "1.2.840.113549.1.1.13" => {
          let verifier =
            rsa::pkcs1v15::VerifyingKey::<sha2::Sha512>::new(rsa_key.clone());
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // id-RSASSA-PSS
        "1.2.840.113549.1.1.10" => verify_rsa_pss(
          rsa_key,
          &cert_inner.signature_algorithm,
          tbs_raw,
          sig_value,
        )?,
        _ => false,
      };
      Ok(result)
    }
    AsymmetricPublicKey::RsaPss(rsa_pss_key) => {
      use rsa::signature::Verifier;

      let result = match sig_alg_oid.as_str() {
        // sha1WithRSAEncryption
        "1.2.840.113549.1.1.5" => {
          let verifier = rsa::pkcs1v15::VerifyingKey::<sha1::Sha1>::new(
            rsa_pss_key.key.clone(),
          );
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha256WithRSAEncryption
        "1.2.840.113549.1.1.11" => {
          let verifier = rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(
            rsa_pss_key.key.clone(),
          );
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha384WithRSAEncryption
        "1.2.840.113549.1.1.12" => {
          let verifier = rsa::pkcs1v15::VerifyingKey::<sha2::Sha384>::new(
            rsa_pss_key.key.clone(),
          );
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // sha512WithRSAEncryption
        "1.2.840.113549.1.1.13" => {
          let verifier = rsa::pkcs1v15::VerifyingKey::<sha2::Sha512>::new(
            rsa_pss_key.key.clone(),
          );
          let sig = rsa::pkcs1v15::Signature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          verifier.verify(tbs_raw, &sig).is_ok()
        }
        // id-RSASSA-PSS
        "1.2.840.113549.1.1.10" => verify_rsa_pss(
          &rsa_pss_key.key,
          &cert_inner.signature_algorithm,
          tbs_raw,
          sig_value,
        )?,
        _ => false,
      };
      Ok(result)
    }
    AsymmetricPublicKey::Ec(ec_key) => {
      use crate::keys::EcPublicKey;

      match ec_key {
        EcPublicKey::P256(key) => {
          use p256::ecdsa::signature::Verifier;
          let verifying_key = p256::ecdsa::VerifyingKey::from(key);
          let sig = p256::ecdsa::DerSignature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          Ok(verifying_key.verify(tbs_raw, &sig).is_ok())
        }
        EcPublicKey::P384(key) => {
          use p384::ecdsa::signature::Verifier;
          let verifying_key = p384::ecdsa::VerifyingKey::from(key);
          let sig = p384::ecdsa::DerSignature::try_from(sig_value)
            .map_err(|_| X509VerifyError::ParseFailed)?;
          Ok(verifying_key.verify(tbs_raw, &sig).is_ok())
        }
        _ => Err(X509VerifyError::UnsupportedEcCurve),
      }
    }
    AsymmetricPublicKey::Ed25519(key) => {
      let verified = aws_lc_rs::signature::UnparsedPublicKey::new(
        &aws_lc_rs::signature::ED25519,
        key.as_bytes().as_slice(),
      )
      .verify(tbs_raw, sig_value)
      .is_ok();
      Ok(verified)
    }
    _ => Ok(false),
  }
}

#[op2]
#[string]
pub fn op_node_x509_get_info_access(
  #[cppgc] cert: &Certificate,
) -> Option<String> {
  let cert = cert.inner.get().deref();

  // OID for Authority Information Access
  let oid_aia = Oid::from(&[1, 3, 6, 1, 5, 5, 7, 1, 1]).ok()?;
  let oid_ocsp = Oid::from(&[1, 3, 6, 1, 5, 5, 7, 48, 1]).ok()?;
  let oid_ca_issuers = Oid::from(&[1, 3, 6, 1, 5, 5, 7, 48, 2]).ok()?;

  let ext = cert.extensions().iter().find(|e| e.oid == oid_aia)?;

  // Parse the AIA extension value manually
  // AIA is a SEQUENCE of AccessDescription
  // Each AccessDescription is SEQUENCE { accessMethod OID, accessLocation GeneralName }
  let data = ext.value;
  let (_, seq) =
    x509_parser::der_parser::asn1_rs::Sequence::from_der(data).ok()?;

  let mut entries = Vec::new();
  let mut remaining = seq.content.as_ref();

  while !remaining.is_empty() {
    let (rest, access_desc) =
      x509_parser::der_parser::asn1_rs::Sequence::from_der(remaining).ok()?;
    remaining = rest;

    let (general_name_data, method_oid) =
      Oid::from_der(access_desc.content.as_ref()).ok()?;

    let method_name = if method_oid == oid_ocsp {
      "OCSP - URI"
    } else if method_oid == oid_ca_issuers {
      "CA Issuers - URI"
    } else {
      continue;
    };

    // GeneralName is context-tagged. Tag [6] = uniformResourceIdentifier (IA5String)
    if !general_name_data.is_empty() {
      let (_, any) =
        x509_parser::der_parser::asn1_rs::Any::from_der(general_name_data)
          .ok()?;
      // Tag 6 is context-specific for URI in GeneralName
      if any.tag().0 == 6
        && let Ok(uri) = std::str::from_utf8(any.data)
      {
        entries.push(format!("{}:{}", method_name, uri));
      }
    }
  }

  if entries.is_empty() {
    None
  } else {
    Some(entries.join("\n"))
  }
}

#[op2]
#[serde]
pub fn op_node_x509_to_legacy_object(
  #[cppgc] cert: &Certificate,
) -> Result<CertificateObject, JsX509Error> {
  cert.to_object(true).map_err(Into::into)
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
