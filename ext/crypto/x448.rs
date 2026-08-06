// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Encode;
use spki::der::asn1::BitString;
use subtle::ConstantTimeEq;

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

/// Generate a random 56-byte X448 private scalar into `pkey` and write the
/// corresponding 56-byte Montgomery-form public key into `pubkey`. Called
/// from the cppgc X448 generate-key path in `subtle_generate_key.rs`.
pub fn generate_x448_keypair(pkey: &mut [u8], pubkey: &mut [u8]) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  // x448(pkey, 5)
  let pkey: &[u8; 56] = (&*pkey).try_into().expect("Expected byteLength 56");
  let point = deno_crypto_provider::x448::x448(
    pkey,
    &deno_crypto_provider::x448::BASE_POINT,
  );
  pubkey.copy_from_slice(&point);
}

static MONTGOMERY_IDENTITY: [u8; 56] = [0; 56];

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
  let point = deno_crypto_provider::x448::x448(&k, &u);
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return Ok(true);
  }

  secret.copy_from_slice(&point);
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
  let point = deno_crypto_provider::x448::x448(
    &private_key,
    &deno_crypto_provider::x448::BASE_POINT,
  );
  Ok(BASE64_URL_SAFE_NO_PAD.encode(point))
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
