// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of `SubtleCrypto.encrypt` / `SubtleCrypto.decrypt`.
//!
//! This ports the per-algorithm normalization and validation that used to live
//! in `ext/crypto/00_crypto.js` (the `encrypt()` helper and the `decrypt`
//! method) into Rust ops. The thin JS stubs are responsible only for webidl
//! argument conversion and `normalizeAlgorithm` (the webidl dictionary member
//! coercion), then hand the extracted key fields + algorithm params here.
//!
//! The actual crypto is reused from `encrypt.rs` / `decrypt.rs` via the
//! `encrypt_compute` / `decrypt_compute` functions, dispatched on the
//! `EncryptAlgorithm` / `DecryptAlgorithm` enums those modules already define.

use serde::Deserialize;

use crate::decrypt::DecryptAlgorithm;
use crate::decrypt::DecryptError;
use crate::encrypt::EncryptAlgorithm;
use crate::encrypt::EncryptError;
use crate::shared::ShaHash;

/// Errors that mirror the `DOMException`s thrown by the JS `encrypt`/`decrypt`
/// methods before dispatching to the crypto ops. The crypto itself surfaces
/// `EncryptError` / `DecryptError`.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebCipherError {
  #[class("DOMExceptionInvalidAccessError")]
  #[error("{0}")]
  InvalidAccess(String),
  #[class("DOMExceptionOperationError")]
  #[error("{0}")]
  Operation(String),
  #[class("DOMExceptionNotSupportedError")]
  #[error("{0}")]
  NotSupported(String),
  #[class(type)]
  #[error("{0}")]
  Type(String),
  #[class(inherit)]
  #[error(transparent)]
  Encrypt(
    #[from]
    #[inherit]
    EncryptError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Decrypt(
    #[from]
    #[inherit]
    DecryptError,
  ),
}

/// The shape passed from the JS stub. `algorithm` is the (already
/// webidl-normalized) algorithm name as provided by the caller; it is
/// re-canonicalized case-insensitively here against the encrypt/decrypt
/// registry. The remaining fields are the relevant `CryptoKey` internals and
/// the normalized algorithm parameters.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCipherArg {
  /// `key[_type]`: "secret" | "private" | "public".
  pub key_type: String,
  /// `key[_usages]`.
  pub key_usages: Vec<String>,
  /// `key[_algorithm].name`.
  pub key_algorithm_name: String,
  /// `key[_algorithm].length` for AES keys (bits).
  pub key_length: Option<usize>,
  /// `key[_algorithm].hash.name` for RSA-OAEP keys.
  pub key_hash: Option<String>,
  /// The requested algorithm name (post webidl `normalizeAlgorithm`).
  pub algorithm: String,
  /// `iv` parameter (AES-CBC / AES-GCM / AES-OCB).
  #[serde(with = "serde_bytes", default)]
  pub iv: Option<Vec<u8>>,
  /// `counter` parameter (AES-CTR).
  #[serde(with = "serde_bytes", default)]
  pub counter: Option<Vec<u8>>,
  /// `length` parameter (AES-CTR counter length in bits).
  pub length: Option<usize>,
  /// `label` parameter (RSA-OAEP).
  #[serde(with = "serde_bytes", default)]
  pub label: Option<Vec<u8>>,
  /// `additionalData` parameter (AES-GCM / AES-OCB / ChaCha20-Poly1305).
  #[serde(with = "serde_bytes", default)]
  pub additional_data: Option<Vec<u8>>,
  /// `tagLength` parameter (AES-GCM / AES-OCB).
  pub tag_length: Option<usize>,
  /// `nonce` parameter (ChaCha20-Poly1305). `None` means it was not provided.
  #[serde(with = "serde_bytes", default)]
  pub nonce: Option<Vec<u8>>,
}

const ENCRYPT_DECRYPT_ALGORITHMS: &[&str] = &[
  "RSA-OAEP",
  "AES-CBC",
  "AES-CTR",
  "AES-GCM",
  "AES-OCB",
  "ChaCha20-Poly1305",
];

/// Canonicalize an algorithm name case-insensitively against the encrypt/decrypt
/// registry. Mirrors the case-insensitive lookup in `normalizeAlgorithm`. Unknown
/// names throw `NotSupportedError` ("Unrecognized algorithm name").
pub(crate) fn canonical_name(
  name: &str,
) -> Result<&'static str, WebCipherError> {
  ENCRYPT_DECRYPT_ALGORITHMS
    .iter()
    .copied()
    .find(|n| n.eq_ignore_ascii_case(name))
    .ok_or_else(|| {
      WebCipherError::NotSupported("Unrecognized algorithm name".to_string())
    })
}

fn sha_hash(name: &str) -> Result<ShaHash, WebCipherError> {
  Ok(match name {
    "SHA-1" => ShaHash::Sha1,
    "SHA-256" => ShaHash::Sha256,
    "SHA-384" => ShaHash::Sha384,
    "SHA-512" => ShaHash::Sha512,
    "SHA3-256" => ShaHash::Sha3_256,
    "SHA3-384" => ShaHash::Sha3_384,
    "SHA3-512" => ShaHash::Sha3_512,
    _ => {
      return Err(WebCipherError::NotSupported(
        "Unrecognized hash algorithm name".to_string(),
      ));
    }
  })
}

