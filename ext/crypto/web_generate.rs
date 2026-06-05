// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of `SubtleCrypto.generateKey`.
//!
//! This ports the per-algorithm key-usage validation and the actual key
//! GENERATION that used to live in the `generateKey()` helper in
//! `ext/crypto/00_crypto.js` into a single Rust op. The thin JS stub is
//! responsible only for webidl argument conversion + `normalizeAlgorithm` (the
//! webidl dictionary member coercion), then hands the normalized algorithm
//! fields here. The `CryptoKey` / `CryptoKeyPair` object construction
//! (`constructKey(...)`) and the `KEY_STORE` WeakMap wiring STAY in JS, because
//! `CryptoKey` is a JS class.
//!
//! The op returns the raw key material (and, for ML-DSA, the seed) tagged by
//! the algorithm "kind" so the JS stub knows which `constructKey(...)` shape to
//! build.
//!
//! The crypto compute bodies are reused from `generate_key.rs` (RSA / EC / AES
//! / HMAC) via `pub fn`s added there, and replicated for the curve / PQ
//! algorithms whose ops (`op_crypto_generate_ed25519_keypair`,
//! `op_crypto_generate_x25519_keypair`, `op_crypto_generate_x448_keypair`,
//! `op_crypto_ml_kem_generate_key`, `op_crypto_mldsa_from_seed`) cannot be
//! called from another op. The parent is expected to dedup this later.

use aws_lc_rs::signature::Ed25519KeyPair;
use aws_lc_rs::signature::KeyPair;
use aws_lc_rs::unstable::signature::ML_DSA_44_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_65_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_87_SIGNING;
use aws_lc_rs::unstable::signature::PqdsaKeyPair;
use aws_lc_rs::unstable::signature::PqdsaSigningAlgorithm;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint as Ed448MontgomeryPoint;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Deserialize;

use crate::generate_key::GenerateKeyError;
use crate::generate_key::generate_key_aes;
use crate::generate_key::generate_key_ec;
use crate::generate_key::generate_key_hmac;
use crate::generate_key::generate_key_rsa;
use crate::shared::EcNamedCurve;
use crate::shared::ShaHash;

// u-coordinate of the X25519 base point.
const X25519_BASEPOINT_BYTES: [u8; 32] = [
  9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0,
];

/// Errors thrown by `generateKey` before/while generating, mapped to the SAME
/// `DOMException` types thrown by the original JS. Each of these class strings
/// has a registered error builder in `runtime/js/99_main.js`
/// (`DOMExceptionSyntaxError`, `DOMExceptionOperationError`,
/// `DOMExceptionNotSupportedError`), so they surface as real `DOMException`s.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebGenerateError {
  // "Invalid key usage" -> SyntaxError
  #[class("DOMExceptionSyntaxError")]
  #[error("Invalid key usage")]
  InvalidKeyUsage,
  // ECDSA / ECDH: "Curve not supported" -> NotSupportedError
  #[class("DOMExceptionNotSupportedError")]
  #[error("Curve not supported")]
  CurveNotSupported,
  // HMAC zero length -> OperationError ("Invalid length")
  #[class("DOMExceptionOperationError")]
  #[error("Invalid length")]
  InvalidLength,
  // AES invalid length -> OperationError ("Invalid key length: {len}")
  #[class("DOMExceptionOperationError")]
  #[error("Invalid key length: {0}")]
  InvalidAesKeyLength(i64),
  // Ed25519 generation failure -> OperationError ("Failed to generate key")
  #[class("DOMExceptionOperationError")]
  #[error("Failed to generate key")]
  FailedToGenerateKey,
  // Unknown ML-DSA variant (should be unreachable after canonicalization).
  #[class("DOMExceptionDataError")]
  #[error("Unknown ML-DSA variant")]
  UnknownMlDsaVariant,
  // The underlying crypto compute (RSA / EC / AES / HMAC) surfaces
  // `GenerateKeyError`, all of whose variants are `DOMExceptionOperationError`.
  #[class(inherit)]
  #[error(transparent)]
  Generate(
    #[from]
    #[inherit]
    GenerateKeyError,
  ),
  // ML-KEM / ML-DSA generation failures.
  #[class("DOMExceptionOperationError")]
  #[error("Key generation failed")]
  PqGenerationFailed,
}

