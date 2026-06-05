// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use curve25519_dalek::montgomery::MontgomeryPoint;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use elliptic_curve::subtle::ConstantTimeEq;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;

use crate::key_store::CryptoKeyHandle;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X25519Error {
  #[class("DOMExceptionOperationError")]
  #[error("Failed to export key")]
  FailedExport,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] spki::der::Error),
  #[class("DOMExceptionDataError")]
  #[error("Invalid key data")]
  InvalidKeyLength,
}
// u-coordinate of the base point.
const X25519_BASEPOINT_BYTES: [u8; 32] = [
  9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0,
];
/// Computes the X25519 public key for a private key, base64url-encoded.
pub fn x25519_public_key(private_key: &[u8]) -> String {
  use base64::Engine;

  let private_key: [u8; 32] =
    private_key.try_into().expect("Expected byteLength 32");
  BASE64_URL_SAFE_NO_PAD
    .encode(x25519_dalek::x25519(private_key, X25519_BASEPOINT_BYTES))
}

const MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 32]);

#[op2(fast)]
pub fn op_crypto_derive_bits_x25519(
  #[cppgc] k: &CryptoKeyHandle,
  #[cppgc] u: &CryptoKeyHandle,
  #[buffer] secret: &mut [u8],
) -> Result<bool, X25519Error> {
  let k: [u8; 32] = k
    .data()
    .bytes()
    .try_into()
    .map_err(|_| X25519Error::InvalidKeyLength)?;
  let u: [u8; 32] = u
    .data()
    .bytes()
    .try_into()
    .map_err(|_| X25519Error::InvalidKeyLength)?;
  let sh_sec = x25519_dalek::x25519(k, u);
  let point = MontgomeryPoint(sh_sec);
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return Ok(true);
  }
  secret.copy_from_slice(&sh_sec);
  Ok(false)
}

// id-X25519 OBJECT IDENTIFIER ::= { 1 3 101 110 }
pub const X25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.110");

/// Extracts the raw X25519 public key bytes from SPKI DER.
pub fn import_spki_x25519(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = spki::SubjectPublicKeyInfoRef::try_from(key_data).ok()?;
  if pk_info.algorithm.oid != X25519_OID {
    return None;
  }
  if pk_info.algorithm.parameters.is_some() {
    return None;
  }
  Some(pk_info.subject_public_key.raw_bytes().to_vec())
}

/// Extracts the raw X25519 private key bytes from PKCS#8 DER.
pub fn import_pkcs8_x25519(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = PrivateKeyInfo::from_der(key_data).ok()?;
  if pk_info.algorithm.oid != X25519_OID {
    return None;
  }
  if pk_info.algorithm.parameters.is_some() {
    return None;
  }
  // CurvePrivateKey ::= OCTET STRING
  if pk_info.private_key.len() != 34 {
    return None;
  }
  Some(pk_info.private_key[2..].to_vec())
}

/// Core of [`op_crypto_export_spki_x25519`].
pub fn export_spki_x25519(pubkey: &[u8]) -> Result<Vec<u8>, X25519Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierRef {
      // id-X25519
      oid: X25519_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  key_info.to_der().map_err(|_| X25519Error::FailedExport)
}

#[op2]
pub fn op_crypto_export_spki_x25519(
  #[buffer] pubkey: &[u8],
) -> Result<Uint8Array, X25519Error> {
  Ok(export_spki_x25519(pubkey)?.into())
}

/// Core of [`op_crypto_export_pkcs8_x25519`].
pub fn export_pkcs8_x25519(pkey: &[u8]) -> Result<Vec<u8>, X25519Error> {
  use rsa::pkcs1::der::Encode;

  // This should probably use OneAsymmetricKey instead
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      // id-X25519
      oid: X25519_OID,
      parameters: None,
    },
    private_key: pkey, // OCTET STRING
  };

  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
}

#[op2]
pub fn op_crypto_export_pkcs8_x25519(
  #[buffer] pkey: &[u8],
) -> Result<Uint8Array, X25519Error> {
  Ok(export_pkcs8_x25519(pkey)?.into())
}
