// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto algorithm registry, dictionary converters and normalization
//! logic. Replaces the `supportedAlgorithms` / `normalizeAlgorithm` /
//! `dictXxx` machinery that used to live in `ext/crypto/00_crypto.js`.

use deno_core::op2;

/// An operation slot in the WebCrypto algorithm registry. The string passed
/// in from JS is mapped here via [`Operation::from_name`]; aliases that
/// `SubtleCrypto.supports()` allows (e.g. `"encapsulateKey"`) are resolved
/// *before* this lookup in [`op_crypto_check_support_for_algorithm`].
#[derive(Copy, Clone)]
pub enum Operation {
  Digest,
  GenerateKey,
  Sign,
  Verify,
  Encrypt,
  Decrypt,
  ImportKey,
  Encapsulate,
  Decapsulate,
  DeriveBits,
  GetKeyLength,
  WrapKey,
  UnwrapKey,
}

impl Operation {
  fn from_name(name: &str) -> Option<Self> {
    Some(match name {
      "digest" => Self::Digest,
      "generateKey" => Self::GenerateKey,
      "sign" => Self::Sign,
      "verify" => Self::Verify,
      "encrypt" => Self::Encrypt,
      "decrypt" => Self::Decrypt,
      "importKey" => Self::ImportKey,
      "encapsulate" => Self::Encapsulate,
      "decapsulate" => Self::Decapsulate,
      "deriveBits" => Self::DeriveBits,
      "get key length" => Self::GetKeyLength,
      "wrapKey" => Self::WrapKey,
      "unwrapKey" => Self::UnwrapKey,
      _ => return None,
    })
  }

