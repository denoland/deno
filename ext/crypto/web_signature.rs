// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of `SubtleCrypto.sign` / `SubtleCrypto.verify`.
//!
//! These ops replace the per-algorithm crypto dispatch that used to live in
//! `00_crypto.js`. The thin JS stubs still perform the validations that throw
//! `InvalidAccessError` / `NotSupportedError` DOMExceptions (those class
//! strings have no registered error builder in `runtime/js/99_main.js`, so a
//! Rust `#[class(...)]` would surface as a plain `Error`, not a real
//! `DOMException` — see the "RISKS" note in the deliverable). Everything else
//! — algorithm-name normalization for dispatch, parameter handling and the
//! actual cryptography — is performed here.
//!
//! The compute bodies are deliberately *replicated* from `op_crypto_sign_key`
//! / `op_crypto_verify_key` (lib.rs) and `op_crypto_sign_ed25519` /
//! `op_crypto_verify_ed25519` (ed25519.rs) and `op_crypto_sign_mldsa` /
//! `op_crypto_verify_mldsa` (mldsa.rs), because one op cannot call another and
//! the task forbids editing those files. The parent is expected to dedup this
//! later.

use aws_lc_rs::hmac::Algorithm as HmacAlgorithm;
use aws_lc_rs::hmac::Key as HmacKey;
use aws_lc_rs::signature::Ed25519KeyPair;
use aws_lc_rs::signature::UnparsedPublicKey;
use aws_lc_rs::unstable::signature::ML_DSA_44;
use aws_lc_rs::unstable::signature::ML_DSA_44_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_65;
use aws_lc_rs::unstable::signature::ML_DSA_65_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_87;
use aws_lc_rs::unstable::signature::ML_DSA_87_SIGNING;
use aws_lc_rs::unstable::signature::PqdsaKeyPair;
use aws_lc_rs::unstable::signature::PqdsaSigningAlgorithm;
use aws_lc_rs::unstable::signature::PqdsaVerificationAlgorithm;
use deno_core::unsync::spawn_blocking;
use p256::ecdsa::Signature as P256Signature;
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::ecdsa::VerifyingKey as P256VerifyingKey;
use p256::pkcs8::DecodePrivateKey;
use p384::ecdsa::Signature as P384Signature;
use p384::ecdsa::SigningKey as P384SigningKey;
use p384::ecdsa::VerifyingKey as P384VerifyingKey;
use p521::ecdsa::Signature as P521Signature;
use p521::ecdsa::SigningKey as P521SigningKey;
use p521::ecdsa::VerifyingKey as P521VerifyingKey;
use rand::rngs::OsRng;
use rsa::Pss;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::signature::SignatureEncoding;
use rsa::signature::Signer;
use rsa::signature::Verifier;
use rsa::traits::SignatureScheme;
use serde::Deserialize;
use sha1::Sha1;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;
use sha3::Sha3_256;
use sha3::Sha3_384;
use sha3::Sha3_512;
use signature::hazmat::PrehashSigner;
use signature::hazmat::PrehashVerifier;

use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;

/// The `[[type]]` of the `CryptoKey` (`key[_type]`), passed verbatim from JS.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebKeyType {
  Secret,
  Private,
  Public,
}