/// Shared validation done by both encrypt and decrypt: algorithm-name vs key
/// name mismatch (InvalidAccessError) and usage-not-included (InvalidAccessError).
pub(crate) fn check_name_and_usage(
  name: &str,
  arg: &WebCipherArg,
  usage: &str,
  mismatch_message: &str,
) -> Result<(), WebCipherError> {
  // 8. Encryption/Decryption algorithm vs key algorithm.
  if name != arg.key_algorithm_name {
    return Err(WebCipherError::InvalidAccess(mismatch_message.to_string()));
  }
  // 9. Usage included.
  if !arg.key_usages.iter().any(|u| u == usage) {
    return Err(WebCipherError::InvalidAccess(
      "The requested operation is not valid for the provided key".to_string(),
    ));
  }
  Ok(())
}

const VALID_TAG_LENGTHS: &[usize] = &[32, 64, 96, 104, 112, 120, 128];

/// Reproduces the per-algorithm validation of the JS `encrypt()` helper and
/// builds the `EncryptAlgorithm` consumed by `encrypt_compute`.
pub(crate) fn build_encrypt_algorithm(
  name: &str,
  arg: &WebCipherArg,
  data: &[u8],
) -> Result<EncryptAlgorithm, WebCipherError> {
  match name {
    "RSA-OAEP" => {
      // 1.
      if arg.key_type != "public" {
        return Err(WebCipherError::InvalidAccess(
          "Key type not supported".to_string(),
        ));
      }
      // 2. label defaults to empty.
      let label = arg.label.clone().unwrap_or_default();
      // 3-5.
      let hash = sha_hash(arg.key_hash.as_deref().unwrap_or(""))?;
      Ok(EncryptAlgorithm::RsaOaep { hash, label })
    }
    "AES-CBC" => {
      let iv = arg.iv.clone().unwrap_or_default();
      // 1.
      if iv.len() != 16 {
        return Err(WebCipherError::Operation(
          "Initialization vector must be 16 bytes".to_string(),
        ));
      }
      Ok(EncryptAlgorithm::AesCbc {
        iv,
        length: arg.key_length.unwrap_or(0),
      })
    }
    "AES-CTR" => {
      let counter = arg.counter.clone().unwrap_or_default();
      // 1.
      if counter.len() != 16 {
        return Err(WebCipherError::Operation(
          "Counter vector must be 16 bytes".to_string(),
        ));
      }
      // 2.
      let ctr_length = arg.length.unwrap_or(0);
      if ctr_length == 0 || ctr_length > 128 {
        return Err(WebCipherError::Operation(
          "Counter length must not be 0 or greater than 128".to_string(),
        ));
      }
      Ok(EncryptAlgorithm::AesCtr {
        counter,
        ctr_length,
        key_length: arg.key_length.unwrap_or(0),
      })
    }
    "AES-GCM" => {
      let iv = arg.iv.clone().unwrap_or_default();
      // 1.
      if data.len() as u64 > (1u64 << 39) - 256 {
        return Err(WebCipherError::Operation(
          "Plaintext too large".to_string(),
        ));
      }
      // 2. The JS reference checks `ArrayPrototypeIncludes([12,16], len) === undefined`,
      // which is always false (includes returns a boolean), so this iv-length
      // check is effectively dead code in the original. We preserve that
      // behavior: invalid iv lengths fall through to `encrypt_aes_gcm`, which
      // returns `EncryptError::InvalidIvLength` (a TypeError). Do NOT add a
      // NotSupportedError check here.
      // 4. tagLength default + validation.
      let tag_length = validate_tag_length(arg.tag_length)?;
      Ok(EncryptAlgorithm::AesGcm {
        iv,
        additional_data: arg.additional_data.clone(),
        length: arg.key_length.unwrap_or(0),
        tag_length,
      })
    }
    "AES-OCB" => {
      let iv = arg.iv.clone().unwrap_or_default();
      // 1.
      if data.len() as u64 > (1u64 << 39) - 256 {
        return Err(WebCipherError::Operation(
          "Plaintext too large".to_string(),
        ));
      }
      // 2. nonce 1-15 bytes.
      if iv.is_empty() || iv.len() > 15 {
        return Err(WebCipherError::Operation(
          "Invalid nonce length for AES-OCB (must be 1-15 bytes)".to_string(),
        ));
      }
      // 3. tagLength default + validation.
      let tag_length = validate_tag_length(arg.tag_length)?;
      Ok(EncryptAlgorithm::AesOcb {
        iv,
        additional_data: arg.additional_data.clone(),
        length: arg.key_length.unwrap_or(0),
        tag_length,
      })
    }
    "ChaCha20-Poly1305" => {
      let nonce = match &arg.nonce {
        Some(n) => n.clone(),
        None => {
          return Err(WebCipherError::Type("nonce is required".to_string()));
        }
      };
      if nonce.len() != 12 {
        return Err(WebCipherError::Operation(
          "ChaCha20-Poly1305 nonce must be 12 bytes".to_string(),
        ));
      }
      // RFC 8439 plaintext size cap.
      if data.len() as u64 > ((1u64 << 32) - 1) * 64 {
        return Err(WebCipherError::Operation(
          "Plaintext too large".to_string(),
        ));
      }
      Ok(EncryptAlgorithm::ChaCha20Poly1305 {
        nonce,
        additional_data: arg.additional_data.clone(),
      })
    }
    // Unreachable: canonical_name already validated.
    _ => Err(WebCipherError::NotSupported("Not implemented".to_string())),
  }
}