  /// Return the (algorithmName, dictionaryType) pairs that this operation
  /// slot registers. The dictionary-type string identifies the parameter
  /// dictionary used to coerce the input algorithm (the same names used by
  /// the JS-side webidl converters); `None` means the algorithm registers
  /// no operation-specific parameters.
  fn registered(self) -> &'static [(&'static str, Option<&'static str>)] {
    use Operation::*;
    match self {
      Digest => &[
        ("SHA-1", None),
        ("SHA-256", None),
        ("SHA-384", None),
        ("SHA-512", None),
        ("SHA3-256", None),
        ("SHA3-384", None),
        ("SHA3-512", None),
        ("cSHAKE128", Some("CShakeParams")),
        ("cSHAKE256", Some("CShakeParams")),
        ("TurboSHAKE128", Some("TurboShakeParams")),
        ("TurboSHAKE256", Some("TurboShakeParams")),
      ],
      GenerateKey => &[
        ("RSASSA-PKCS1-v1_5", Some("RsaHashedKeyGenParams")),
        ("RSA-PSS", Some("RsaHashedKeyGenParams")),
        ("RSA-OAEP", Some("RsaHashedKeyGenParams")),
        ("ECDSA", Some("EcKeyGenParams")),
        ("ECDH", Some("EcKeyGenParams")),
        ("AES-CTR", Some("AesKeyGenParams")),
        ("AES-CBC", Some("AesKeyGenParams")),
        ("AES-GCM", Some("AesKeyGenParams")),
        ("AES-OCB", Some("AesKeyGenParams")),
        ("AES-KW", Some("AesKeyGenParams")),
        ("HMAC", Some("HmacKeyGenParams")),
        ("ChaCha20-Poly1305", None),
        ("X25519", None),
        ("X448", None),
        ("Ed25519", None),
        ("ML-KEM-512", None),
        ("ML-KEM-768", None),
        ("ML-KEM-1024", None),
        ("ML-DSA-44", None),
        ("ML-DSA-65", None),
        ("ML-DSA-87", None),
      ],
      Sign => &[
        ("RSASSA-PKCS1-v1_5", None),
        ("RSA-PSS", Some("RsaPssParams")),
        ("ECDSA", Some("EcdsaParams")),
        ("HMAC", None),
        ("Ed25519", None),
        ("ML-DSA-44", Some("MlDsaParams")),
        ("ML-DSA-65", Some("MlDsaParams")),
        ("ML-DSA-87", Some("MlDsaParams")),
      ],
      Verify => &[
        ("RSASSA-PKCS1-v1_5", None),
        ("RSA-PSS", Some("RsaPssParams")),
        ("ECDSA", Some("EcdsaParams")),
        ("HMAC", None),
        ("Ed25519", None),
        ("ML-DSA-44", Some("MlDsaParams")),
        ("ML-DSA-65", Some("MlDsaParams")),
        ("ML-DSA-87", Some("MlDsaParams")),
      ],
      ImportKey => &[
        ("RSASSA-PKCS1-v1_5", Some("RsaHashedImportParams")),
        ("RSA-PSS", Some("RsaHashedImportParams")),
        ("RSA-OAEP", Some("RsaHashedImportParams")),
        ("ECDSA", Some("EcKeyImportParams")),
        ("ECDH", Some("EcKeyImportParams")),
        ("HMAC", Some("HmacImportParams")),
        ("HKDF", None),
        ("PBKDF2", None),
        ("AES-CTR", None),
        ("AES-CBC", None),
        ("AES-GCM", None),
        ("AES-OCB", None),
        ("AES-KW", None),
        ("ChaCha20-Poly1305", None),
        ("Ed25519", None),
        ("X25519", None),
        ("X448", None),
        ("ML-KEM-512", None),
        ("ML-KEM-768", None),
        ("ML-KEM-1024", None),
        ("ML-DSA-44", None),
        ("ML-DSA-65", None),
        ("ML-DSA-87", None),
      ],
      Encapsulate => &[
        ("ML-KEM-512", None),
        ("ML-KEM-768", None),
        ("ML-KEM-1024", None),
      ],
      Decapsulate => &[
        ("ML-KEM-512", None),
        ("ML-KEM-768", None),
        ("ML-KEM-1024", None),
      ],
      DeriveBits => &[
        ("HKDF", Some("HkdfParams")),
        ("PBKDF2", Some("Pbkdf2Params")),
        ("ECDH", Some("EcdhKeyDeriveParams")),
        ("X25519", Some("EcdhKeyDeriveParams")),
        ("X448", Some("EcdhKeyDeriveParams")),
      ],
      Encrypt => &[
        ("RSA-OAEP", Some("RsaOaepParams")),
        ("AES-CBC", Some("AesCbcParams")),
        ("AES-GCM", Some("AesGcmParams")),
        ("AES-OCB", Some("AesGcmParams")),
        ("AES-CTR", Some("AesCtrParams")),
        ("ChaCha20-Poly1305", Some("ChaCha20Poly1305Params")),
      ],
      Decrypt => &[
        ("RSA-OAEP", Some("RsaOaepParams")),
        ("AES-CBC", Some("AesCbcParams")),
        ("AES-GCM", Some("AesGcmParams")),
        ("AES-OCB", Some("AesGcmParams")),
        ("AES-CTR", Some("AesCtrParams")),
        ("ChaCha20-Poly1305", Some("ChaCha20Poly1305Params")),
      ],
      GetKeyLength => &[
        ("AES-CBC", Some("AesDerivedKeyParams")),
        ("AES-CTR", Some("AesDerivedKeyParams")),
        ("AES-GCM", Some("AesDerivedKeyParams")),
        ("AES-KW", Some("AesDerivedKeyParams")),
        ("HMAC", Some("HmacImportParams")),
        ("ChaCha20-Poly1305", None),
        ("HKDF", None),
        ("PBKDF2", None),
      ],
      WrapKey => &[("AES-KW", None)],
      UnwrapKey => &[("AES-KW", None)],
    }
  }
}

