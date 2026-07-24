// Copyright 2018-2026 the Deno authors. MIT license.

//! aws-lc fast paths for the `SubtleCrypto.sign()` / `verify()` RSA and
//! ECDSA arms.
//!
//! Parse-first dispatch: an operation commits to aws-lc only when the
//! (algorithm, curve, hash, salt length) combination maps to an aws-lc
//! algorithm and aws-lc accepts the key material. From then on verify
//! failures mean "invalid signature" (false) and sign failures are
//! errors. Anything else returns `None` and the caller runs the
//! RustCrypto path, whose behavior is unchanged.

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature;
use aws_lc_rs::signature::EcdsaKeyPair;
use aws_lc_rs::signature::EcdsaSigningAlgorithm;
use aws_lc_rs::signature::KeyPair as _;
use aws_lc_rs::signature::ParsedPublicKey;
use aws_lc_rs::signature::RsaEncoding;
use aws_lc_rs::signature::RsaKeyPair;
use aws_lc_rs::signature::VerificationAlgorithm;

use crate::CryptoError;
use crate::KeyData;
use crate::KeyType;
use crate::SignArg;
use crate::VerifyArg;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;

/// RSA modulus sizes accepted by the aws-lc `RSA_*_2048_8192_*`
/// verification parameters used below.
const RSA_MODULUS_BITS: std::ops::RangeInclusive<u32> = 2048..=8192;

/// Attempts to sign via aws-lc. `Ok(None)` means the fast path does not
/// apply and the caller must run the RustCrypto path.
pub(crate) fn try_sign(
  key: &KeyData,
  args: &SignArg,
  data: &[u8],
) -> Result<Option<Vec<u8>>, CryptoError> {
  if !matches!(key.r#type, KeyType::Private) {
    return Ok(None);
  }
  match args.algorithm {
    Algorithm::RsassaPkcs1v15 | Algorithm::RsaPss => {
      let Some(hash) = args.hash else {
        return Ok(None);
      };
      let Some(encoding) =
        rsa_sign_encoding(args.algorithm, hash, args.salt_length)
      else {
        return Ok(None);
      };
      // Parse also enforces the 2048-8192 bit modulus range; smaller or
      // larger keys are rejected here and take the fallback path.
      let Ok(key_pair) = RsaKeyPair::from_der(&key.data) else {
        return Ok(None);
      };
      let mut signature_bytes = vec![0u8; key_pair.public_modulus_len()];
      key_pair.sign(
        encoding,
        &SystemRandom::new(),
        data,
        &mut signature_bytes,
      )?;
      Ok(Some(signature_bytes))
    }
    Algorithm::Ecdsa => {
      let Some(hash) = args.hash else {
        return Ok(None);
      };
      let Some(named_curve) = args.named_curve else {
        return Ok(None);
      };
      let Some(alg) = ecdsa_signing_alg(named_curve, hash) else {
        return Ok(None);
      };
      let Ok(key_pair) = EcdsaKeyPair::from_pkcs8(alg, &key.data) else {
        return Ok(None);
      };
      let sig = key_pair.sign(&SystemRandom::new(), data)?;
      Ok(Some(sig.as_ref().to_vec()))
    }
    _ => Ok(None),
  }
}

/// Attempts to verify via aws-lc. `None` means the fast path does not
/// apply and the caller must run the RustCrypto path.
///
/// `KeyType::Private` key data is also handled: generated key pairs store
/// the private-key DER as the public `CryptoKey`'s data (see
/// `subtle_generate_key.rs`), so the public key is extracted from it.
pub(crate) fn try_verify(
  key: &KeyData,
  args: &VerifyArg,
  data: &[u8],
) -> Option<bool> {
  let hash = args.hash?;
  match args.algorithm {
    Algorithm::RsassaPkcs1v15 | Algorithm::RsaPss => {
      let alg = rsa_verification_alg(args.algorithm, hash, args.salt_length)?;
      let parsed = match key.r#type {
        KeyType::Public => {
          // aws-lc checks the modulus range during verification, where a
          // failure is indistinguishable from an invalid signature; gate
          // on the size up front so out-of-range keys keep today's
          // behavior.
          let bits = rsa_public_key_modulus_bits(&key.data)?;
          if !RSA_MODULUS_BITS.contains(&bits) {
            return None;
          }
          ParsedPublicKey::new(alg, &key.data).ok()?
        }
        KeyType::Private => {
          // Parse enforces the 2048-8192 bit modulus range.
          let key_pair = RsaKeyPair::from_der(&key.data).ok()?;
          ParsedPublicKey::new(alg, key_pair.public_key()).ok()?
        }
        KeyType::Secret => return None,
      };
      Some(parsed.verify_sig(data, &args.signature).is_ok())
    }
    Algorithm::Ecdsa => {
      let named_curve = args.named_curve?;
      let alg = ecdsa_verification_alg(named_curve, hash)?;
      let parsed = match key.r#type {
        KeyType::Public => ParsedPublicKey::new(alg, &key.data).ok()?,
        KeyType::Private => {
          let signing_alg = ecdsa_signing_alg(named_curve, hash)?;
          let key_pair =
            EcdsaKeyPair::from_pkcs8(signing_alg, &key.data).ok()?;
          ParsedPublicKey::new(alg, key_pair.public_key()).ok()?
        }
        KeyType::Secret => return None,
      };
      Some(parsed.verify_sig(data, &args.signature).is_ok())
    }
    _ => None,
  }
}

