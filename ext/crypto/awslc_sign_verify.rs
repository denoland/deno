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
  use super::*;
  use crate::KeyData;
  use crate::KeyType;
  use crate::SignArg;
  use crate::VerifyArg;
  use crate::shared::EcNamedCurve;

  // See testdata/README.md. The signatures are RSASSA-PKCS1-v1_5 SHA-256
  // over `DATA`.
  const RSA_2048_PRIVATE: &[u8] =
    include_bytes!("testdata/rsa2048_private_pkcs1.der");
  const RSA_1024_PRIVATE: &[u8] =
    include_bytes!("testdata/rsa1024_private_pkcs1.der");
  const RSA_8192_PUBLIC: &[u8] =
    include_bytes!("testdata/rsa8192_public_pkcs1.der");
  const RSA_8192_SIG: &[u8] = include_bytes!("testdata/rsa8192_sig_sha256.bin");
  const RSA_9216_PUBLIC: &[u8] =
    include_bytes!("testdata/rsa9216_public_pkcs1.der");
  const RSA_9216_SIG: &[u8] = include_bytes!("testdata/rsa9216_sig_sha256.bin");

  const DATA: &[u8] = b"deno ext/crypto sign/verify fixture";

  fn private_key(data: &[u8]) -> KeyData {
    KeyData {
      r#type: KeyType::Private,
      data: data.into(),
    }
  }

  fn public_key(data: &[u8]) -> KeyData {
    KeyData {
      r#type: KeyType::Public,
      data: data.into(),
    }
  }

  /// PKCS#1 `RSAPublicKey` DER for a PKCS#1 private key, the shape
  /// `import_key` stores for RSA public keys.
  fn rsa_public_pkcs1(private_pkcs1: &[u8]) -> Vec<u8> {
    use rsa::pkcs1::DecodeRsaPrivateKey as _;
    use rsa::pkcs1::EncodeRsaPublicKey as _;
    rsa::RsaPrivateKey::from_pkcs1_der(private_pkcs1)
      .unwrap()
      .to_public_key()
      .to_pkcs1_der()
      .unwrap()
      .as_bytes()
      .to_vec()
  }

  // Every intended fast-path combination must actually reach aws-lc: a
  // `None` here silently degrades to the RustCrypto path with no test
  // failure anywhere else. Keys are produced by the production keygen
  // (`generate_ec`), which uses aws-lc for P-256/P-384 but RustCrypto
  // PKCS#8 for P-521, so the P-521 case also pins that
  // `EcdsaKeyPair::from_pkcs8` keeps accepting that foreign encoding.
  #[test]
  fn ecdsa_fast_path_reachable() {
    use elliptic_curve::sec1::ToEncodedPoint as _;
    use p256::pkcs8::DecodePrivateKey as _;

    let cases = [
      (
        "P-256+SHA-256",
        EcNamedCurve::P256,
        CryptoNamedCurve::P256,
        CryptoHash::Sha256,
      ),
      (
        "P-384+SHA-384",
        EcNamedCurve::P384,
        CryptoNamedCurve::P384,
        CryptoHash::Sha384,
      ),
      (
        "P-521+SHA-512",
        EcNamedCurve::P521,
        CryptoNamedCurve::P521,
        CryptoHash::Sha512,
      ),
    ];
    for (label, gen_curve, curve, hash) in cases {
      let pkcs8 = crate::generate_key::generate_ec(gen_curve).unwrap();
      // Raw SEC1 point, the shape imported public keys store.
      let sec1 = match curve {
        CryptoNamedCurve::P256 => p256::SecretKey::from_pkcs8_der(&pkcs8)
          .unwrap()
          .public_key()
          .to_encoded_point(false)
          .as_bytes()
          .to_vec(),
        CryptoNamedCurve::P384 => p384::SecretKey::from_pkcs8_der(&pkcs8)
          .unwrap()
          .public_key()
          .to_encoded_point(false)
          .as_bytes()
          .to_vec(),
        CryptoNamedCurve::P521 => p521::SecretKey::from_pkcs8_der(&pkcs8)
          .unwrap()
          .public_key()
          .to_encoded_point(false)
          .as_bytes()
          .to_vec(),
      };

      let sign_args =
        SignArg::new(Algorithm::Ecdsa, None, Some(hash), Some(curve));
      let sig = try_sign(&private_key(&pkcs8), &sign_args, DATA)
        .unwrap()
        .unwrap_or_else(|| panic!("{label}: sign missed the fast path"));

      let verify_args = |sig: Vec<u8>| {
        VerifyArg::new(Algorithm::Ecdsa, None, Some(hash), sig, Some(curve))
      };
      // Private key data, the shape generated key pairs store.
      assert_eq!(
        try_verify(&private_key(&pkcs8), &verify_args(sig.clone()), DATA),
        Some(true),
        "{label}: verify with private key data missed the fast path"
      );
      assert_eq!(
        try_verify(&public_key(&sec1), &verify_args(sig.clone()), DATA),
        Some(true),
        "{label}: verify with SEC1 public key missed the fast path"
      );
      // A committed fast path reports tampering as false, not fallback.
      let mut tampered = sig;
      tampered[0] ^= 0xff;
      assert_eq!(
        try_verify(&public_key(&sec1), &verify_args(tampered), DATA),
        Some(false),
        "{label}: tampered signature"
      );
    }
  }

  #[test]
  fn rsa_fast_path_reachable() {
    let public_der = rsa_public_pkcs1(RSA_2048_PRIVATE);
    let cases = [
      (
        "RSASSA-PKCS1 SHA-256",
        Algorithm::RsassaPkcs1v15,
        CryptoHash::Sha256,
        None,
      ),
      (
        "RSASSA-PKCS1 SHA-384",
        Algorithm::RsassaPkcs1v15,
        CryptoHash::Sha384,
        None,
      ),
      (
        "RSASSA-PKCS1 SHA-512",
        Algorithm::RsassaPkcs1v15,
        CryptoHash::Sha512,
        None,
      ),
      (
        "RSA-PSS SHA-256",
        Algorithm::RsaPss,
        CryptoHash::Sha256,
        Some(32),
      ),
      (
        "RSA-PSS SHA-384",
        Algorithm::RsaPss,
        CryptoHash::Sha384,
        Some(48),
      ),
      (
        "RSA-PSS SHA-512",
        Algorithm::RsaPss,
        CryptoHash::Sha512,
        Some(64),
      ),
    ];
    for (label, algorithm, hash, salt_length) in cases {
      let sign_args = SignArg::new(algorithm, salt_length, Some(hash), None);
      let sig = try_sign(&private_key(RSA_2048_PRIVATE), &sign_args, DATA)
        .unwrap()
        .unwrap_or_else(|| panic!("{label}: sign missed the fast path"));

      let verify_args =
        VerifyArg::new(algorithm, salt_length, Some(hash), sig, None);
      assert_eq!(
        try_verify(&private_key(RSA_2048_PRIVATE), &verify_args, DATA),
        Some(true),
        "{label}: verify with private key data missed the fast path"
      );
      assert_eq!(
        try_verify(&public_key(&public_der), &verify_args, DATA),
        Some(true),
        "{label}: verify with public key missed the fast path"
      );
    }
  }

  // Both ends of the modulus gate: 8192 bits is the last fast-path size,
  // anything past it must return None so the fallback decides.
  #[test]
  fn rsa_modulus_gate_boundaries() {
    assert_eq!(rsa_public_key_modulus_bits(RSA_8192_PUBLIC), Some(8192));
    assert_eq!(rsa_public_key_modulus_bits(RSA_9216_PUBLIC), Some(9216));

    let verify_args = |sig: &[u8]| {
      VerifyArg::new(
        Algorithm::RsassaPkcs1v15,
        None,
        Some(CryptoHash::Sha256),
        sig.to_vec(),
        None,
      )
    };
    assert_eq!(
      try_verify(
        &public_key(RSA_8192_PUBLIC),
        &verify_args(RSA_8192_SIG),
        DATA
      ),
      Some(true)
    );
    assert_eq!(
      try_verify(
        &public_key(RSA_9216_PUBLIC),
        &verify_args(RSA_9216_SIG),
        DATA
      ),
      None
    );
  }

  #[test]
  fn rsa_below_range_falls_back() {
    let public_der = rsa_public_pkcs1(RSA_1024_PRIVATE);
    let sign_args = SignArg::new(
      Algorithm::RsassaPkcs1v15,
      None,
      Some(CryptoHash::Sha256),
      None,
    );
    assert!(
      try_sign(&private_key(RSA_1024_PRIVATE), &sign_args, DATA)
        .unwrap()
        .is_none()
    );
    let verify_args = VerifyArg::new(
      Algorithm::RsassaPkcs1v15,
      None,
      Some(CryptoHash::Sha256),
      vec![0; 128],
      None,
    );
    assert_eq!(
      try_verify(&private_key(RSA_1024_PRIVATE), &verify_args, DATA),
      None
    );
    assert_eq!(
      try_verify(&public_key(&public_der), &verify_args, DATA),
      None
    );
  }

  // `RSA_MODULUS_BITS` mirrors a range aws-lc enforces internally: at key
  // parse for sign, but only at verification time for verify, where a
  // range failure is indistinguishable from a bad signature. If either
  // assertion fails, aws-lc's accepted range changed and the constant
  // (and the `RSA_*_2048_8192_*` algorithm choices) must be revisited.
  #[test]
  fn rsa_modulus_range_matches_awslc() {
    // Sign side: the gate is aws-lc's own parse-time range check.
    assert!(RsaKeyPair::from_der(RSA_1024_PRIVATE).is_err());
    assert!(RsaKeyPair::from_der(RSA_2048_PRIVATE).is_ok());

    // Verify side: a *valid* signature from an out-of-range key is
    // rejected by the 2048-8192 parameters, so without the up-front gate
    // it would surface as "invalid signature" instead of falling back.
    use rsa::pkcs1::DecodeRsaPrivateKey as _;
    use rsa::signature::SignatureEncoding as _;
    use rsa::signature::Signer as _;
    let small_key =
      rsa::RsaPrivateKey::from_pkcs1_der(RSA_1024_PRIVATE).unwrap();
    let small_sig = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(small_key)
      .sign(DATA)
      .to_vec();
    let small_public = rsa_public_pkcs1(RSA_1024_PRIVATE);
    for (public, sig) in [
      (&small_public[..], &small_sig[..]),
      (RSA_9216_PUBLIC, RSA_9216_SIG),
    ] {
      let rejected = match ParsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        public,
      ) {
        Ok(parsed) => parsed.verify_sig(DATA, sig).is_err(),
        Err(_) => true,
      };
      assert!(rejected, "aws-lc accepted an out-of-range modulus");
    }
  }

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
