// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;

use std::borrow::Cow;

use x509_parser::der_parser::asn1_rs::Any;
use x509_parser::der_parser::asn1_rs::Tag;
use x509_parser::der_parser::oid::Oid;
use x509_parser::extensions;
use x509_parser::pem;
use x509_parser::prelude::*;

use digest::Digest;

struct Certificate {
  _buf: Vec<u8>,
  pem: Option<pem::Pem>,
  cert: X509Certificate<'static>,
}

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

impl Resource for Certificate {
  fn name(&self) -> Cow<str> {
    "x509Certificate".into()
  }
}

#[op2(fast)]
pub fn op_node_x509_parse(
  state: &mut OpState,
  #[buffer] buf: &[u8],
) -> Result<u32, AnyError> {
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
    cert: unsafe { std::mem::transmute(cert) },
    pem,
  };
  let rid = state.resource_table.add(cert);
  Ok(rid)
}

#[op2(fast)]
pub fn op_node_x509_ca(
  state: &mut OpState,
  rid: u32,
) -> Result<bool, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.is_ca())
}

#[op2(fast)]
pub fn op_node_x509_check_email(
  state: &mut OpState,
  rid: u32,
  #[string] email: &str,
) -> Result<bool, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;

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
      dbg!(name);
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
  state: &mut OpState,
  rid: u32,
) -> Result<Option<String>, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.fingerprint::<sha1::Sha1>())
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint256(
  state: &mut OpState,
  rid: u32,
) -> Result<Option<String>, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.fingerprint::<sha2::Sha256>())
}

#[op2]
#[string]
pub fn op_node_x509_fingerprint512(
  state: &mut OpState,
  rid: u32,
) -> Result<Option<String>, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.fingerprint::<sha2::Sha512>())
}

#[op2]
#[string]
pub fn op_node_x509_get_issuer(
  state: &mut OpState,
  rid: u32,
) -> Result<String, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(x509name_to_string(cert.issuer(), oid_registry())?)
}

#[op2]
#[string]
pub fn op_node_x509_get_subject(
  state: &mut OpState,
  rid: u32,
) -> Result<String, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
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
  state: &mut OpState,
  rid: u32,
) -> Result<String, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.validity().not_before.to_string())
}

#[op2]
#[string]
pub fn op_node_x509_get_valid_to(
  state: &mut OpState,
  rid: u32,
) -> Result<String, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  Ok(cert.validity().not_after.to_string())
}

#[op2]
#[string]
pub fn op_node_x509_get_serial_number(
  state: &mut OpState,
  rid: u32,
) -> Result<String, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;
  let mut s = cert.serial.to_str_radix(16);
  s.make_ascii_uppercase();
  Ok(s)
}

#[op2(fast)]
pub fn op_node_x509_key_usage(
  state: &mut OpState,
  rid: u32,
) -> Result<u16, AnyError> {
  let cert = state
    .resource_table
    .get::<Certificate>(rid)
    .map_err(|_| bad_resource_id())?;

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
