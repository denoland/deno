// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto algorithm registry, dictionary converters and normalization
//! logic. Replaces the `supportedAlgorithms` / `normalizeAlgorithm` /
//! `dictXxx` machinery that used to live in `ext/crypto/00_crypto.js`.

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
        ("KT128", Some("KangarooTwelveParams")),
        ("KT256", Some("KangarooTwelveParams")),
        ("KangarooTwelve", Some("KangarooTwelveParams")),
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
        ("KMAC128", Some("KmacKeyGenParams")),
        ("KMAC256", Some("KmacKeyGenParams")),
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
        ("SLH-DSA-SHA2-128s", None),
        ("SLH-DSA-SHA2-128f", None),
        ("SLH-DSA-SHA2-192s", None),
        ("SLH-DSA-SHA2-192f", None),
        ("SLH-DSA-SHA2-256s", None),
        ("SLH-DSA-SHA2-256f", None),
        ("SLH-DSA-SHAKE-128s", None),
        ("SLH-DSA-SHAKE-128f", None),
        ("SLH-DSA-SHAKE-192s", None),
        ("SLH-DSA-SHAKE-192f", None),
        ("SLH-DSA-SHAKE-256s", None),
        ("SLH-DSA-SHAKE-256f", None),
      ],
      Sign => &[
        ("RSASSA-PKCS1-v1_5", None),
        ("RSA-PSS", Some("RsaPssParams")),
        ("ECDSA", Some("EcdsaParams")),
        ("HMAC", None),
        ("KMAC128", Some("KmacParams")),
        ("KMAC256", Some("KmacParams")),
        ("Ed25519", None),
        ("ML-DSA-44", Some("MlDsaParams")),
        ("ML-DSA-65", Some("MlDsaParams")),
        ("ML-DSA-87", Some("MlDsaParams")),
        ("SLH-DSA-SHA2-128s", Some("ContextParams")),
        ("SLH-DSA-SHA2-128f", Some("ContextParams")),
        ("SLH-DSA-SHA2-192s", Some("ContextParams")),
        ("SLH-DSA-SHA2-192f", Some("ContextParams")),
        ("SLH-DSA-SHA2-256s", Some("ContextParams")),
        ("SLH-DSA-SHA2-256f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-128s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-128f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-192s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-192f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-256s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-256f", Some("ContextParams")),
      ],
      Verify => &[
        ("RSASSA-PKCS1-v1_5", None),
        ("RSA-PSS", Some("RsaPssParams")),
        ("ECDSA", Some("EcdsaParams")),
        ("HMAC", None),
        ("KMAC128", Some("KmacParams")),
        ("KMAC256", Some("KmacParams")),
        ("Ed25519", None),
        ("ML-DSA-44", Some("MlDsaParams")),
        ("ML-DSA-65", Some("MlDsaParams")),
        ("ML-DSA-87", Some("MlDsaParams")),
        ("SLH-DSA-SHA2-128s", Some("ContextParams")),
        ("SLH-DSA-SHA2-128f", Some("ContextParams")),
        ("SLH-DSA-SHA2-192s", Some("ContextParams")),
        ("SLH-DSA-SHA2-192f", Some("ContextParams")),
        ("SLH-DSA-SHA2-256s", Some("ContextParams")),
        ("SLH-DSA-SHA2-256f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-128s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-128f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-192s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-192f", Some("ContextParams")),
        ("SLH-DSA-SHAKE-256s", Some("ContextParams")),
        ("SLH-DSA-SHAKE-256f", Some("ContextParams")),
      ],
      ImportKey => &[
        ("RSASSA-PKCS1-v1_5", Some("RsaHashedImportParams")),
        ("RSA-PSS", Some("RsaHashedImportParams")),
        ("RSA-OAEP", Some("RsaHashedImportParams")),
        ("ECDSA", Some("EcKeyImportParams")),
        ("ECDH", Some("EcKeyImportParams")),
        ("HMAC", Some("HmacImportParams")),
        ("KMAC128", Some("KmacImportParams")),
        ("KMAC256", Some("KmacImportParams")),
        ("HKDF", None),
        ("PBKDF2", None),
        ("Argon2i", None),
        ("Argon2d", None),
        ("Argon2id", None),
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
        ("SLH-DSA-SHA2-128s", None),
        ("SLH-DSA-SHA2-128f", None),
        ("SLH-DSA-SHA2-192s", None),
        ("SLH-DSA-SHA2-192f", None),
        ("SLH-DSA-SHA2-256s", None),
        ("SLH-DSA-SHA2-256f", None),
        ("SLH-DSA-SHAKE-128s", None),
        ("SLH-DSA-SHAKE-128f", None),
        ("SLH-DSA-SHAKE-192s", None),
        ("SLH-DSA-SHAKE-192f", None),
        ("SLH-DSA-SHAKE-256s", None),
        ("SLH-DSA-SHAKE-256f", None),
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
        ("Argon2i", Some("Argon2Params")),
        ("Argon2d", Some("Argon2Params")),
        ("Argon2id", Some("Argon2Params")),
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
        ("AES-OCB", Some("AesDerivedKeyParams")),
        ("AES-KW", Some("AesDerivedKeyParams")),
        ("HMAC", Some("HmacImportParams")),
        ("KMAC128", Some("KmacImportParams")),
        ("KMAC256", Some("KmacImportParams")),
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
  "SLH-DSA-SHA2-128s",
  "SLH-DSA-SHA2-128f",
  "SLH-DSA-SHA2-192s",
  "SLH-DSA-SHA2-192f",
  "SLH-DSA-SHA2-256s",
  "SLH-DSA-SHA2-256f",
  "SLH-DSA-SHAKE-128s",
  "SLH-DSA-SHAKE-128f",
  "SLH-DSA-SHAKE-192s",
  "SLH-DSA-SHAKE-192f",
  "SLH-DSA-SHAKE-256s",
  "SLH-DSA-SHAKE-256f",
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
  #[error("Invalid KMAC length: {0}")]
  KmacInvalidLength(u32),
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
    "KMAC128" | "KMAC256" => {
      let l = length.unwrap_or(if name == "KMAC128" { 128 } else { 256 });
      if l == 0 || !l.is_multiple_of(8) {
        return Err(GetKeyLengthError::KmacInvalidLength(l));
      }
      Ok(Some(l))
    }
    "HKDF" | "PBKDF2" | "Argon2i" | "Argon2d" | "Argon2id" => Ok(None),
    _ => Err(GetKeyLengthError::Unreachable),
  }
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