/// The normalized algorithm fields handed from the JS stub. Optional fields are
/// only present for the algorithms that use them (post `normalizeAlgorithm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebGenerateArg {
  /// `normalizedAlgorithm.name` (already canonical-cased by webidl
  /// `normalizeAlgorithm`).
  name: String,
  /// The requested key usages.
  usages: Vec<String>,
  /// RSA: `normalizedAlgorithm.modulusLength`.
  modulus_length: Option<u32>,
  /// RSA: `normalizedAlgorithm.publicExponent`.
  #[serde(with = "serde_bytes", default)]
  public_exponent: Option<Vec<u8>>,
  /// EC (ECDSA / ECDH): `normalizedAlgorithm.namedCurve` (string form, so an
  /// unsupported curve can produce the right `NotSupportedError`).
  named_curve: Option<String>,
  /// AES: `normalizedAlgorithm.length` (bits). `i64` so we can faithfully
  /// reproduce the `Invalid key length: <value>` message for any value.
  length: Option<i64>,
  /// HMAC: `normalizedAlgorithm.hash.name`.
  hash: Option<ShaHash>,
}

/// The raw key material returned to the JS stub, tagged so JS knows which
/// `constructKey(...)` shape to build. The JS stub never re-derives anything;
/// it just stores these bytes in `KEY_STORE` and constructs `CryptoKey`s.
///
/// Returned via `#[derive(ToV8)]` with `Uint8Array` fields so the raw key
/// material crosses the boundary as real typed arrays (this is the same
/// pattern used by `ImportKeyResult` / `MlKemKeyPair`). An internally-tagged
/// `#[serde]` enum would break the zero-copy buffer magic and hand JS empty
/// buffers, so we avoid `#[serde]` here. The JS stub dispatches on the
/// algorithm name (not a `kind` tag) and reads only the fields it needs.
#[derive(ToV8)]
#[to_v8(untagged)]
pub enum WebGenerateResult {
  /// A single private key blob (RSA: PKCS#1 DER, used for both public & private
  /// CryptoKeys which share the same handle).
  Private { data: Uint8Array },
  /// A single secret key blob (AES / HMAC / ChaCha20-Poly1305).
  Secret { data: Uint8Array },
  /// A public/private key pair stored as separate handles (curves / PQ).
  KeyPair {
    public_key: Uint8Array,
    private_key: Uint8Array,
  },
  /// ML-DSA: the private handle stores `{ seed, privateKey }`, the public
  /// handle stores the public key bytes.
  MlDsa {
    seed: Uint8Array,
    private_key: Uint8Array,
    public_key: Uint8Array,
  },
  /// ML-KEM: the private handle stores `{ seed, privateKey }` (the FIPS 203
  /// 64-byte seed and the expanded decapsulation key), the public handle
  /// stores the encapsulation key bytes.
  MlKem {
    seed: Uint8Array,
    private_key: Uint8Array,
    public_key: Uint8Array,
  },
}

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

fn ec_named_curve(name: &str) -> Result<EcNamedCurve, WebGenerateError> {
  match name {
    "P-256" => Ok(EcNamedCurve::P256),
    "P-384" => Ok(EcNamedCurve::P384),
    "P-521" => Ok(EcNamedCurve::P521),
    _ => Err(WebGenerateError::CurveNotSupported),
  }
}

/// Mirrors the JS usage-validation: every requested usage must be in `allowed`,
/// else `SyntaxError`.
fn check_usages(
  usages: &[String],
  allowed: &[&str],
) -> Result<(), WebGenerateError> {
  if usages.iter().any(|u| !allowed.contains(&u.as_str())) {
    return Err(WebGenerateError::InvalidKeyUsage);
  }
  Ok(())
}