/// Reproduces the per-algorithm validation of the JS `decrypt` method and
/// builds the `DecryptAlgorithm` consumed by `decrypt_compute`.
pub(crate) fn build_decrypt_algorithm(
  name: &str,
  arg: &WebCipherArg,
  data: &[u8],
) -> Result<DecryptAlgorithm, WebCipherError> {
  match name {
    "RSA-OAEP" => {
      // 1.
      if arg.key_type != "private" {
        return Err(WebCipherError::InvalidAccess(
          "Key type not supported".to_string(),
        ));
      }
      // 2. label defaults to empty.
      let label = arg.label.clone().unwrap_or_default();
      // 3-5.
      let hash = sha_hash(arg.key_hash.as_deref().unwrap_or(""))?;
      Ok(DecryptAlgorithm::RsaOaep { hash, label })
    }
    "AES-CBC" => {
      let iv = arg.iv.clone().unwrap_or_default();
      // 1.
      if iv.len() != 16 {
        return Err(WebCipherError::Operation(
          "Counter must be 16 bytes".to_string(),
        ));
      }
      Ok(DecryptAlgorithm::AesCbc {
        iv,
        length: arg.key_length.unwrap_or(0),
      })
    }
    "AES-CTR" => {
      let counter = arg.counter.clone().unwrap_or_default();
      // 1.
      if counter.len() != 16 {
        return Err(WebCipherError::Operation(
          "Counter vector must be 16 bytes".to_string(),
        ));
      }
      // 2.
      let ctr_length = arg.length.unwrap_or(0);
      if ctr_length == 0 || ctr_length > 128 {
        return Err(WebCipherError::Operation(format!(
          "Counter length must not be 0 or greater than 128: received {ctr_length}"
        )));
      }
      Ok(DecryptAlgorithm::AesCtr {
        counter,
        ctr_length,
        key_length: arg.key_length.unwrap_or(0),
      })
    }
    "AES-GCM" | "AES-OCB" => {
      let iv = arg.iv.clone().unwrap_or_default();
      // 1. tagLength default + validation.
      let tag_length = validate_tag_length(arg.tag_length)?;
      // 2. data must be at least tagLength/8 bytes.
      if data.len() < tag_length / 8 {
        return Err(WebCipherError::Operation(
          "The provided data is too small".to_string(),
        ));
      }
      // 3. iv length checks.
      if name == "AES-GCM" {
        if iv.len() != 12 && iv.len() != 16 {
          return Err(WebCipherError::NotSupported(
            "Initialization vector length not supported".to_string(),
          ));
        }
      } else if iv.is_empty() || iv.len() > 15 {
        return Err(WebCipherError::NotSupported(
          "Initialization vector length not supported".to_string(),
        ));
      }
      let length = arg.key_length.unwrap_or(0);
      if name == "AES-GCM" {
        Ok(DecryptAlgorithm::AesGcm {
          iv,
          additional_data: arg.additional_data.clone(),
          length,
          tag_length,
        })
      } else {
        Ok(DecryptAlgorithm::AesOcb {
          iv,
          additional_data: arg.additional_data.clone(),
          length,
          tag_length,
        })
      }
    }
    "ChaCha20-Poly1305" => {
      let nonce = match &arg.nonce {
        Some(n) => n.clone(),
        None => {
          return Err(WebCipherError::Type("nonce is required".to_string()));
        }
      };
      if nonce.len() != 12 {
        return Err(WebCipherError::Operation(
          "ChaCha20-Poly1305 nonce must be 12 bytes".to_string(),
        ));
      }
      if data.len() < 16 {
        return Err(WebCipherError::Operation(
          "The provided data is too small".to_string(),
        ));
      }
      Ok(DecryptAlgorithm::ChaCha20Poly1305 {
        nonce,
        additional_data: arg.additional_data.clone(),
      })
    }
    // Unreachable: canonical_name already validated.
    _ => Err(WebCipherError::NotSupported("Not implemented".to_string())),
  }
}

/// `tagLength` defaults to 128 and must be one of the allowed lengths.
fn validate_tag_length(
  tag_length: Option<usize>,
) -> Result<usize, WebCipherError> {
  match tag_length {
    None => Ok(128),
    Some(t) if VALID_TAG_LENGTHS.contains(&t) => Ok(t),
    Some(t) => Err(WebCipherError::Operation(format!(
      "Invalid tag length: {t}"
    ))),
  }
}
