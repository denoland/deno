// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint;
use ed448_goldilocks::subtle::ConstantTimeEq;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;

use crate::key_store::CryptoKeyHandle;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X448Error {
  #[class("DOMExceptionOperationError")]
  #[error("Failed to export key")]
  FailedExport,
  #[class("DOMExceptionDataError")]
  #[error("Invalid key data")]
  InvalidKeyLength,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] spki::der::Error),
}

#[op2(fast)]
pub fn op_crypto_generate_x448_keypair(
  #[buffer] pkey: &mut [u8],
  #[buffer] pubkey: &mut [u8],
) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  // x448(pkey, 5)
  let mut scalar_bytes = [0u8; 57];
  scalar_bytes[..56].copy_from_slice(pkey);
  let scalar = EdwardsScalar::from_bytes_mod_order(&scalar_bytes.into());
  let point = &MontgomeryPoint::GENERATOR * &scalar;
  pubkey.copy_from_slice(&point.0);
}

static MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 56]);

#[op2(fast)]
pub fn op_crypto_derive_bits_x448(
  #[cppgc] k: &CryptoKeyHandle,
  #[cppgc] u: &CryptoKeyHandle,
  #[buffer] secret: &mut [u8],
) -> Result<bool, X448Error> {
  let k: [u8; 56] = k
    .data()
    .bytes()
    .try_into()
    .map_err(|_| X448Error::InvalidKeyLength)?;
  let u: [u8; 56] = u
    .data()
    .bytes()
    .try_into()
    .map_err(|_| X448Error::InvalidKeyLength)?;

  // x448(k, u)
  let mut scalar_bytes = [0u8; 57];
  scalar_bytes[..56].copy_from_slice(&k);
  let scalar = EdwardsScalar::from_bytes_mod_order(&scalar_bytes.into());
  let point = &MontgomeryPoint(u) * &scalar;
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return Ok(true);
  }

  secret.copy_from_slice(&point.0);
  Ok(false)
}

// id-X448 OBJECT IDENTIFIER ::= { 1 3 101 111 }
const X448_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.111");

#[op2]
#[string]
pub fn op_crypto_x448_public_key(
  #[buffer] private_key: &[u8],
) -> Result<String, X448Error> {
  use base64::Engine;

  let private_key: [u8; 56] = private_key
    .try_into()
    .map_err(|_| X448Error::InvalidKeyLength)?;
  // x448(pkey, 5), identical derivation to op_crypto_generate_x448_keypair.
  let mut scalar_bytes = [0u8; 57];
  scalar_bytes[..56].copy_from_slice(&private_key);
  let scalar = EdwardsScalar::from_bytes_mod_order(&scalar_bytes.into());
  let point = &MontgomeryPoint::GENERATOR * &scalar;
  Ok(BASE64_URL_SAFE_NO_PAD.encode(point.0))
}

/// Core of [`op_crypto_export_spki_x448`].
pub fn export_spki_x448(pubkey: &[u8]) -> Result<Vec<u8>, X448Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierRef {
      oid: X448_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  key_info.to_der().map_err(|_| X448Error::FailedExport)
}

#[op2]
pub fn op_crypto_export_spki_x448(
  #[buffer] pubkey: &[u8],
) -> Result<Uint8Array, X448Error> {
  Ok(export_spki_x448(pubkey)?.into())
}

/// Core of [`op_crypto_export_pkcs8_x448`].
pub fn export_pkcs8_x448(pkey: &[u8]) -> Result<Vec<u8>, X448Error> {
  use rsa::pkcs1::der::Encode;

  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      oid: X448_OID,
      parameters: None,
    },
    private_key: pkey, // OCTET STRING
  };

  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
}

#[op2]
pub fn op_crypto_export_pkcs8_x448(
  #[buffer] pkey: &[u8],
) -> Result<Uint8Array, X448Error> {
  Ok(export_pkcs8_x448(pkey)?.into())
}

/// Extracts the raw X448 public key bytes from SPKI DER.
pub fn import_spki_x448(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = spki::SubjectPublicKeyInfoRef::try_from(key_data).ok()?;
  if pk_info.algorithm.oid != X448_OID {
    return None;
  }
  if pk_info.algorithm.parameters.is_some() {
    return None;
  }
  Some(pk_info.subject_public_key.raw_bytes().to_vec())
}

/// Extracts the raw X448 private key bytes from PKCS#8 DER.
pub fn import_pkcs8_x448(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = PrivateKeyInfo::from_der(key_data).ok()?;
  if pk_info.algorithm.oid != X448_OID {
    return None;
  }
  if pk_info.algorithm.parameters.is_some() {
    return None;
  }
  // CurvePrivateKey ::= OCTET STRING
  if pk_info.private_key.len() != 58 {
    return None;
  }
  Some(pk_info.private_key[2..].to_vec())
}