#[op2]
pub async fn op_crypto_generate_key_web(
  #[serde] arg: WebGenerateArg,
) -> Result<WebGenerateResult, WebGenerateError> {
  let name = arg.name.as_str();

  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => {
      check_usages(&arg.usages, &["sign", "verify"])?;
      generate_rsa(arg.modulus_length, arg.public_exponent).await
    }
    "RSA-OAEP" => {
      check_usages(
        &arg.usages,
        &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
      )?;
      generate_rsa(arg.modulus_length, arg.public_exponent).await
    }
    "ECDSA" => {
      check_usages(&arg.usages, &["sign", "verify"])?;
      generate_ec(arg.named_curve.as_deref()).await
    }
    "ECDH" => {
      check_usages(&arg.usages, &["deriveKey", "deriveBits"])?;
      generate_ec(arg.named_curve.as_deref()).await
    }
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" => {
      check_usages(
        &arg.usages,
        &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
      )?;
      generate_aes(arg.length).await
    }
    "AES-KW" => {
      check_usages(&arg.usages, &["wrapKey", "unwrapKey"])?;
      generate_aes(arg.length).await
    }
    "ChaCha20-Poly1305" => {
      check_usages(
        &arg.usages,
        &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
      )?;
      // ChaCha20-Poly1305 keys are always 256 bits.
      let data = spawn_blocking(|| generate_key_aes(256)).await.unwrap()?;
      Ok(WebGenerateResult::Secret { data: data.into() })
    }
    "HMAC" => {
      check_usages(&arg.usages, &["sign", "verify"])?;
      // Mirrors the JS length handling: `undefined` -> default, `0` ->
      // OperationError, otherwise the provided length.
      let length = match arg.length {
        None => None,
        Some(0) => return Err(WebGenerateError::InvalidLength),
        Some(l) => Some(l as usize),
      };
      let hash = arg.hash.expect("HMAC requires hash");
      let data = spawn_blocking(move || generate_key_hmac(hash, length))
        .await
        .unwrap()?;
      Ok(WebGenerateResult::Secret { data: data.into() })
    }
    "X25519" => {
      check_usages(&arg.usages, &["deriveKey", "deriveBits"])?;
      Ok(generate_x25519())
    }
    "X448" => {
      check_usages(&arg.usages, &["deriveKey", "deriveBits"])?;
      Ok(generate_x448())
    }
    "Ed25519" => {
      check_usages(&arg.usages, &["sign", "verify"])?;
      generate_ed25519()
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      check_usages(&arg.usages, &["sign", "verify"])?;
      generate_mldsa(name)
    }
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      check_usages(
        &arg.usages,
        &[
          "encapsulateKey",
          "encapsulateBits",
          "decapsulateKey",
          "decapsulateBits",
        ],
      )?;
      generate_mlkem(name)
    }
    // Unreachable: webidl `normalizeAlgorithm` only dispatches here for the
    // algorithms registered in `supportedAlgorithms["generateKey"]`.
    _ => Err(WebGenerateError::Generate(
      GenerateKeyError::UnsupportedAlgorithm,
    )),
  }
}

async fn generate_rsa(
  modulus_length: Option<u32>,
  public_exponent: Option<Vec<u8>>,
) -> Result<WebGenerateResult, WebGenerateError> {
  let modulus_length = modulus_length.unwrap_or(0);
  let public_exponent = public_exponent.unwrap_or_default();
  let data =
    spawn_blocking(move || generate_key_rsa(modulus_length, &public_exponent))
      .await
      .unwrap()?;
  Ok(WebGenerateResult::Private { data: data.into() })
}

async fn generate_ec(
  named_curve: Option<&str>,
) -> Result<WebGenerateResult, WebGenerateError> {
  // Reproduce the JS `supportedNamedCurves` check (NotSupportedError on miss).
  let name = named_curve.unwrap_or("");
  if !SUPPORTED_NAMED_CURVES.contains(&name) {
    return Err(WebGenerateError::CurveNotSupported);
  }
  let curve = ec_named_curve(name)?;
  let data = spawn_blocking(move || generate_key_ec(curve))
    .await
    .unwrap()?;
  Ok(WebGenerateResult::Private { data: data.into() })
}

async fn generate_aes(
  length: Option<i64>,
) -> Result<WebGenerateResult, WebGenerateError> {
  // JS `generateKeyAES`: length must be one of 128/192/256, else OperationError
  // with the raw provided value in the message.
  let length = length.unwrap_or(-1);
  if length != 128 && length != 192 && length != 256 {
    return Err(WebGenerateError::InvalidAesKeyLength(length));
  }
  let length = length as usize;
  let data = spawn_blocking(move || generate_key_aes(length))
    .await
    .unwrap()?;
  Ok(WebGenerateResult::Secret { data: data.into() })
}

