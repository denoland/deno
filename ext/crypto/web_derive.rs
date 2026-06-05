// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of `SubtleCrypto.deriveBits` (and the key-length
//! computation used by `SubtleCrypto.deriveKey`).
//!
//! This replaces the per-algorithm crypto dispatch + validation that used to
//! live in the `deriveBits()` helper of `00_crypto.js`. The op performs:
//!   - case-insensitive algorithm-name normalization,
//!   - the `InvalidAccessError` key-type / algorithm-name / named-curve
//!     validations,
//!   - the `OperationError` length / iterations / identity-point validations,
//!   - the actual key derivation (PBKDF2, HKDF, ECDH, X25519, X448), and
//!   - the final length slicing.
//!
//! All of those DOMException class strings (`InvalidAccessError`,
//! `OperationError`, `NotSupportedError`) have registered builders in
//! `runtime/js/99_main.js`, so they surface as real `DOMException`s.
//!
//! The crypto compute bodies are deliberately *replicated* from
//! `op_crypto_derive_bits` (lib.rs) and `op_crypto_derive_bits_x25519` /
//! `op_crypto_derive_bits_x448` (x25519.rs / x448.rs), because one op cannot
//! call another and the task forbids editing those files. The parent is
//! expected to dedup this later.
//!
//! `deriveKey` stays as thin orchestration in JS: it composes `getKeyLength`
//! (now `op_crypto_get_key_length`), this op, and `importKey`. Only the
//! `getKeyLength` validation/computation was cleanly portable.

use std::num::NonZeroU32;

use aws_lc_rs::hkdf;
use aws_lc_rs::pbkdf2;
use curve25519_dalek::montgomery::MontgomeryPoint as X25519MontgomeryPoint;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint as X448MontgomeryPoint;
use ed448_goldilocks::subtle::ConstantTimeEq as _;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use serde::Deserialize;

use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::key::HkdfOutput;

const X25519_MONTGOMERY_IDENTITY: X25519MontgomeryPoint =
  X25519MontgomeryPoint([0; 32]);
static X448_MONTGOMERY_IDENTITY: X448MontgomeryPoint =
  X448MontgomeryPoint([0; 56]);

/// The `[[type]]` of a `CryptoKey` (`key[_type]`), passed verbatim from JS.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebKeyType {
  Secret,
  Private,
  Public,
}

