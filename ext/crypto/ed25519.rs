// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use rand::rngs::OsRng;
use rand::RngCore;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;
use spki::der::Decode;
use spki::der::Encode;

#[op(fast)]
pub fn op_generate_ed25519_keypair(pkey: &mut [u8], pubkey: &mut [u8]) -> bool {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  let pair = match Ed25519KeyPair::from_seed_unchecked(pkey) {
    Ok(p) => p,
    Err(_) => return false,
  };
  pubkey.copy_from_slice(pair.public_key().as_ref());
  true
}

#[op(fast)]
pub fn op_sign_ed25519(key: &[u8], data: &[u8], signature: &mut [u8]) -> bool {
  let pair = match Ed25519KeyPair::from_seed_unchecked(key) {
    Ok(p) => p,
    Err(_) => return false,
  };
  signature.copy_from_slice(pair.sign(data).as_ref());
  true
}

#[op(fast)]
pub fn op_verify_ed25519(pubkey: &[u8], data: &[u8], signature: &[u8]) -> bool {
  ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pubkey)
    .verify(data, signature)
    .is_ok()
}

// id-Ed25519 OBJECT IDENTIFIER ::= { 1 3 101 112 }
pub const ED25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.112");

#[op(fast)]
pub fn op_import_spki_ed25519(key_data: &[u8], out: &mut [u8]) -> bool {
  // 2-3.
  let pk_info = match spki::SubjectPublicKeyInfo::from_der(key_data) {
    Ok(pk_info) => pk_info,
    Err(_) => return false,
  };
  // 4.
  let alg = pk_info.algorithm.oid;
  if alg != ED25519_OID {
    return false;
  }
  // 5.
  if pk_info.algorithm.parameters.is_some() {
    return false;
  }
  out.copy_from_slice(pk_info.subject_public_key);
  true
}

#[op(fast)]
pub fn op_import_pkcs8_ed25519(key_data: &[u8], out: &mut [u8]) -> bool {
  // 2-3.
  // This should probably use OneAsymmetricKey instead
  let pk_info = match PrivateKeyInfo::from_der(key_data) {
    Ok(pk_info) => pk_info,
    Err(_) => return false,
  };
  // 4.
  let alg = pk_info.algorithm.oid;
  if alg != ED25519_OID {
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

#[op]
pub fn op_export_spki_ed25519(pubkey: &[u8]) -> Result<ZeroCopyBuf, AnyError> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifier {
      // id-Ed25519
      oid: ED25519_OID,
      parameters: None,
    },
    subject_public_key: pubkey,
  };
  Ok(key_info.to_vec()?.into())
}

#[op]
pub fn op_export_pkcs8_ed25519(pkey: &[u8]) -> Result<ZeroCopyBuf, AnyError> {
  // This should probably use OneAsymmetricKey instead
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifier {
      // id-Ed25519
      oid: ED25519_OID,
      parameters: None,
    },
    private_key: pkey, // OCTET STRING
  };

  Ok(pk_info.to_vec()?.into())
}

// 'x' from Section 2 of RFC 8037
// https://www.rfc-editor.org/rfc/rfc8037#section-2
#[op]
pub fn op_jwk_x_ed25519(pkey: &[u8]) -> Result<String, AnyError> {
  let pair = Ed25519KeyPair::from_seed_unchecked(pkey)?;
  Ok(base64::encode_config(
    pair.public_key().as_ref(),
    base64::URL_SAFE_NO_PAD,
  ))
}