/// Errors that map cleanly to a *registered* DOMException builder or a plain
/// `TypeError`/`Error`. The DOMException validations that must remain in JS
/// (`InvalidAccessError`, `NotSupportedError`) are intentionally not modelled
/// here.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebSignatureError {
  #[class(inherit)]
  #[error(transparent)]
  JoinError(
    #[from]
    #[inherit]
    tokio::task::JoinError,
  ),
  // RSASSA / RSA-PSS key decoding and signing/verifying.
  #[class(generic)]
  #[error(transparent)]
  Rsa(#[from] rsa::Error),
  #[class(generic)]
  #[error(transparent)]
  Pkcs1(#[from] rsa::pkcs1::Error),
  // ECDSA.
  #[class(generic)]
  #[error(transparent)]
  P256Ecdsa(#[from] p256::ecdsa::Error),
  #[class(type)]
  #[error("Invalid key format")]
  InvalidKeyFormat,
  #[class(type)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class(type)]
  #[error("Missing argument hash")]
  MissingArgumentHash,
  #[class(type)]
  #[error("Missing argument saltLength")]
  MissingArgumentSaltLength,
  #[class(type)]
  #[error("Missing argument namedCurve")]
  MissingArgumentNamedCurve,
  #[class(type)]
  #[error("unsupported algorithm")]
  UnsupportedAlgorithm,
  // Ed25519 sign failure -> OperationError (matches JS DOMException).
  #[class("DOMExceptionOperationError")]
  #[error("Failed to sign")]
  Ed25519SignFailed,
  // ML-DSA: mirror the variants from `mldsa::MlDsaError`.
  #[class("DOMExceptionDataError")]
  #[error("Invalid key data")]
  MlDsaInvalidKeyData,
  #[class("DOMExceptionOperationError")]
  #[error("Signing failed")]
  MlDsaSigningFailed,
  #[class("DOMExceptionNotSupportedError")]
  #[error("Non-empty context is not supported")]
  MlDsaContextNotSupported,
  #[class("DOMExceptionDataError")]
  #[error("Unknown ML-DSA variant")]
  MlDsaUnknownVariant,
}

/// Canonicalize a sign/verify algorithm name case-insensitively against the
/// `supportedAlgorithms["sign"|"verify"]` registry. The JS `normalizeAlgorithm`
/// has already done this, but we re-normalize defensively so the Rust dispatch
/// is self-contained.
fn canonical_signature_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &[
    "RSASSA-PKCS1-v1_5",
    "RSA-PSS",
    "ECDSA",
    "HMAC",
    "Ed25519",
    "ML-DSA-44",
    "ML-DSA-65",
    "ML-DSA-87",
  ];
  NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

fn mldsa_variant(name: &str) -> Option<u8> {
  match name {
    "ML-DSA-44" => Some(0),
    "ML-DSA-65" => Some(1),
    "ML-DSA-87" => Some(2),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// ML-DSA parameter table (replicated from mldsa.rs).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct MlDsaParams {
  signing: &'static PqdsaSigningAlgorithm,
  verifying: &'static PqdsaVerificationAlgorithm,
  sig_len: usize,
}

fn mldsa_params(variant: u8) -> Result<MlDsaParams, WebSignatureError> {
  match variant {
    0 => Ok(MlDsaParams {
      signing: &ML_DSA_44_SIGNING,
      verifying: &ML_DSA_44,
      sig_len: 2420,
    }),
    1 => Ok(MlDsaParams {
      signing: &ML_DSA_65_SIGNING,
      verifying: &ML_DSA_65,
      sig_len: 3309,
    }),
    2 => Ok(MlDsaParams {
      signing: &ML_DSA_87_SIGNING,
      verifying: &ML_DSA_87,
      sig_len: 4627,
    }),
    _ => Err(WebSignatureError::MlDsaUnknownVariant),
  }
}

// ---------------------------------------------------------------------------
// HMAC helpers (replicated from lib.rs).
// ---------------------------------------------------------------------------

fn hmac_sign<M: hmac::Mac + hmac::digest::KeyInit>(
  key: &[u8],
  data: &[u8],
) -> Result<Vec<u8>, WebSignatureError> {
  let mut mac = <M as hmac::Mac>::new_from_slice(key)
    .map_err(|_| WebSignatureError::InvalidKeyLength)?;
  mac.update(data);
  Ok(mac.finalize().into_bytes().to_vec())
}

fn hmac_verify<M: hmac::Mac + hmac::digest::KeyInit>(
  key: &[u8],
  data: &[u8],
  signature: &[u8],
) -> Result<bool, WebSignatureError> {
  let mut mac = <M as hmac::Mac>::new_from_slice(key)
    .map_err(|_| WebSignatureError::InvalidKeyLength)?;
  mac.update(data);
  Ok(mac.verify_slice(signature).is_ok())
}

/// Read an RSA public key from either a public (PKCS#1 DER SPKI body) or
/// private (PKCS#1 DER) key blob. Mirrors `read_rsa_public_key` in lib.rs.
fn read_rsa_public_key(
  key_type: WebKeyType,
  key_data: &[u8],
) -> Result<RsaPublicKey, WebSignatureError> {
  Ok(match key_type {
    WebKeyType::Private => {
      RsaPrivateKey::from_pkcs1_der(key_data)?.to_public_key()
    }
    WebKeyType::Public => RsaPublicKey::from_pkcs1_der(key_data)?,
    WebKeyType::Secret => return Err(WebSignatureError::InvalidKeyFormat),
  })
}

// ---------------------------------------------------------------------------
// sign
// ---------------------------------------------------------------------------

/// `SubtleCrypto.sign` crypto compute. The JS stub has already done the
/// DOMException-throwing validations (algorithm/key match, usage, key type,
/// ECDSA curve support). `key_data` is the raw key material:
///   - RSA/ECDSA/HMAC: the `keyData.data` bytes (PKCS#1/PKCS#8 DER, or raw HMAC
///     secret),
///   - Ed25519: the 32-byte seed,
///   - ML-DSA: the raw private key bytes (`keyData.privateKey`).
///
/// `hash` is the key's hash (`key[_algorithm].hash.name`) for RSA/HMAC or the
/// algorithm's hash (`normalizedAlgorithm.hash.name`) for ECDSA. `named_curve`
/// is `key[_algorithm].namedCurve` for ECDSA. `salt_length` is
/// `normalizedAlgorithm.saltLength` for RSA-PSS. `context` is
/// `normalizedAlgorithm.context` for ML-DSA.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn sign_web_compute(
  algorithm: String,
  key_type: WebKeyType,
  hash: Option<CryptoHash>,
  salt_length: Option<u32>,
  named_curve: Option<CryptoNamedCurve>,
  key_data: Vec<u8>,
  data: Vec<u8>,
  context: Option<Vec<u8>>,
) -> Result<Vec<u8>, WebSignatureError> {
  let name = canonical_signature_name(&algorithm)
    .ok_or(WebSignatureError::UnsupportedAlgorithm)?;

  // Ed25519 / ML-DSA are fast/sync ops in the original; keep them inline (no
  // spawn_blocking) to match the original behaviour.
  match name {
    "Ed25519" => {
      let pair = Ed25519KeyPair::from_seed_unchecked(&key_data)
        .map_err(|_| WebSignatureError::Ed25519SignFailed)?;
      let sig = pair.sign(&data);
      return Ok(sig.as_ref().to_vec());
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      let variant =
        mldsa_variant(name).ok_or(WebSignatureError::MlDsaUnknownVariant)?;
      let p = mldsa_params(variant)?;
      // aws-lc-rs 1.16 cannot set the FIPS 204 context; reject non-empty.
      if context.as_ref().is_some_and(|c| !c.is_empty()) {
        return Err(WebSignatureError::MlDsaContextNotSupported);
      }
      let key_pair = PqdsaKeyPair::from_raw_private_key(p.signing, &key_data)
        .map_err(|_| WebSignatureError::MlDsaInvalidKeyData)?;
      let mut signature = vec![0u8; p.sig_len];
      key_pair
        .sign(&data, &mut signature)
        .map_err(|_| WebSignatureError::MlDsaSigningFailed)?;
      return Ok(signature);
    }
    _ => {}
  }

  let signature = spawn_blocking(move || {
    let data = &*data;
    let key = &*key_data;
    let signature: Vec<u8> = match name {
      "RSASSA-PKCS1-v1_5" => {
        use rsa::pkcs1v15::SigningKey;
        let private_key = RsaPrivateKey::from_pkcs1_der(key)?;
        match hash.ok_or(WebSignatureError::MissingArgumentHash)? {
          CryptoHash::Sha1 => SigningKey::<Sha1>::new(private_key).sign(data),
          CryptoHash::Sha256 => {
            SigningKey::<Sha256>::new(private_key).sign(data)
          }
          CryptoHash::Sha384 => {
            SigningKey::<Sha384>::new(private_key).sign(data)
          }
          CryptoHash::Sha512 => {
            SigningKey::<Sha512>::new(private_key).sign(data)
          }
          _ => return Err(WebSignatureError::UnsupportedAlgorithm),
        }
        .to_vec()
      }
      "RSA-PSS" => {
        let private_key = RsaPrivateKey::from_pkcs1_der(key)?;
        let salt_len = salt_length
          .ok_or(WebSignatureError::MissingArgumentSaltLength)?
          as usize;
        let mut rng = OsRng;
        match hash.ok_or(WebSignatureError::MissingArgumentHash)? {
          CryptoHash::Sha1 => {
            let s = Pss::new_with_salt::<Sha1>(salt_len);
            s.sign(Some(&mut rng), &private_key, &Sha1::digest(data))?
          }
          CryptoHash::Sha256 => {
            let s = Pss::new_with_salt::<Sha256>(salt_len);
            s.sign(Some(&mut rng), &private_key, &Sha256::digest(data))?
          }
          CryptoHash::Sha384 => {
            let s = Pss::new_with_salt::<Sha384>(salt_len);
            s.sign(Some(&mut rng), &private_key, &Sha384::digest(data))?
          }
          CryptoHash::Sha512 => {
            let s = Pss::new_with_salt::<Sha512>(salt_len);
            s.sign(Some(&mut rng), &private_key, &Sha512::digest(data))?
          }
          _ => return Err(WebSignatureError::UnsupportedAlgorithm),
        }
      }
      "ECDSA" => {
        let hash = hash.ok_or(WebSignatureError::MissingArgumentHash)?;
        let named_curve =
          named_curve.ok_or(WebSignatureError::MissingArgumentNamedCurve)?;
        match named_curve {
          CryptoNamedCurve::P256 => {
            let secret_key = p256::SecretKey::from_pkcs8_der(key)
              .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
            let signing_key = P256SigningKey::from(secret_key);
            let prehash = ecdsa_prehash(hash, data)?;
            let sig: P256Signature = signing_key.sign_prehash(&prehash)?;
            sig.to_bytes().to_vec()
          }
          CryptoNamedCurve::P384 => {
            let secret_key = p384::SecretKey::from_pkcs8_der(key)
              .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
            let signing_key = P384SigningKey::from(secret_key);
            let prehash = ecdsa_prehash(hash, data)?;
            let sig: P384Signature = signing_key.sign_prehash(&prehash)?;
            sig.to_bytes().to_vec()
          }
          CryptoNamedCurve::P521 => {
            let secret_key = p521::SecretKey::from_pkcs8_der(key)
              .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
            let signing_key =
              P521SigningKey::from_bytes(&secret_key.to_bytes())
                .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
            let prehash = ecdsa_prehash_p521(hash, data)?;
            let sig: P521Signature = signing_key.sign_prehash(&prehash)?;
            sig.to_bytes().to_vec()
          }
        }
      }
      "HMAC" => {
        let hash = hash.ok_or(WebSignatureError::UnsupportedAlgorithm)?;
        match hash {
          CryptoHash::Sha3_256 => hmac_sign::<hmac::Hmac<Sha3_256>>(key, data)?,
          CryptoHash::Sha3_384 => hmac_sign::<hmac::Hmac<Sha3_384>>(key, data)?,
          CryptoHash::Sha3_512 => hmac_sign::<hmac::Hmac<Sha3_512>>(key, data)?,
          _ => {
            let hash: HmacAlgorithm = hash.into();
            let hmac_key = HmacKey::new(hash, key);
            aws_lc_rs::hmac::sign(&hmac_key, data).as_ref().to_vec()
          }
        }
      }
      _ => return Err(WebSignatureError::UnsupportedAlgorithm),
    };
    Ok::<Vec<u8>, WebSignatureError>(signature)
  })
  .await??;

  let _ = key_type; // key_type is only needed for verify; silence unused.
  Ok(signature)
}

// ---------------------------------------------------------------------------
// verify
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) async fn verify_web_compute(
  algorithm: String,
  key_type: WebKeyType,
  hash: Option<CryptoHash>,
  salt_length: Option<u32>,
  named_curve: Option<CryptoNamedCurve>,
  key_data: Vec<u8>,
  data: Vec<u8>,
  signature: Vec<u8>,
  context: Option<Vec<u8>>,
) -> Result<bool, WebSignatureError> {
  let name = canonical_signature_name(&algorithm)
    .ok_or(WebSignatureError::UnsupportedAlgorithm)?;

  match name {
    "Ed25519" => {
      return Ok(
        UnparsedPublicKey::new(&aws_lc_rs::signature::ED25519, &key_data)
          .verify(&data, &signature)
          .is_ok(),
      );
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      let variant = match mldsa_variant(name) {
        Some(v) => v,
        None => return Ok(false),
      };
      let p = match mldsa_params(variant) {
        Ok(p) => p,
        Err(_) => return Ok(false),
      };
      if context.as_ref().is_some_and(|c| !c.is_empty()) {
        return Ok(false);
      }
      return Ok(
        UnparsedPublicKey::new(p.verifying, &key_data)
          .verify(&data, &signature)
          .is_ok(),
      );
    }
    _ => {}
  }

  let verification = spawn_blocking(move || {
    let data = &*data;
    let key = &*key_data;
    let sig = &*signature;
    let result = match name {
      "RSASSA-PKCS1-v1_5" => {
        use rsa::pkcs1v15::Signature;
        use rsa::pkcs1v15::VerifyingKey;
        let public_key = read_rsa_public_key(key_type, key)?;
        let signature: Signature = sig.try_into()?;
        match hash.ok_or(WebSignatureError::MissingArgumentHash)? {
          CryptoHash::Sha1 => VerifyingKey::<Sha1>::new(public_key)
            .verify(data, &signature)
            .is_ok(),
          CryptoHash::Sha256 => VerifyingKey::<Sha256>::new(public_key)
            .verify(data, &signature)
            .is_ok(),
          CryptoHash::Sha384 => VerifyingKey::<Sha384>::new(public_key)
            .verify(data, &signature)
            .is_ok(),
          CryptoHash::Sha512 => VerifyingKey::<Sha512>::new(public_key)
            .verify(data, &signature)
            .is_ok(),
          _ => return Err(WebSignatureError::UnsupportedAlgorithm),
        }
      }
      "RSA-PSS" => {
        let public_key = read_rsa_public_key(key_type, key)?;
        let salt_len = salt_length
          .ok_or(WebSignatureError::MissingArgumentSaltLength)?
          as usize;
        match hash.ok_or(WebSignatureError::MissingArgumentHash)? {
          CryptoHash::Sha1 => Pss::new_with_salt::<Sha1>(salt_len)
            .verify(&public_key, &Sha1::digest(data), sig)
            .is_ok(),
          CryptoHash::Sha256 => Pss::new_with_salt::<Sha256>(salt_len)
            .verify(&public_key, &Sha256::digest(data), sig)
            .is_ok(),
          CryptoHash::Sha384 => Pss::new_with_salt::<Sha384>(salt_len)
            .verify(&public_key, &Sha384::digest(data), sig)
            .is_ok(),
          CryptoHash::Sha512 => Pss::new_with_salt::<Sha512>(salt_len)
            .verify(&public_key, &Sha512::digest(data), sig)
            .is_ok(),
          _ => return Err(WebSignatureError::UnsupportedAlgorithm),
        }
      }
      "HMAC" => {
        let hash = hash.ok_or(WebSignatureError::UnsupportedAlgorithm)?;
        match hash {
          CryptoHash::Sha3_256 => {
            hmac_verify::<hmac::Hmac<Sha3_256>>(key, data, sig)?
          }
          CryptoHash::Sha3_384 => {
            hmac_verify::<hmac::Hmac<Sha3_384>>(key, data, sig)?
          }
          CryptoHash::Sha3_512 => {
            hmac_verify::<hmac::Hmac<Sha3_512>>(key, data, sig)?
          }
          _ => {
            let hash: HmacAlgorithm = hash.into();
            let hmac_key = HmacKey::new(hash, key);
            aws_lc_rs::hmac::verify(&hmac_key, data, sig).is_ok()
          }
        }
      }
      "ECDSA" => {
        let hash = hash.ok_or(WebSignatureError::MissingArgumentHash)?;
        let named_curve =
          named_curve.ok_or(WebSignatureError::MissingArgumentNamedCurve)?;
        match named_curve {
          CryptoNamedCurve::P256 => {
            let verifying_key = match key_type {
              WebKeyType::Public => P256VerifyingKey::from_sec1_bytes(key)
                .map_err(|_| WebSignatureError::InvalidKeyFormat)?,
              WebKeyType::Private => {
                let secret_key = p256::SecretKey::from_pkcs8_der(key)
                  .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
                *P256SigningKey::from(secret_key).verifying_key()
              }
              _ => return Err(WebSignatureError::InvalidKeyFormat),
            };
            match P256Signature::from_slice(sig) {
              Ok(signature) => {
                let prehash = ecdsa_prehash(hash, data)?;
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
          CryptoNamedCurve::P384 => {
            let verifying_key = match key_type {
              WebKeyType::Public => P384VerifyingKey::from_sec1_bytes(key)
                .map_err(|_| WebSignatureError::InvalidKeyFormat)?,
              WebKeyType::Private => {
                let secret_key = p384::SecretKey::from_pkcs8_der(key)
                  .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
                *P384SigningKey::from(secret_key).verifying_key()
              }
              _ => return Err(WebSignatureError::InvalidKeyFormat),
            };
            match P384Signature::from_slice(sig) {
              Ok(signature) => {
                let prehash = ecdsa_prehash(hash, data)?;
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
          CryptoNamedCurve::P521 => {
            let verifying_key = match key_type {
              WebKeyType::Public => P521VerifyingKey::from_sec1_bytes(key)
                .map_err(|_| WebSignatureError::InvalidKeyFormat)?,
              WebKeyType::Private => {
                let secret_key = p521::SecretKey::from_pkcs8_der(key)
                  .map_err(|_| WebSignatureError::InvalidKeyFormat)?;
                let inner_signing_key =
                  ecdsa::SigningKey::<p521::NistP521>::from(secret_key);
                P521VerifyingKey::from(*inner_signing_key.verifying_key())
              }
              _ => return Err(WebSignatureError::InvalidKeyFormat),
            };
            match P521Signature::from_slice(sig) {
              Ok(signature) => {
                let prehash = ecdsa_prehash_p521(hash, data)?;
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
        }
      }
      _ => return Err(WebSignatureError::UnsupportedAlgorithm),
    };
    Ok::<bool, WebSignatureError>(result)
  })
  .await??;

  Ok(verification)
}

/// Prehash for P-256/P-384 ECDSA (replicated from lib.rs).
fn ecdsa_prehash(
  hash: CryptoHash,
  data: &[u8],
) -> Result<Vec<u8>, WebSignatureError> {
  Ok(match hash {
    CryptoHash::Sha1 => Sha1::digest(data).to_vec(),
    CryptoHash::Sha256 => Sha256::digest(data).to_vec(),
    CryptoHash::Sha384 => Sha384::digest(data).to_vec(),
    CryptoHash::Sha512 => Sha512::digest(data).to_vec(),
    _ => return Err(WebSignatureError::UnsupportedAlgorithm),
  })
}

/// Prehash for P-521 ECDSA: left-pads short digests to the 33-byte minimum
/// required by `bits2field` (replicated from lib.rs).
fn ecdsa_prehash_p521(
  hash: CryptoHash,
  data: &[u8],
) -> Result<Vec<u8>, WebSignatureError> {
  let prehash = ecdsa_prehash(hash, data)?;
  Ok(if prehash.len() < 33 {
    let mut padded = vec![0u8; 33 - prehash.len()];
    padded.extend_from_slice(&prehash);
    padded
  } else {
    prehash
  })
}
