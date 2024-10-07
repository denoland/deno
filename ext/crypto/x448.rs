// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::ToJsBuffer;
use ed448_goldilocks::curve::MontgomeryPoint;
use ed448_goldilocks::Scalar;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use elliptic_curve::subtle::ConstantTimeEq;
use rand::rngs::OsRng;
use rand::RngCore;
use spki::der::asn1::BitString;
use spki::der::Decode;
use spki::der::Encode;

#[op2(fast)]
pub fn op_crypto_generate_x448_keypair(
  #[buffer] pkey: &mut [u8],
  #[buffer] pubkey: &mut [u8],
) {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  // x448(pkey, 5)
  let point = &MontgomeryPoint::generator()
    * &Scalar::from_bytes(pkey.try_into().unwrap());
  pubkey.copy_from_slice(&point.0);
}

const MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 56]);

#[op2(fast)]
pub fn op_crypto_derive_bits_x448(
  #[buffer] k: &[u8],
  #[buffer] u: &[u8],
  #[buffer] secret: &mut [u8],
) -> bool {
  let k: [u8; 56] = k.try_into().expect("Expected byteLength 56");
  let u: [u8; 56] = u.try_into().expect("Expected byteLength 56");

  // x448(k, u)
  let point = &MontgomeryPoint(u) * &Scalar::from_bytes(k);
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return true;
  }

  secret.copy_from_slice(&point.0);
  false
}

// id-X448 OBJECT IDENTIFIER ::= { 1 3 101 111 }
const X448_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.111");

#[op2]
#[serde]
pub fn op_crypto_export_spki_x448(
  #[buffer] pubkey: &[u8],
) -> Result<ToJsBuffer, AnyError> {
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
      .map_err(|_| {
        custom_error("DOMExceptionOperationError", "Failed to export key")
      })?
      .into(),
  )
}

#[op2]
#[serde]
pub fn op_crypto_export_pkcs8_x448(
  #[buffer] pkey: &[u8],
) -> Result<ToJsBuffer, AnyError> {
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
  if pk_info.private_key.len() != 56 {
    return false;
  }
  out.copy_from_slice(&pk_info.private_key[2..]);
  true
}
