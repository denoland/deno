// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::signature::Ed25519KeyPair;
use aws_lc_rs::signature::KeyPair;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use spki::der::Encode;
use spki::der::asn1::BitString;

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

/// Rust-callable wrapper for [`op_crypto_generate_ed25519_keypair`]. Fills
/// the provided 32-byte buffers with a random Ed25519 keypair.
pub fn generate_ed25519_keypair(pkey: &mut [u8], pubkey: &mut [u8]) -> bool {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);
  let pair = match Ed25519KeyPair::from_seed_unchecked(pkey) {
    Ok(p) => p,
    Err(_) => return false,
  };
  pubkey.copy_from_slice(pair.public_key().as_ref());
  true
}

/// Ed25519 raw sign. `seed` is the 32-byte raw private key,
/// `signature` is the 64-byte destination buffer. Returns `false` if
/// the seed is malformed. Called from
/// [`crate::subtle_sign::run`].
pub(crate) fn ed25519_sign_into(
  seed: &[u8],
  data: &[u8],
  signature: &mut [u8],
) -> bool {
  let pair = match Ed25519KeyPair::from_seed_unchecked(seed) {
    Ok(p) => p,
    Err(_) => return false,
  };
  signature.copy_from_slice(pair.sign(data).as_ref());
  true
}

/// Ed25519 verify. `pubkey` is the raw 32-byte public key. Called from
/// [`crate::subtle_verify::run`].
pub(crate) fn ed25519_verify(
  pubkey: &[u8],
  data: &[u8],
  signature: &[u8],
) -> bool {
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

pub(crate) fn export_spki_ed25519(
  pubkey: &[u8],
) -> Result<Vec<u8>, Ed25519Error> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      oid: ED25519_OID,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(pubkey)?,
  };
  key_info.to_der().map_err(|_| Ed25519Error::FailedExport)
}

pub(crate) fn export_pkcs8_ed25519(
  pkey: &[u8],
) -> Result<Vec<u8>, Ed25519Error> {
  use rsa::pkcs1::der::Encode;
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
      oid: ED25519_OID,
      parameters: None,
    },
    private_key: pkey,
  };
  let mut buf = Vec::new();
  pk_info.encode_to_vec(&mut buf)?;
  Ok(buf)
}

pub(crate) fn jwk_x_ed25519(pkey: &[u8]) -> Result<String, Ed25519Error> {
  let pair = Ed25519KeyPair::from_seed_unchecked(pkey)?;
  Ok(BASE64_URL_SAFE_NO_PAD.encode(pair.public_key().as_ref()))
}

// 'x' from Section 2 of RFC 8037
// https://www.rfc-editor.org/rfc/rfc8037#section-2
