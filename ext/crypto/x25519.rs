// Copyright 2018-2026 the Deno authors. MIT license.

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use curve25519_dalek::montgomery::MontgomeryPoint;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use elliptic_curve::subtle::ConstantTimeEq;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X25519Error {
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
// u-coordinate of the base point.
const X25519_BASEPOINT_BYTES: [u8; 32] = [
  9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0,
];
#[op2(fast)]
pub fn op_crypto_generate_x25519_keypair(
  #[buffer] pkey: &mut [u8],
  #[buffer] pubkey: &mut [u8],
) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);
  // https://www.rfc-editor.org/rfc/rfc7748#section-6.1
  // pubkey = x25519(a, 9) which is constant-time Montgomery ladder.
  //   https://eprint.iacr.org/2014/140.pdf page 4
  //   https://eprint.iacr.org/2017/212.pdf algorithm 8
  // pubkey is in LE order.
  let pkey: [u8; 32] = pkey.try_into().expect("Expected byteLength 32");
  pubkey.copy_from_slice(&x25519_dalek::x25519(pkey, X25519_BASEPOINT_BYTES));
}

pub(crate) fn x25519_public_key(private_key: &[u8]) -> String {
  use base64::Engine;
  let private_key: [u8; 32] =
    private_key.try_into().expect("Expected byteLength 32");
  BASE64_URL_SAFE_NO_PAD
    .encode(x25519_dalek::x25519(private_key, X25519_BASEPOINT_BYTES))
}

#[op2]
#[string]
pub fn op_crypto_x25519_public_key(#[buffer] private_key: &[u8]) -> String {
  x25519_public_key(private_key)
}

const MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 32]);

/// Compute the X25519 shared secret from a raw 32-byte private key
/// `k` and 32-byte peer public key `u`, writing into `secret`. Returns
/// `Ok(true)` if the result is the Montgomery identity (low-order
/// point), in which case the caller must reject. Called from
/// [`crate::subtle_derive_bits::run`].
pub(crate) fn x25519_derive_bits(
  k: &[u8],
  u: &[u8],
  secret: &mut [u8],
) -> Result<bool, X25519Error> {
  let k: [u8; 32] = k.try_into().map_err(|_| X25519Error::InvalidKeyLength)?;
  let u: [u8; 32] = u.try_into().map_err(|_| X25519Error::InvalidKeyLength)?;
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

#[op2(fast)]
pub fn op_crypto_import_spki_x25519(
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
  if alg != X25519_OID {
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
pub fn op_crypto_import_pkcs8_x25519(
  #[buffer] key_data: &[u8],
  #[buffer] out: &mut [u8],
) -> bool {
  // 2-3.
  // This should probably use OneAsymmetricKey instead
  let pk_info = match PrivateKeyInfo::from_der(key_data) {
    Ok(pk_info) => pk_info,
    Err(_) => return false,
  };
  // 4.
  let alg = pk_info.algorithm.oid;
  if alg != X25519_OID {
    return false;
  }
  // 5.
  if pk_info.algorithm.parameters.is_some() {
    return false;
  }
  // 6.
  // CurvePrivateKey ::= OCTET STRING
  if pk_info.private_key.len() != 34 {
    return false;
  }
  out.copy_from_slice(&pk_info.private_key[2..]);
  true
}

pub(crate) fn export_spki_x25519(
  pubkey: &[u8],
) -> Result<Vec<u8>, X25519Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierRef {
      oid: X25519_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  Ok(key_info.to_der().map_err(|_| X25519Error::FailedExport)?)
}

#[op2]
pub fn op_crypto_export_spki_x25519(
  #[buffer] pubkey: &[u8],
) -> Result<Uint8Array, X25519Error> {
  export_spki_x25519(pubkey).map(Into::into)
}

pub(crate) fn export_pkcs8_x25519(pkey: &[u8]) -> Result<Vec<u8>, X25519Error> {
  use rsa::pkcs1::der::Encode;
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      oid: X25519_OID,
      parameters: None,
    },
    private_key: pkey,
  };
  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
}

#[op2]
pub fn op_crypto_export_pkcs8_x25519(
  #[buffer] pkey: &[u8],
) -> Result<Uint8Array, X25519Error> {
  export_pkcs8_x25519(pkey).map(Into::into)
}