/// Raw key material for the base / public key, mirroring the JS `KeyData`
/// object stored in `KEY_STORE` for ECDH (`{ type, data }`), or the raw bytes
/// for X25519/X448/PBKDF2/HKDF.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct DeriveKeyData {
  r#type: WebKeyType,
  #[serde(with = "serde_bytes")]
  data: Vec<u8>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebDeriveError {
  #[class(inherit)]
  #[error(transparent)]
  JoinError(
    #[from]
    #[inherit]
    tokio::task::JoinError,
  ),
  // ---- InvalidAccessError (registered DOMException) -----------------------
  #[class("DOMExceptionInvalidAccessError")]
  #[error("Invalid key type")]
  InvalidKeyType,
  #[class("DOMExceptionInvalidAccessError")]
  #[error("Algorithm mismatch")]
  AlgorithmMismatch,
  #[class("DOMExceptionInvalidAccessError")]
  #[error("'namedCurve' mismatch")]
  NamedCurveMismatch,
  // ---- OperationError (registered DOMException) ---------------------------
  #[class("DOMExceptionOperationError")]
  #[error("Invalid length")]
  InvalidLength,
  #[class("DOMExceptionOperationError")]
  #[error("iterations must not be zero")]
  ZeroIterations,
  #[class("DOMExceptionOperationError")]
  #[error("Invalid key")]
  InvalidKey,
  #[class("DOMExceptionOperationError")]
  #[error("The length provided for HKDF is too large")]
  HkdfLengthTooLarge,
  // ---- NotSupportedError (registered DOMException) ------------------------
  #[class("DOMExceptionNotSupportedError")]
  #[error("Not implemented")]
  NotImplemented,
  // ---- plain errors -------------------------------------------------------
  #[class(type)]
  #[error("Missing argument hash")]
  MissingArgumentHash,
  #[class(type)]
  #[error("Missing argument info")]
  MissingArgumentInfo,
  #[class(type)]
  #[error("Missing argument iterations")]
  MissingArgumentIterations,
  #[class(type)]
  #[error("Missing argument salt")]
  MissingArgumentSalt,
  #[class(type)]
  #[error("Missing public key")]
  MissingPublicKey,
  #[class(type)]
  #[error("Unexpected error decoding private key")]
  DecodePrivateKey,
  #[class(type)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class(generic)]
  #[error(transparent)]
  Unspecified(#[from] aws_lc_rs::error::Unspecified),
}

/// Canonicalize a `deriveBits` algorithm name case-insensitively against the
/// `supportedAlgorithms["deriveBits"]` registry. The JS `normalizeAlgorithm`
/// has already done this; we re-normalize defensively so dispatch is
/// self-contained.
fn canonical_derive_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &["PBKDF2", "ECDH", "HKDF", "X25519", "X448"];
  NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

/// Apply the WebCrypto "if length is null return all bits, else require
/// `byte_len * 8 >= length` and slice to `ceil(length / 8)`" rule shared by
/// ECDH / X25519 / X448.
fn slice_to_length(
  secret: Vec<u8>,
  length: Option<u32>,
) -> Result<Vec<u8>, WebDeriveError> {
  match length {
    None => Ok(secret),
    Some(length) => {
      if (secret.len() as u64) * 8 < length as u64 {
        return Err(WebDeriveError::InvalidLength);
      }
      let bytes = length.div_ceil(8) as usize;
      Ok(secret[..bytes].to_vec())
    }
  }
}

/// The base key fields, grouped so the op stays under the async-op codegen
/// argument limit (9). Converted via `FromV8` (NOT `#[serde]`) per repo
/// guidance; the individual fields fall back to serde only where the existing
/// `Deserialize` types are reused (`DeriveKeyData`, `CryptoNamedCurve`).
///
/// `FromV8` derive camelCases field names by default and treats `Option<_>`
/// fields as optional, so the JS object keys are `baseKey` / `baseKeyAlgorithm`
/// / `baseNamedCurve`.
/// `SubtleCrypto.deriveBits` crypto compute. The public JS method has already
/// done the webidl conversions and the post-condition algorithm-name / usages
/// check. This op performs the inner per-algorithm validation + derivation
/// that used to be in the `deriveBits()` helper.
///
/// Parameters:
///   - `algorithm`: `normalizedAlgorithm.name`.
///   - `base`: the base key material + its algorithm name / named curve.
///   - `public`: the public key material + its type / algorithm name / named
///     curve (ECDH/X25519/X448).
///   - `hash`: `normalizedAlgorithm.hash.name` (PBKDF2/HKDF).
///   - `iterations`: `normalizedAlgorithm.iterations` (PBKDF2).
///   - `length`: the requested bit length, or `null`.
///   - `salt`: `normalizedAlgorithm.salt` (PBKDF2/HKDF).
///   - `info`: `normalizedAlgorithm.info` (HKDF).
///
/// Owned base-key fields for `derive_bits_compute`.
pub(crate) struct DeriveBase {
  pub key_type: WebKeyType,
  pub data: Vec<u8>,
  pub algorithm: String,
  pub named_curve: Option<CryptoNamedCurve>,
}

/// Owned public-key fields for `derive_bits_compute` (ECDH/X25519/X448).
pub(crate) struct DerivePublic {
  /// Inner `{ type, data }.type` used to shape `DeriveKeyData`.
  pub data_type: Option<WebKeyType>,
  /// Outer cppgc key type (`publicKey.type`) validated by the op.
  pub key_type: Option<WebKeyType>,
  pub data: Option<Vec<u8>>,
  pub algorithm: Option<String>,
  pub named_curve: Option<CryptoNamedCurve>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn derive_bits_compute(
  algorithm: String,
  base: DeriveBase,
  public: DerivePublic,
  hash: Option<CryptoHash>,
  iterations: Option<u32>,
  length: Option<u32>,
  salt: Option<Vec<u8>>,
  info: Option<Vec<u8>>,
) -> Result<Vec<u8>, WebDeriveError> {
  let base_key = DeriveKeyData {
    r#type: base.key_type,
    data: base.data,
  };
  let base_key_algorithm = base.algorithm;
  let base_named_curve = base.named_curve;
  let public_key = public.data.map(|data| DeriveKeyData {
    r#type: public.data_type.unwrap_or(WebKeyType::Public),
    data,
  });
  let public_key_type = public.key_type;
  let public_key_algorithm = public.algorithm;
  let public_named_curve = public.named_curve;
  let name =
    canonical_derive_name(&algorithm).ok_or(WebDeriveError::NotImplemented)?;

  match name {
    "PBKDF2" => {
      // length: must be non-null, non-zero, multiple of 8.
      let length = match length {
        Some(l) if l != 0 && l.is_multiple_of(8) => l,
        _ => return Err(WebDeriveError::InvalidLength),
      };
      let iterations =
        iterations.ok_or(WebDeriveError::MissingArgumentIterations)?;
      if iterations == 0 {
        return Err(WebDeriveError::ZeroIterations);
      }
      let salt = salt.ok_or(WebDeriveError::MissingArgumentSalt)?;
      let hash = hash.ok_or(WebDeriveError::MissingArgumentHash)?;
      let secret = base_key.data;
      let out = spawn_blocking(move || {
        let algorithm = match hash {
          CryptoHash::Sha1 => pbkdf2::PBKDF2_HMAC_SHA1,
          CryptoHash::Sha256 => pbkdf2::PBKDF2_HMAC_SHA256,
          CryptoHash::Sha384 => pbkdf2::PBKDF2_HMAC_SHA384,
          CryptoHash::Sha512 => pbkdf2::PBKDF2_HMAC_SHA512,
          _ => return Err(WebDeriveError::NotImplemented),
        };
        // Safe: checked non-zero above.
        let iterations = NonZeroU32::new(iterations).unwrap();
        let mut out = vec![0u8; length as usize / 8];
        pbkdf2::derive(algorithm, iterations, &salt, &secret, &mut out);
        Ok::<Vec<u8>, WebDeriveError>(out)
      })
      .await??;
      Ok(out)
    }
    "HKDF" => {
      let length = match length {
        Some(l) if l != 0 && l.is_multiple_of(8) => l,
        _ => return Err(WebDeriveError::InvalidLength),
      };
      let salt = salt.ok_or(WebDeriveError::MissingArgumentSalt)?;
      let info = info.ok_or(WebDeriveError::MissingArgumentInfo)?;
      let hash = hash.ok_or(WebDeriveError::MissingArgumentHash)?;
      let secret = base_key.data;
      let out = spawn_blocking(move || {
        let algorithm = match hash {
          CryptoHash::Sha1 => hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY,
          CryptoHash::Sha256 => hkdf::HKDF_SHA256,
          CryptoHash::Sha384 => hkdf::HKDF_SHA384,
          CryptoHash::Sha512 => hkdf::HKDF_SHA512,
          _ => return Err(WebDeriveError::NotImplemented),
        };
        let length = length as usize / 8;
        let salt = hkdf::Salt::new(algorithm, &salt);
        let prk = salt.extract(&secret);
        let info: &[&[u8]] = &[&info];
        let okm = prk
          .expand(info, HkdfOutput(length))
          .map_err(|_| WebDeriveError::HkdfLengthTooLarge)?;
        let mut r = vec![0u8; length];
        okm.fill(&mut r)?;
        Ok::<Vec<u8>, WebDeriveError>(r)
      })
      .await??;
      Ok(out)
    }
    "ECDH" => {
      // 1. base key must be private.
      if base_key.r#type != WebKeyType::Private {
        return Err(WebDeriveError::InvalidKeyType);
      }
      let public_key = public_key.ok_or(WebDeriveError::MissingPublicKey)?;
      // 3. public key must be public.
      if public_key_type != Some(WebKeyType::Public) {
        return Err(WebDeriveError::InvalidKeyType);
      }
      // 4. algorithm names must match.
      if public_key_algorithm.as_deref() != Some(base_key_algorithm.as_str()) {
        return Err(WebDeriveError::AlgorithmMismatch);
      }
      // 5. named curves must match.
      let base_named_curve =
        base_named_curve.ok_or(WebDeriveError::NamedCurveMismatch)?;
      let public_named_curve =
        public_named_curve.ok_or(WebDeriveError::NamedCurveMismatch)?;
      if !named_curves_eq(base_named_curve, public_named_curve) {
        return Err(WebDeriveError::NamedCurveMismatch);
      }
      // 6. named curve must be supported (P-256/384/521 always are here).
      let secret = spawn_blocking(move || {
        ecdh_derive(base_named_curve, &base_key.data, public_key)
      })
      .await??;
      // 7-8. length slicing.
      let out = slice_to_length(secret, length)?;
      Ok(out)
    }
    "X25519" => {
      if base_key.r#type != WebKeyType::Private {
        return Err(WebDeriveError::InvalidKeyType);
      }
      let public_key = public_key.ok_or(WebDeriveError::MissingPublicKey)?;
      if public_key_type != Some(WebKeyType::Public) {
        return Err(WebDeriveError::InvalidKeyType);
      }
      if public_key_algorithm.as_deref() != Some(base_key_algorithm.as_str()) {
        return Err(WebDeriveError::AlgorithmMismatch);
      }
      let k: [u8; 32] = base_key
        .data
        .as_slice()
        .try_into()
        .map_err(|_| WebDeriveError::InvalidKeyLength)?;
      let u: [u8; 32] = public_key
        .data
        .as_slice()
        .try_into()
        .map_err(|_| WebDeriveError::InvalidKeyLength)?;
      let sh_sec = x25519_dalek::x25519(k, u);
      let point = X25519MontgomeryPoint(sh_sec);
      if point.ct_eq(&X25519_MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
        return Err(WebDeriveError::InvalidKey);
      }
      let out = slice_to_length(sh_sec.to_vec(), length)?;
      Ok(out)
    }
    "X448" => {
      if base_key.r#type != WebKeyType::Private {
        return Err(WebDeriveError::InvalidKeyType);
      }
      let public_key = public_key.ok_or(WebDeriveError::MissingPublicKey)?;
      if public_key_type != Some(WebKeyType::Public) {
        return Err(WebDeriveError::InvalidKeyType);
      }
      if public_key_algorithm.as_deref() != Some(base_key_algorithm.as_str()) {
        return Err(WebDeriveError::AlgorithmMismatch);
      }
      let k: [u8; 56] = base_key
        .data
        .as_slice()
        .try_into()
        .map_err(|_| WebDeriveError::InvalidKeyLength)?;
      let u: [u8; 56] = public_key
        .data
        .as_slice()
        .try_into()
        .map_err(|_| WebDeriveError::InvalidKeyLength)?;
      let mut scalar_bytes = [0u8; 57];
      scalar_bytes[..56].copy_from_slice(&k);
      let scalar = EdwardsScalar::from_bytes_mod_order(&scalar_bytes.into());
      let point = &X448MontgomeryPoint(u) * &scalar;
      if point.ct_eq(&X448_MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
        return Err(WebDeriveError::InvalidKey);
      }
      let out = slice_to_length(point.0.to_vec(), length)?;
      Ok(out)
    }
    _ => Err(WebDeriveError::NotImplemented),
  }
}