/// X25519 keypair (replicated from `op_crypto_generate_x25519_keypair`).
fn generate_x25519() -> WebGenerateResult {
  let mut rng = OsRng;
  let mut pkey = [0u8; 32];
  rng.fill_bytes(&mut pkey);
  let pubkey = x25519_dalek::x25519(pkey, X25519_BASEPOINT_BYTES);
  WebGenerateResult::KeyPair {
    private_key: pkey.to_vec().into(),
    public_key: pubkey.to_vec().into(),
  }
}

/// X448 keypair (replicated from `op_crypto_generate_x448_keypair`).
fn generate_x448() -> WebGenerateResult {
  let mut rng = OsRng;
  let mut pkey = [0u8; 56];
  rng.fill_bytes(&mut pkey);

  let mut scalar_bytes = [0u8; 57];
  scalar_bytes[..56].copy_from_slice(&pkey);
  let scalar = EdwardsScalar::from_bytes_mod_order(&scalar_bytes.into());
  let point = &Ed448MontgomeryPoint::GENERATOR * &scalar;
  WebGenerateResult::KeyPair {
    private_key: pkey.to_vec().into(),
    public_key: point.0.to_vec().into(),
  }
}

/// Ed25519 keypair (replicated from `op_crypto_generate_ed25519_keypair`).
fn generate_ed25519() -> Result<WebGenerateResult, WebGenerateError> {
  let mut rng = OsRng;
  let mut pkey = [0u8; 32];
  rng.fill_bytes(&mut pkey);
  let pair = Ed25519KeyPair::from_seed_unchecked(&pkey)
    .map_err(|_| WebGenerateError::FailedToGenerateKey)?;
  let pubkey = pair.public_key().as_ref().to_vec();
  Ok(WebGenerateResult::KeyPair {
    private_key: pkey.to_vec().into(),
    public_key: pubkey.into(),
  })
}

fn mldsa_signing(
  name: &str,
) -> Result<&'static PqdsaSigningAlgorithm, WebGenerateError> {
  match name {
    "ML-DSA-44" => Ok(&ML_DSA_44_SIGNING),
    "ML-DSA-65" => Ok(&ML_DSA_65_SIGNING),
    "ML-DSA-87" => Ok(&ML_DSA_87_SIGNING),
    _ => Err(WebGenerateError::UnknownMlDsaVariant),
  }
}

/// ML-DSA keypair from a fresh random 32-byte seed (replicated from the JS
/// `op_crypto_get_random_values` + `op_crypto_mldsa_from_seed` sequence). The
/// private handle keeps both seed and private key bytes.
fn generate_mldsa(name: &str) -> Result<WebGenerateResult, WebGenerateError> {
  let signing = mldsa_signing(name)?;
  let mut seed = [0u8; 32];
  OsRng.fill_bytes(&mut seed);
  use aws_lc_rs::encoding::AsRawBytes;
  let key_pair = PqdsaKeyPair::from_seed(signing, &seed)
    .map_err(|_| WebGenerateError::PqGenerationFailed)?;
  let private_key = key_pair
    .private_key()
    .as_raw_bytes()
    .map_err(|_| WebGenerateError::PqGenerationFailed)?
    .as_ref()
    .to_vec();
  let public_key = key_pair.public_key().as_ref().to_vec();
  Ok(WebGenerateResult::MlDsa {
    seed: seed.to_vec().into(),
    private_key: private_key.into(),
    public_key: public_key.into(),
  })
}

/// ML-KEM keypair from a fresh random 64-byte FIPS 203 seed (`d || z`). The
/// expanded decapsulation key and encapsulation key are derived from the seed
/// (matching the FIPS 203 seed model that `getPublicKey`/`raw-seed`/`pkcs8`/
/// `jwk` export rely on). The private handle keeps both the seed and the
/// expanded private key bytes.
fn generate_mlkem(name: &str) -> Result<WebGenerateResult, WebGenerateError> {
  let variant = crate::mlkem::MlKemVariant::from_name(name)
    .ok_or(WebGenerateError::PqGenerationFailed)?;
  let mut seed = [0u8; 64];
  OsRng.fill_bytes(&mut seed);
  let (private_key, public_key) = variant
    .expand_seed(&seed)
    .map_err(|_| WebGenerateError::PqGenerationFailed)?;
  Ok(WebGenerateResult::MlKem {
    seed: seed.to_vec().into(),
    private_key: private_key.into(),
    public_key: public_key.into(),
  })
}
