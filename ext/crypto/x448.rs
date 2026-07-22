// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint;
use ed448_goldilocks::elliptic_curve::bigint::U448;
use ed448_goldilocks::elliptic_curve::scalar::FromUintUnchecked;
use ed448_goldilocks::subtle::ConstantTimeEq;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Encode;
use spki::der::asn1::BitString;

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

/// Generate a random 56-byte X448 private scalar into `pkey` and write the
/// corresponding 56-byte Montgomery-form public key into `pubkey`. Called
/// from the cppgc X448 generate-key path in `subtle_generate_key.rs`.
pub fn generate_x448_keypair(pkey: &mut [u8], pubkey: &mut [u8]) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  // x448(pkey, 5)
  let pkey: &[u8; 56] = (&*pkey).try_into().expect("Expected byteLength 56");
  let point = x448(pkey, MontgomeryPoint::GENERATOR);
  pubkey.copy_from_slice(&point.0);
}

static MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 56]);

/// Compute the X448 shared secret from a raw 56-byte private key `k`
/// and 56-byte peer public key `u`, writing into `secret`. Returns
/// `Ok(true)` if the result is the Montgomery identity (low-order
/// point), in which case the caller must reject. Called from
/// [`crate::subtle_derive_bits::run`].
pub(crate) fn x448_derive_bits(
  k: &[u8],
  u: &[u8],
  secret: &mut [u8],
) -> Result<bool, X448Error> {
  let k: [u8; 56] = k.try_into().map_err(|_| X448Error::InvalidKeyLength)?;
  let u: [u8; 56] = u.try_into().map_err(|_| X448Error::InvalidKeyLength)?;

  // x448(k, u)
  let point = x448(&k, MontgomeryPoint(u));
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return Ok(true);
  }

  secret.copy_from_slice(&point.0);
  Ok(false)
}

// id-X448 OBJECT IDENTIFIER ::= { 1 3 101 111 }
pub const X448_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.111");

pub(crate) fn x448_public_key(private_key: &[u8]) -> Result<String, X448Error> {
  use base64::Engine;
  let private_key: [u8; 56] = private_key
    .try_into()
    .map_err(|_| X448Error::InvalidKeyLength)?;
  // x448(pkey, 5), identical derivation to generate_x448_keypair.
  let point = x448(&private_key, MontgomeryPoint::GENERATOR);
  Ok(BASE64_URL_SAFE_NO_PAD.encode(point.0))
}

pub(crate) fn export_spki_x448(pubkey: &[u8]) -> Result<Vec<u8>, X448Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierRef {
      oid: X448_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  key_info.to_der().map_err(|_| X448Error::FailedExport)
}

pub(crate) fn export_pkcs8_x448(pkey: &[u8]) -> Result<Vec<u8>, X448Error> {
  use rsa::pkcs1::der::Encode;
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      oid: X448_OID,
      parameters: None,
    },
    private_key: pkey,
  };
  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
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

  #[test]
  fn x448_rfc7748_ecdh() {
    // RFC 7748 section 5.2 X448 test vector.
    let scalar = from_hex(
      "3d262fddf9ec8e88495266fea19a34d28882acef045104d0d1aae121700a779c984c24f8cdd78fbff44943eba368f54b29259a4f1c600ad3",
    );
    let u = from_hex(
      "06fce640fa3487bfda5f6cf2d5263f8aad88334cbd07437f020f08f9814dc031ddbdc38c19c6da2583fa5429db94ada18aa7a7fb4ef8a086",
    );
    let expected = from_hex(
      "ce3e4ff95a60dc6697da1db1d85e6afbdf79b50a2412d7546d5f239fe14fbaadeb445fc66a01b0779d98223961111e21766282f73dd96b6f",
    );
    let scalar: [u8; 56] = scalar.try_into().unwrap();
    let u: [u8; 56] = u.try_into().unwrap();
    let point = x448(&scalar, MontgomeryPoint(u));
    assert_eq!(point.0.as_slice(), expected.as_slice());
  }
}