/// Asymmetric algorithms whose private keys carry enough information for
/// `SubtleCrypto.prototype.getPublicKey()` to recover the public key.
const PUBLIC_KEY_DERIVABLE_ALGORITHMS: &[&str] = &[
  "RSASSA-PKCS1-v1_5",
  "RSA-PSS",
  "RSA-OAEP",
  "ECDSA",
  "ECDH",
  "Ed25519",
  "X25519",
  "X448",
  "ML-KEM-512",
  "ML-KEM-768",
  "ML-KEM-1024",
  "ML-DSA-44",
  "ML-DSA-65",
  "ML-DSA-87",
];

/// Operations that `SubtleCrypto.supports()` accepts.
const SUPPORTS_OPERATIONS: &[&str] = &[
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "digest",
  "generateKey",
  "deriveKey",
  "deriveBits",
  "importKey",
  "exportKey",
  "wrapKey",
  "unwrapKey",
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
  "getPublicKey",
];

fn is_algorithm_registered_for(algorithm_name: &str, op: Operation) -> bool {
  op.registered()
    .iter()
    .any(|(name, _)| name.eq_ignore_ascii_case(algorithm_name))
}

fn supports_get_public_key(algorithm_name: &str) -> bool {
  PUBLIC_KEY_DERIVABLE_ALGORITHMS
    .iter()
    .any(|name| name.eq_ignore_ascii_case(algorithm_name))
}

/// Implementation of "check support for an algorithm" from the WICG modern
/// algorithms spec. `_length` from the spec signature is accepted by
/// callers but does not affect the result -- supports() is name+operation
/// only.
pub fn check_support_for_algorithm(
  operation: &str,
  algorithm_name: &str,
) -> bool {
  if !SUPPORTS_OPERATIONS.contains(&operation) {
    return false;
  }
  let registered_op = match operation {
    "encapsulateKey" | "encapsulateBits" => "encapsulate",
    "decapsulateKey" | "decapsulateBits" => "decapsulate",
    "deriveKey" => "deriveBits",
    "exportKey" | "getPublicKey" => "importKey",
    other => other,
  };
  let Some(op) = Operation::from_name(registered_op) else {
    return false;
  };
  if is_algorithm_registered_for(algorithm_name, op) {
    if operation == "getPublicKey" {
      return supports_get_public_key(algorithm_name);
    }
    return true;
  }
  match operation {
    "wrapKey" => {
      is_algorithm_registered_for(algorithm_name, Operation::Encrypt)
    }
    "unwrapKey" => {
      is_algorithm_registered_for(algorithm_name, Operation::Decrypt)
    }
    _ => false,
  }
}

/// Op wrapper around [`check_support_for_algorithm`] for the JS shim that
/// hasn't been moved onto the `SubtleCrypto` cppgc impl block yet (the
/// two-argument-name overload of `supports()` still does extra paperwork in
/// JS).
#[op2(fast)]
pub fn op_crypto_check_support_for_algorithm(
  #[string] operation: &str,
  #[string] algorithm_name: &str,
) -> bool {
  check_support_for_algorithm(operation, algorithm_name)
}

/// Result of the algorithm-registry lookup used by `normalizeAlgorithm` in
/// JS. `name == ""` is the "not found" sentinel (instead of `Option`, to
/// keep this a plain ToV8 struct).
#[derive(deno_core::ToV8)]
pub struct RegisteredAlgorithm {
  pub name: String,
  pub dict: Option<String>,
}

#[op2]
pub fn op_crypto_get_registered_algorithm(
  #[string] operation: &str,
  #[string] algorithm_name: &str,
) -> RegisteredAlgorithm {
  let Some(op) = Operation::from_name(operation) else {
    return RegisteredAlgorithm {
      name: String::new(),
      dict: None,
    };
  };
  match op.registered().iter().find_map(|(name, dict)| {
    name
      .eq_ignore_ascii_case(algorithm_name)
      .then(|| (*name, dict.map(|s| s.to_string())))
  }) {
    Some((name, dict)) => RegisteredAlgorithm {
      name: name.to_string(),
      dict,
    },
    None => RegisteredAlgorithm {
      name: String::new(),
      dict: None,
    },
  }
}

