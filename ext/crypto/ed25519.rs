// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::signature::Ed25519KeyPair;
use aws_lc_rs::signature::KeyPair;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;

use crate::key_store::CryptoKeyHandle;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum Ed25519Error {
  #[class("DOMExceptionOperationError")]
  #[error("Failed to export key")]
  FailedExport,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] rsa::pkcs1::der::Error),
  #[class(generic)]
  #[error(transparent)]
  KeyRejected(#[from] aws_lc_rs::error::KeyRejected),
}

#[op2(fast)]
pub fn op_crypto_generate_ed25519_keypair(
  #[buffer] pkey: &mut [u8],
  #[buffer] pubkey: &mut [u8],
) -> bool {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  let pair = match Ed25519KeyPair::from_seed_unchecked(pkey) {
    Ok(p) => p,
    Err(_) => return false,
  };
  pubkey.copy_from_slice(pair.public_key().as_ref());
  true
}

#[op2(fast)]
pub fn op_crypto_sign_ed25519(
  #[cppgc] key: &CryptoKeyHandle,
  #[buffer] data: &[u8],
  #[buffer] signature: &mut [u8],
) -> bool {
  let key = key.data().bytes();
  let pair = match Ed25519KeyPair::from_seed_unchecked(key) {
    Ok(p) => p,
    Err(_) => return false,
  };
  signature.copy_from_slice(pair.sign(data).as_ref());
  true
}

#[op2(fast)]
pub fn op_crypto_verify_ed25519(
  #[cppgc] pubkey: &CryptoKeyHandle,
  #[buffer] data: &[u8],
  #[buffer] signature: &[u8],
) -> bool {
  let pubkey = pubkey.data().bytes();
  aws_lc_rs::signature::UnparsedPublicKey::new(
    &aws_lc_rs::signature::ED25519,
    pubkey,
  )
  .verify(data, signature)
  .is_ok()
}

// id-Ed25519 OBJECT IDENTIFIER ::= { 1 3 101 112 }
pub const ED25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.112");

/// Returns the 32-byte raw Ed25519 public key from SPKI DER on success.
/// Callable from Rust (the cppgc `importKey` path).
pub fn import_spki_ed25519(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = spki::SubjectPublicKeyInfoRef::try_from(key_data).ok()?;
  if pk_info.algorithm.oid != ED25519_OID {
    return None;
  }
  if pk_info.algorithm.parameters.is_some() {
    return None;
  }
  Some(pk_info.subject_public_key.raw_bytes().to_vec())
}

/// Returns the 32-byte raw Ed25519 private key from PKCS#8 DER on success.
pub fn import_pkcs8_ed25519(key_data: &[u8]) -> Option<Vec<u8>> {
  let pk_info = PrivateKeyInfo::from_der(key_data).ok()?;
  if pk_info.algorithm.oid != ED25519_OID {
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

/// Core of [`op_crypto_export_spki_ed25519`].
pub fn export_spki_ed25519(pubkey: &[u8]) -> Result<Vec<u8>, Ed25519Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      // id-Ed25519
      oid: ED25519_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  key_info.to_der().map_err(|_| Ed25519Error::FailedExport)
}

#[op2]
pub fn op_crypto_export_spki_ed25519(
  #[buffer] pubkey: &[u8],
) -> Result<Uint8Array, Ed25519Error> {
  Ok(export_spki_ed25519(pubkey)?.into())
}

/// Core of [`op_crypto_export_pkcs8_ed25519`].
pub fn export_pkcs8_ed25519(pkey: &[u8]) -> Result<Vec<u8>, Ed25519Error> {
  use rsa::pkcs1::der::Encode;

  // This should probably use OneAsymmetricKey instead
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      // id-Ed25519
      oid: ED25519_OID,
      parameters: None,
    },
    private_key: pkey, // OCTET STRING
  };

  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
}

#[op2]
pub fn op_crypto_export_pkcs8_ed25519(
  #[buffer] pkey: &[u8],
) -> Result<Uint8Array, Ed25519Error> {
  Ok(export_pkcs8_ed25519(pkey)?.into())
}

// 'x' from Section 2 of RFC 8037
// https://www.rfc-editor.org/rfc/rfc8037#section-2
//
// Computes the base64url-encoded Ed25519 public key ('x') from a seed.
pub fn jwk_x_ed25519(pkey: &[u8]) -> Result<String, Ed25519Error> {
  let pair = Ed25519KeyPair::from_seed_unchecked(pkey)?;
  Ok(BASE64_URL_SAFE_NO_PAD.encode(pair.public_key().as_ref()))
}
