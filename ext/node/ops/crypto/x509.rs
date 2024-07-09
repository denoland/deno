// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;

use x509_parser::der_parser::asn1_rs::Any;
use x509_parser::der_parser::asn1_rs::Tag;
use x509_parser::der_parser::oid::Oid;
use x509_parser::extensions;
use x509_parser::pem;
use x509_parser::prelude::*;

use digest::Digest;

pub(crate) struct Certificate {
  _buf: Vec<u8>,
  pem: Option<pem::Pem>,
  cert: X509Certificate<'static>,
}

impl deno_core::GarbageCollected for Certificate {}

impl Certificate {
  fn fingerprint<D: Digest>(&self) -> Option<String> {
    self.pem.as_ref().map(|pem| {
      let mut hasher = D::new();
      hasher.update(&pem.contents);
      let bytes = hasher.finalize();
      // OpenSSL returns colon separated upper case hex values.
      let mut hex = String::with_capacity(bytes.len() * 2);
      for byte in bytes {
        hex.push_str(&format!("{:02X}:", byte));
      }
      hex.pop();
      hex
    })
  }
}

impl std::ops::Deref for Certificate {
  type Target = X509Certificate<'static>;

  fn deref(&self) -> &Self::Target {
    &self.cert
  }
}

#[op2]
pub fn op_node_x509_parse<'s>(
  scope: &'s mut v8::HandleScope,
  #[buffer] buf: &[u8],
) -> Result<v8::Local<'s, v8::Object>, AnyError> {
  let pem = match pem::parse_x509_pem(buf) {
    Ok((_, pem)) => Some(pem),
    Err(_) => None,
  };

  let cert = pem
    .as_ref()
    .map(|pem| pem.parse_x509())
    .unwrap_or_else(|| X509Certificate::from_der(buf).map(|(_, cert)| cert))?;

  let cert = Certificate {
    _buf: buf.to_vec(),
    // SAFETY: Extending the lifetime of the certificate. Backing buffer is
    // owned by the resource.
    cert: unsafe {
      std::mem::transmute::<X509Certificate<'_>, X509Certificate<'_>>(cert)
    },
    pem,
  };

  let obj = deno_core::cppgc::make_cppgc_object(scope, cert);
  Ok(obj)
}

#[op2(fast)]
pub fn op_node_x509_ca(#[cppgc] cert: &Certificate) -> Result<bool, AnyError> {
  Ok(cert.is_ca())
}

#[op2(fast)]
pub fn op_node_x509_check_email(
  #[cppgc] cert: &Certificate,
  #[string] email: &str,
) -> Result<bool, AnyError> {
  let subject = cert.subject();
  if subject
    .iter_email()
    .any(|e| e.as_str().unwrap_or("") == email)
  {
    return Ok(true);
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
      if let extensions::GeneralName::RFC822Name(n) = name {
        if *n == email {
          return Ok(true);
        }
      }
    }
  }

  Ok(false)
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint(
  #[cppgc] cert: &Certificate,
) -> Result<Option<String>, AnyError> {
  Ok(cert.fingerprint::<sha1::Sha1>())
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint256(
  #[cppgc] cert: &Certificate,
) -> Result<Option<String>, AnyError> {
  Ok(cert.fingerprint::<sha2::Sha256>())
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint512(
  #[cppgc] cert: &Certificate,
) -> Result<Option<String>, AnyError> {
  Ok(cert.fingerprint::<sha2::Sha512>())
}

#[op2]
#[string]
pub fn op_node_x509_get_issuer(
  #[cppgc] cert: &Certificate,
) -> Result<String, AnyError> {
  Ok(x509name_to_string(cert.issuer(), oid_registry())?)
}

#[op2]
#[string]
pub fn op_node_x509_get_subject(
  #[cppgc] cert: &Certificate,
) -> Result<String, AnyError> {
  Ok(x509name_to_string(cert.subject(), oid_registry())?)
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
pub fn op_node_x509_get_valid_from(
  #[cppgc] cert: &Certificate,
) -> Result<String, AnyError> {
  Ok(cert.validity().not_before.to_string())
}

#[op2]
#[string]
pub fn op_node_x509_get_valid_to(
  #[cppgc] cert: &Certificate,
) -> Result<String, AnyError> {
  Ok(cert.validity().not_after.to_string())
}

#[op2]
#[string]
pub fn op_node_x509_get_serial_number(
  #[cppgc] cert: &Certificate,
) -> Result<String, AnyError> {
  let mut s = cert.serial.to_str_radix(16);
  s.make_ascii_uppercase();
  Ok(s)
}

#[op2(fast)]
pub fn op_node_x509_key_usage(
  #[cppgc] cert: &Certificate,
) -> Result<u16, AnyError> {
  let key_usage = cert
    .extensions()
    .iter()
    .find(|e| e.oid == x509_parser::oid_registry::OID_X509_EXT_KEY_USAGE)
    .and_then(|e| match e.parsed_extension() {
      extensions::ParsedExtension::KeyUsage(k) => Some(k),
      _ => None,
    });

  Ok(key_usage.map(|k| k.flags).unwrap_or(0))
}
