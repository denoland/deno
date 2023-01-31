// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use curve25519_dalek::montgomery::MontgomeryPoint;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use elliptic_curve::subtle::ConstantTimeEq;
use rand::rngs::OsRng;
use rand::RngCore;
use spki::der::Decode;
use spki::der::Encode;

#[op(fast)]
pub fn op_generate_x25519_keypair(pkey: &mut [u8], pubkey: &mut [u8]) {
  // u-coordinate of the base point.
  const X25519_BASEPOINT_BYTES: [u8; 32] = [
    9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
  ];
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

const MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 32]);

#[op(fast)]
pub fn op_derive_bits_x25519(k: &[u8], u: &[u8], secret: &mut [u8]) -> bool {
  let k: [u8; 32] = k.try_into().expect("Expected byteLength 32");
  let u: [u8; 32] = u.try_into().expect("Expected byteLength 32");
  let sh_sec = x25519_dalek::x25519(k, u);
  let point = MontgomeryPoint(sh_sec);
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return false;
  }
  secret.copy_from_slice(&sh_sec);
  true
}

// id-X25519 OBJECT IDENTIFIER ::= { 1 3 101 110 }
pub const X25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.110");

#[op(fast)]
pub fn op_import_spki_x25519(key_data: &[u8], out: &mut [u8]) -> bool {
  // 2-3.
  let pk_info = match spki::SubjectPublicKeyInfo::from_der(key_data) {
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
  out.copy_from_slice(pk_info.subject_public_key);
  true
}

#[op(fast)]
pub fn op_import_pkcs8_x25519(key_data: &[u8], out: &mut [u8]) -> bool {
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

#[op]
pub fn op_export_spki_x25519(pubkey: &[u8]) -> Result<ZeroCopyBuf, AnyError> {
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifier {
      // id-X25519
      oid: X25519_OID,
      parameters: None,
    },
    subject_public_key: pubkey,
  };
  Ok(key_info.to_vec()?.into())
}

#[op]
pub fn op_export_pkcs8_x25519(pkey: &[u8]) -> Result<ZeroCopyBuf, AnyError> {
  // This should probably use OneAsymmetricKey instead
  let pk_info = rsa::pkcs8::PrivateKeyInfo {
    public_key: None,
    algorithm: rsa::pkcs8::AlgorithmIdentifier {
      // id-X25519
      oid: X25519_OID,
      parameters: None,
    },
    private_key: pkey, // OCTET STRING
  };

  Ok(pk_info.to_vec()?.into())
}