// The (algorithm, hash, curve, salt) matrix below must mirror the arms in
// `sign_key_sync` / `verify_key_sync` (lib.rs) so the fast path and the
// fallback agree on which inputs each handles.
fn rsa_sign_encoding(
  algorithm: Algorithm,
  hash: CryptoHash,
  salt_length: Option<u32>,
) -> Option<&'static dyn RsaEncoding> {
  match algorithm {
    Algorithm::RsassaPkcs1v15 => match hash {
      CryptoHash::Sha256 => Some(&signature::RSA_PKCS1_SHA256),
      CryptoHash::Sha384 => Some(&signature::RSA_PKCS1_SHA384),
      CryptoHash::Sha512 => Some(&signature::RSA_PKCS1_SHA512),
      _ => None,
    },
    // aws-lc hardcodes the PSS salt length to the digest length.
    Algorithm::RsaPss => match (hash, salt_length?) {
      (CryptoHash::Sha256, 32) => Some(&signature::RSA_PSS_SHA256),
      (CryptoHash::Sha384, 48) => Some(&signature::RSA_PSS_SHA384),
      (CryptoHash::Sha512, 64) => Some(&signature::RSA_PSS_SHA512),
      _ => None,
    },
    _ => None,
  }
}

fn rsa_verification_alg(
  algorithm: Algorithm,
  hash: CryptoHash,
  salt_length: Option<u32>,
) -> Option<&'static dyn VerificationAlgorithm> {
  match algorithm {
    Algorithm::RsassaPkcs1v15 => match hash {
      CryptoHash::Sha256 => Some(&signature::RSA_PKCS1_2048_8192_SHA256),
      CryptoHash::Sha384 => Some(&signature::RSA_PKCS1_2048_8192_SHA384),
      CryptoHash::Sha512 => Some(&signature::RSA_PKCS1_2048_8192_SHA512),
      _ => None,
    },
    // aws-lc hardcodes the PSS salt length to the digest length.
    Algorithm::RsaPss => match (hash, salt_length?) {
      (CryptoHash::Sha256, 32) => Some(&signature::RSA_PSS_2048_8192_SHA256),
      (CryptoHash::Sha384, 48) => Some(&signature::RSA_PSS_2048_8192_SHA384),
      (CryptoHash::Sha512, 64) => Some(&signature::RSA_PSS_2048_8192_SHA512),
      _ => None,
    },
    _ => None,
  }
}

/// WebCrypto ECDSA uses the fixed-length r||s signature format, so only
/// the `_FIXED` aws-lc algorithms apply. Curve/hash pairs without a
/// `_FIXED` variant fall back.
fn ecdsa_signing_alg(
  curve: CryptoNamedCurve,
  hash: CryptoHash,
) -> Option<&'static EcdsaSigningAlgorithm> {
  match (curve, hash) {
    (CryptoNamedCurve::P256, CryptoHash::Sha256) => {
      Some(&signature::ECDSA_P256_SHA256_FIXED_SIGNING)
    }
    (CryptoNamedCurve::P384, CryptoHash::Sha384) => {
      Some(&signature::ECDSA_P384_SHA384_FIXED_SIGNING)
    }
    (CryptoNamedCurve::P521, CryptoHash::Sha512) => {
      Some(&signature::ECDSA_P521_SHA512_FIXED_SIGNING)
    }
    _ => None,
  }
}

fn ecdsa_verification_alg(
  curve: CryptoNamedCurve,
  hash: CryptoHash,
) -> Option<&'static dyn VerificationAlgorithm> {
  match (curve, hash) {
    (CryptoNamedCurve::P256, CryptoHash::Sha256) => {
      Some(&signature::ECDSA_P256_SHA256_FIXED)
    }
    (CryptoNamedCurve::P384, CryptoHash::Sha384) => {
      Some(&signature::ECDSA_P384_SHA384_FIXED)
    }
    (CryptoNamedCurve::P521, CryptoHash::Sha512) => {
      Some(&signature::ECDSA_P521_SHA512_FIXED)
    }
    _ => None,
  }
}

/// Modulus bit length of a DER-encoded PKCS#1 `RSAPublicKey`, the format
/// `import_key` stores for RSA public keys. Returns `None` when the input
/// does not parse as one.
fn rsa_public_key_modulus_bits(der: &[u8]) -> Option<u32> {
  use rsa::pkcs1::der::Decode as _;
  let public_key = rsa::pkcs1::RsaPublicKey::from_der(der).ok()?;
  let modulus = public_key.modulus.as_bytes();
  // Exact bit count, not len * 8: rounding up would let a non-byte-aligned
  // modulus pass the gate and fail inside aws-lc as "invalid signature"
  // instead of taking the fallback path.
  let bits = u32::try_from(modulus.len()).ok()?.checked_mul(8)?
    - modulus.first()?.leading_zeros();
  Some(bits)
}

#[cfg(test)]
mod tests {
  use super::rsa_public_key_modulus_bits;

  // SEQUENCE { INTEGER 0x01ffff (17 bits), INTEGER 65537 }. Pins the exact
  // bit count; len * 8 would report 24.
  #[test]
  fn modulus_bits_exact() {
    let der = [
      0x30, 0x0a, 0x02, 0x03, 0x01, 0xff, 0xff, 0x02, 0x03, 0x01, 0x00, 0x01,
    ];
    assert_eq!(rsa_public_key_modulus_bits(&der), Some(17));
  }

  #[test]
  fn modulus_bits_rejects_garbage() {
    assert_eq!(rsa_public_key_modulus_bits(&[]), None);
    assert_eq!(rsa_public_key_modulus_bits(&[0x30]), None);
    assert_eq!(rsa_public_key_modulus_bits(&[0x04, 0x01, 0x00]), None);
  }
}