fn named_curves_eq(a: CryptoNamedCurve, b: CryptoNamedCurve) -> bool {
  use CryptoNamedCurve::*;
  matches!((a, b), (P256, P256) | (P384, P384) | (P521, P521))
}

/// ECDH P-256/384/521 shared-secret derivation, replicated from
/// `op_crypto_derive_bits` in lib.rs. Returns the raw x-coordinate.
fn ecdh_derive(
  named_curve: CryptoNamedCurve,
  base_key_data: &[u8],
  public_key: DeriveKeyData,
) -> Result<Vec<u8>, WebDeriveError> {
  match named_curve {
    CryptoNamedCurve::P256 => {
      let secret_key = p256::SecretKey::from_pkcs8_der(base_key_data)
        .map_err(|_| WebDeriveError::DecodePrivateKey)?;
      let public_key = match public_key.r#type {
        WebKeyType::Private => {
          p256::SecretKey::from_pkcs8_der(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?
            .public_key()
        }
        WebKeyType::Public => {
          let point = p256::EncodedPoint::from_bytes(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?;
          let pk = p256::PublicKey::from_encoded_point(&point);
          if pk.is_some().into() {
            pk.unwrap()
          } else {
            return Err(WebDeriveError::DecodePrivateKey);
          }
        }
        WebKeyType::Secret => return Err(WebDeriveError::DecodePrivateKey),
      };
      let shared_secret = p256::elliptic_curve::ecdh::diffie_hellman(
        secret_key.to_nonzero_scalar(),
        public_key.as_affine(),
      );
      Ok(shared_secret.raw_secret_bytes().to_vec())
    }
    CryptoNamedCurve::P384 => {
      let secret_key = p384::SecretKey::from_pkcs8_der(base_key_data)
        .map_err(|_| WebDeriveError::DecodePrivateKey)?;
      let public_key = match public_key.r#type {
        WebKeyType::Private => {
          p384::SecretKey::from_pkcs8_der(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?
            .public_key()
        }
        WebKeyType::Public => {
          let point = p384::EncodedPoint::from_bytes(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?;
          let pk = p384::PublicKey::from_encoded_point(&point);
          if pk.is_some().into() {
            pk.unwrap()
          } else {
            return Err(WebDeriveError::DecodePrivateKey);
          }
        }
        WebKeyType::Secret => return Err(WebDeriveError::DecodePrivateKey),
      };
      let shared_secret = p384::elliptic_curve::ecdh::diffie_hellman(
        secret_key.to_nonzero_scalar(),
        public_key.as_affine(),
      );
      Ok(shared_secret.raw_secret_bytes().to_vec())
    }
    CryptoNamedCurve::P521 => {
      let secret_key = p521::SecretKey::from_pkcs8_der(base_key_data)
        .map_err(|_| WebDeriveError::DecodePrivateKey)?;
      let public_key = match public_key.r#type {
        WebKeyType::Private => {
          p521::SecretKey::from_pkcs8_der(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?
            .public_key()
        }
        WebKeyType::Public => {
          let point = p521::EncodedPoint::from_bytes(&public_key.data)
            .map_err(|_| WebDeriveError::DecodePrivateKey)?;
          let pk = p521::PublicKey::from_encoded_point(&point);
          if pk.is_some().into() {
            pk.unwrap()
          } else {
            return Err(WebDeriveError::DecodePrivateKey);
          }
        }
        WebKeyType::Secret => return Err(WebDeriveError::DecodePrivateKey),
      };
      let shared_secret = p521::elliptic_curve::ecdh::diffie_hellman(
        secret_key.to_nonzero_scalar(),
        public_key.as_affine(),
      );
      Ok(shared_secret.raw_secret_bytes().to_vec())
    }
  }
}

// ---------------------------------------------------------------------------
// get key length (used by deriveKey orchestration in JS)
// ---------------------------------------------------------------------------

/// A `derivedKeyType` normalized for the "get key length" operation. Mirrors
/// the `getKeyLength` switch in `00_crypto.js`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetKeyLengthArg {
  name: String,
  length: Option<u32>,
  #[serde(default)]
  hash: Option<HashName>,
}

#[derive(Deserialize)]
pub struct HashName {
  name: String,
}

/// The result of `getKeyLength`: either a concrete bit length, or `null`
/// (HKDF/PBKDF2 derive all the bits the algorithm produces).
#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum KeyLength {
  Length(u32),
  Null(Option<()>),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum GetKeyLengthError {
  #[class("DOMExceptionOperationError")]
  #[error("Length must be 128, 192, or 256: received {0:?}")]
  InvalidAesLength(Option<u32>),
  #[class("DOMExceptionNotSupportedError")]
  #[error("Unrecognized hash algorithm: {0}")]
  UnrecognizedHash(String),
  #[class(type)]
  #[error("Invalid length: 0")]
  InvalidHmacLength,
  #[class(type)]
  #[error("Unreachable")]
  Unreachable,
}

/// `getKeyLength(normalizedDerivedKeyAlgorithmLength)` from `00_crypto.js`,
/// ported faithfully. Returns the derived-key bit length (or null).
#[op2]
#[serde]
pub fn op_crypto_get_key_length(
  #[serde] algorithm: GetKeyLengthArg,
) -> Result<KeyLength, GetKeyLengthError> {
  // Match the JS switch on the (already-normalized) algorithm name. The names
  // arrive canonical from `normalizeAlgorithm`, but compare case-insensitively
  // to stay self-contained.
  let name = &algorithm.name;
  let eq = |n: &str| name.eq_ignore_ascii_case(n);

  if eq("AES-CBC")
    || eq("AES-CTR")
    || eq("AES-GCM")
    || eq("AES-OCB")
    || eq("AES-KW")
  {
    match algorithm.length {
      Some(128) | Some(192) | Some(256) => {
        Ok(KeyLength::Length(algorithm.length.unwrap()))
      }
      other => Err(GetKeyLengthError::InvalidAesLength(other)),
    }
  } else if eq("HMAC") {
    match algorithm.length {
      None => {
        let hash = algorithm.hash.ok_or(GetKeyLengthError::Unreachable)?;
        let len = match hash.name.as_str() {
          "SHA-1" => 512,
          "SHA-256" => 512,
          "SHA-384" => 1024,
          "SHA-512" => 1024,
          "SHA3-256" => 512,
          "SHA3-384" => 1024,
          "SHA3-512" => 1024,
          _ => return Err(GetKeyLengthError::UnrecognizedHash(hash.name)),
        };
        Ok(KeyLength::Length(len))
      }
      Some(0) => Err(GetKeyLengthError::InvalidHmacLength),
      Some(length) => Ok(KeyLength::Length(length)),
    }
  } else if eq("ChaCha20-Poly1305") {
    // ChaCha20-Poly1305 keys are always 256 bits.
    Ok(KeyLength::Length(256))
  } else if eq("HKDF") || eq("PBKDF2") {
    Ok(KeyLength::Null(None))
  } else {
    Err(GetKeyLengthError::Unreachable)
  }
}