// AlgorithmIdentifier (string-or-dict) coercion is still performed in JS by
// `webidl.converters.AlgorithmIdentifier`. Once `normalizeAlgorithm` itself
// is lifted onto the Rust side that conversion will move here too.

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum GetKeyLengthError {
  #[class("DOMExceptionOperationError")]
  #[error("Length must be 128, 192, or 256: received {0}")]
  AesInvalidLength(u32),
  #[class("DOMExceptionNotSupportedError")]
  #[error("Unrecognized hash algorithm: {0}")]
  HmacUnknownHash(String),
  #[class(type)]
  #[error("Invalid length: {0}")]
  HmacInvalidLength(u32),
  #[class(type)]
  #[error("Unreachable")]
  Unreachable,
}

/// "Get key length" sub-algorithm per
/// https://www.w3.org/TR/WebCryptoAPI/#dfn-aes-keygen-get-key-length etc.
/// Pure Rust core shared between [`op_crypto_get_key_length`] (still called
/// from the JS shim) and the `supports()` 2-arg overload implemented on the
/// `SubtleCrypto` cppgc impl block.
///
/// Returns the bit length, or `Option<u32>::None` for KDFs (`HKDF`,
/// `PBKDF2`) whose output length depends on the caller.
pub fn compute_key_length(
  name: &str,
  length: Option<u32>,
  hash_name: Option<&str>,
) -> Result<Option<u32>, GetKeyLengthError> {
  match name {
    "AES-CBC" | "AES-CTR" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
      let l = length.unwrap_or(0);
      if l != 128 && l != 192 && l != 256 {
        return Err(GetKeyLengthError::AesInvalidLength(l));
      }
      Ok(Some(l))
    }
    "HMAC" => {
      if let Some(l) = length {
        if l == 0 {
          Err(GetKeyLengthError::HmacInvalidLength(l))
        } else {
          Ok(Some(l))
        }
      } else {
        let hash = hash_name.unwrap_or_default();
        let block = match hash {
          "SHA-1" | "SHA-256" | "SHA3-256" => 512,
          "SHA-384" | "SHA-512" | "SHA3-384" | "SHA3-512" => 1024,
          _ => {
            return Err(GetKeyLengthError::HmacUnknownHash(hash.to_string()));
          }
        };
        Ok(Some(block))
      }
    }
    "ChaCha20-Poly1305" => Ok(Some(256)),
    "HKDF" | "PBKDF2" => Ok(None),
    _ => Err(GetKeyLengthError::Unreachable),
  }
}

#[op2]
pub fn op_crypto_get_key_length(
  #[string] name: &str,
  length: Option<u32>,
  #[string] hash_name: Option<String>,
) -> Result<Option<u32>, GetKeyLengthError> {
  compute_key_length(name, length, hash_name.as_deref())
}

/// Resolve `name` against the registered algorithm table for `operation`,
/// returning the canonical (spec-cased) name if registered. Used by the
/// Rust-side `SubtleCrypto` methods (`importKey`, `generateKey`, …) so
/// `"aes-cbc"` and `"AES-CBC"` resolve to the same canonical name without
/// re-entering JS to call `normalizeAlgorithm`.
pub fn canonical_name_for(
  operation: &str,
  algorithm_name: &str,
) -> Option<&'static str> {
  registered_algorithm(operation, algorithm_name).map(|(n, _)| n)
}

/// Lookup the registered (canonical name, dictionary type) for an operation
/// slot. Pure-Rust mirror of the JS shim's `op_crypto_get_registered_algorithm`
/// call sequence inside `normalizeAlgorithm`; used by the `supports()` 2-arg
/// overload to resolve a `lengthOrHash` AlgorithmIdentifier in Rust.
pub fn registered_algorithm(
  operation: &str,
  algorithm_name: &str,
) -> Option<(&'static str, Option<&'static str>)> {
  let op = Operation::from_name(operation)?;
  op.registered().iter().find_map(|(name, dict)| {
    name
      .eq_ignore_ascii_case(algorithm_name)
      .then_some((*name, *dict))
  })
}
