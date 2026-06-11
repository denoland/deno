// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint;
use ed448_goldilocks::elliptic_curve::bigint::U448;
use ed448_goldilocks::elliptic_curve::scalar::FromUintUnchecked;
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

/// The X448 function from RFC 7748: decode and clamp the scalar `k`
/// (section 5) and perform the Montgomery ladder against the point `u`.
///
/// The clamped scalar has its top bit (bit 447) set, so it exceeds the
/// Ed448 group order. It must therefore be used verbatim rather than
/// reduced mod order (which `EdwardsScalar::from_bytes_mod_order` does)
/// or the derived point is wrong. See denoland/deno#35155.
fn x448(k: &[u8; 56], u: MontgomeryPoint) -> MontgomeryPoint {
  // decodeScalar448 (RFC 7748, section 5).
  let mut scalar_bytes = *k;
  scalar_bytes[0] &= 252;
  scalar_bytes[55] |= 128;
  let scalar =
    EdwardsScalar::from_uint_unchecked(U448::from_le_slice(&scalar_bytes));
  &u * &scalar
}

#[op2(fast)]
pub fn op_crypto_generate_x448_keypair(
  #[buffer] pkey: &mut [u8],
  #[buffer] pubkey: &mut [u8],
) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  // x448(pkey, 5)
  let pkey: &[u8; 56] = (&*pkey).try_into().expect("Expected byteLength 56");
  let point = x448(pkey, MontgomeryPoint::GENERATOR);
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
  let point = x448(&k, MontgomeryPoint(u));
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
  let point = x448(&private_key, MontgomeryPoint::GENERATOR);
  Ok(BASE64_URL_SAFE_NO_PAD.encode(point.0))
}

#[op2]
pub fn op_crypto_export_spki_x448(
  #[buffer] pubkey: &[u8],
) -> Result<Uint8Array, X448Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierRef {
      oid: X448_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  Ok(
    key_info
      .to_der()
      .map_err(|_| X448Error::FailedExport)?
      .into(),
  )
}

#[op2]
pub fn op_crypto_export_pkcs8_x448(
  #[buffer] pkey: &[u8],
) -> Result<Uint8Array, X448Error> {
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
  Ok(buf.into())
}

#[op2(fast)]
pub fn op_crypto_import_spki_x448(
  #[buffer] key_data: &[u8],
  #[buffer] out: &mut [u8],
) -> bool {
  // 2-3.
  let pk_info = match spki::SubjectPublicKeyInfoRef::try_from(key_data) {
    Ok(pk_info) => pk_info,
    Err(_) => return false,
  };
  // 4.
  let alg = pk_info.algorithm.oid;
  if alg != X448_OID {
    return false;
  }
  // 5.
  if pk_info.algorithm.parameters.is_some() {
    return false;
  }
  out.copy_from_slice(pk_info.subject_public_key.raw_bytes());
  true
}

#[op2(fast)]
pub fn op_crypto_import_pkcs8_x448(
  #[buffer] key_data: &[u8],
  #[buffer] out: &mut [u8],
) -> bool {
  // 2-3.
  let pk_info = match PrivateKeyInfo::from_der(key_data) {
    Ok(pk_info) => pk_info,
    Err(_) => return false,
  };
  // 4.
  let alg = pk_info.algorithm.oid;
  if alg != X448_OID {
    return false;
  }
  // 5.
  if pk_info.algorithm.parameters.is_some() {
    return false;
  }
  // 6.
  // CurvePrivateKey ::= OCTET STRING
  if pk_info.private_key.len() != 58 {
    return false;
  }
  out.copy_from_slice(&pk_info.private_key[2..]);
  true
}

#[cfg(test)]
mod tests {
  use super::*;

  fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
      .step_by(2)
      .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
      .collect()
  }

  #[test]
  fn x448_public_key_from_scalar() {
    // Test vector from denoland/deno#35155.
    let scalar = from_hex(
      "27a4354608f3bdd38f1f5af305f3e0682efe4e25808249d8fcb55927f6a9f446b8dc1d0a2c3b8cb133a5673b59a6d55ce754ec0c9a555401",
    );
    let expected = from_hex(
      "145d083ea7a6379dbb32dcbd8aff4c206ea5d069b75e96c6dd2a3e38f441471ac97adca641fdad66685a96f32b7c3e064635fab3cc89234e",
    );

    let scalar: [u8; 56] = scalar.try_into().unwrap();
    let point = x448(&scalar, MontgomeryPoint::GENERATOR);
    assert_eq!(point.0.as_slice(), expected.as_slice());
  }
}
