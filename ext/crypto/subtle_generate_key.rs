// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.generateKey()` body in Rust.
//!
//! Coerces the per-algorithm GenerateKey dictionary (modulusLength,
//! publicExponent, hash, length, namedCurve) via a single
//! [`GenerateKeyAlgorithm`] WebIdlConverter, then dispatches to the
//! per-algorithm Rust keygen helpers (sync or `spawn_blocking`). The
//! returned [`GenerateKeyOutput`] is either a single `CryptoKey` (for
//! symmetric algorithms) or `{ publicKey, privateKey }` (for asymmetric
//! pairs); `ToV8` materialises both via [`crate::make_key::make_crypto_key`].

use std::borrow::Cow;

use deno_core::ToV8;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::generate_ed25519_keypair;
use crate::generate_key::generate_aes;
use crate::generate_key::generate_ec;
use crate::generate_key::generate_hmac;
use crate::generate_key::generate_rsa;
use crate::make_key::AlgorithmDict;
use crate::make_key::make_crypto_key;
use crate::shared::EcNamedCurve;
use crate::shared::RawKeyData;
use crate::shared::ShaHash;
use crate::x25519::generate_x25519_keypair;
use crate::x448::generate_x448_keypair;

/// Per-algorithm shape captured at WebIDL conversion time. Mirrors the
/// JS `simpleAlgorithmDictionaries` table.
pub enum GenerateKeyAlgorithm {
  Rsa {
    name: String,
    modulus_length: u32,
    public_exponent: Vec<u8>,
    hash: String,
  },
  Ec {
    name: String,
    named_curve: EcNamedCurve,
  },
  Aes {
    name: String,
    length: u32,
  },
  Hmac {
    hash: ShaHash,
    length: Option<u32>,
  },
  ChaCha20Poly1305,
  Ed25519,
  X25519,
  X448,
  MlKem(crate::mlkem::MlKemVariant),
  MlDsa(u8, String),
}

impl<'a> WebIdlConverter<'a> for GenerateKeyAlgorithm {
  type Options = ();
  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name, obj) = crate::subtle_encrypt::extract_name_and_obj(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
    )?;
    let canonical = crate::algorithm::canonical_name_for("generateKey", &name)
      .map(str::to_string)
      .unwrap_or(name);
    let obj = obj.as_ref();
    Ok(match canonical.as_str() {
      "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => {
        let o = obj.ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing RSA dict"))?;
        let modulus_length = read_u32_member(scope, *o, b"modulusLength")
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'modulusLength'"))?;
        let public_exponent = read_buffer_bytes(scope, *o, b"publicExponent")
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'publicExponent'"))?;
        let hash = read_hash_name(scope, *o)
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'hash'"))?;
        Self::Rsa {
          name: canonical,
          modulus_length,
          public_exponent,
          hash,
        }
      }
      "ECDSA" | "ECDH" => {
        let o = obj.ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing EC dict"))?;
        let curve_str = read_string_member(scope, *o, b"namedCurve")
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'namedCurve'"))?;
        let named_curve = match curve_str.as_str() {
          "P-256" => EcNamedCurve::P256,
          "P-384" => EcNamedCurve::P384,
          "P-521" => EcNamedCurve::P521,
          _ => {
            return Err(make_err(
              prefix.clone(),
              context.borrowed(),
              "Unsupported named curve",
            ));
          }
        };
        Self::Ec {
          name: canonical,
          named_curve,
        }
      }
      "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
        let o = obj.ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing AES dict"))?;
        let length = read_u32_member(scope, *o, b"length")
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'length'"))?;
        Self::Aes {
          name: canonical,
          length,
        }
      }
      "HMAC" => {
        let o = obj.ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing HMAC dict"))?;
        let hash_name = read_hash_name(scope, *o)
          .ok_or_else(|| make_err(prefix.clone(), context.borrowed(), "Missing 'hash'"))?;
        let hash = sha_from_name(&hash_name).ok_or_else(|| {
          make_err(prefix.clone(), context.borrowed(), "Unsupported hash")
        })?;
        let length = read_u32_member(scope, *o, b"length");
        Self::Hmac { hash, length }
      }
      "ChaCha20-Poly1305" => Self::ChaCha20Poly1305,
      "Ed25519" => Self::Ed25519,
      "X25519" => Self::X25519,
      "X448" => Self::X448,
      "ML-KEM-512" => Self::MlKem(crate::mlkem::MlKemVariant::MlKem512),
      "ML-KEM-768" => Self::MlKem(crate::mlkem::MlKemVariant::MlKem768),
      "ML-KEM-1024" => Self::MlKem(crate::mlkem::MlKemVariant::MlKem1024),
      "ML-DSA-44" => Self::MlDsa(0, canonical),
      "ML-DSA-65" => Self::MlDsa(1, canonical),
      "ML-DSA-87" => Self::MlDsa(2, canonical),
      other => {
        return Err(make_err(
          prefix,
          context,
          &format!("Unrecognized algorithm: {other}"),
        ));
      }
    })
  }
}

/// Resolved keygen result. Symmetric algorithms produce a single key;
/// asymmetric algorithms produce a key pair, optionally with HMAC's
/// computed `length` slot.
pub enum GenerateKeyOutput {
  Symmetric {
    algorithm_name: String,
    /// AES length (or HMAC length-in-bits inferred from the random
    /// material) — used to stamp the `algorithm.length` slot.
    length: Option<u32>,
    /// HMAC hash name, when present.
    hash_name: Option<String>,
    bytes: Vec<u8>,
    usages: Vec<String>,
    extractable: bool,
  },
  Pair {
    algorithm: AlgorithmDict,
    pub_usages: Vec<String>,
    priv_usages: Vec<String>,
    pub_raw: RawKeyData,
    priv_raw: RawKeyData,
    extractable: bool,
  },
}

impl<'a> ToV8<'a> for GenerateKeyOutput {
  type Error = JsErrorBox;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    match self {
      Self::Symmetric {
        algorithm_name,
        length,
        hash_name,
        bytes,
        usages,
        extractable,
      } => {
        let mut alg = AlgorithmDict::new(algorithm_name);
        if let Some(l) = length {
          alg.length = Some(l);
        }
        if let Some(h) = hash_name {
          alg.hash_name = Some(h);
        }
        let usages_strs: Vec<&str> = usages.iter().map(String::as_str).collect();
        let key = make_crypto_key(
          scope,
          CryptoKeyType::Secret,
          extractable,
          &usages_strs,
          alg,
          RawKeyData::Secret(bytes.into_boxed_slice()),
        );
        Ok(key.into())
      }
      Self::Pair {
        algorithm,
        pub_usages,
        priv_usages,
        pub_raw,
        priv_raw,
        extractable,
      } => {
        let pub_strs: Vec<&str> = pub_usages.iter().map(String::as_str).collect();
        let priv_strs: Vec<&str> = priv_usages.iter().map(String::as_str).collect();
        let pub_alg = clone_alg(&algorithm);
        let pub_key = make_crypto_key(
          scope,
          CryptoKeyType::Public,
          true,
          &pub_strs,
          pub_alg,
          pub_raw,
        );
        let priv_key = make_crypto_key(
          scope,
          CryptoKeyType::Private,
          extractable,
          &priv_strs,
          algorithm,
          priv_raw,
        );
        let obj = v8::Object::new(scope);
        let pub_k = v8::String::new(scope, "publicKey").unwrap();
        obj.set(scope, pub_k.into(), pub_key.into());
        let priv_k = v8::String::new(scope, "privateKey").unwrap();
        obj.set(scope, priv_k.into(), priv_key.into());
        Ok(obj.into())
      }
    }
  }
}

fn clone_alg(a: &AlgorithmDict) -> AlgorithmDict {
  AlgorithmDict {
    name: a.name.clone(),
    length: a.length,
    hash_name: a.hash_name.clone(),
    named_curve: a.named_curve.clone(),
    modulus_length: a.modulus_length,
    public_exponent: a.public_exponent.clone(),
  }
}

pub async fn run(
  algorithm: GenerateKeyAlgorithm,
  extractable: bool,
  usages: Vec<String>,
) -> Result<GenerateKeyOutput, CryptoError> {
  match algorithm {
    GenerateKeyAlgorithm::Rsa {
      name,
      modulus_length,
      public_exponent,
      hash,
    } => {
      check_usages(&usages, &usages_for_rsa(&name))?;
      let key_data = spawn_blocking(move || {
        generate_rsa(modulus_length, &public_exponent)
      })
      .await
      .map_err(|e| op_error(format!("Failed to generate key: {e}")))?
      .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      let alg = AlgorithmDict::new(&name)
        .with_modulus_length(modulus_length)
        .with_public_exponent({
          // Re-extract from the freshly-generated key for the algorithm slot.
          public_exponent_from_pkcs1(&key_data)
        })
        .with_hash(&hash);
      let (pub_us, priv_us) = pair_usages_rsa(&name, &usages);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Private(key_data.clone().into_boxed_slice()),
        priv_raw: RawKeyData::Private(key_data.into_boxed_slice()),
        extractable,
      })
    }
    GenerateKeyAlgorithm::Ec { name, named_curve } => {
      check_usages(&usages, &usages_for_ec(&name))?;
      let curve_str = ec_curve_str(named_curve);
      let key_data = spawn_blocking(move || generate_ec(named_curve))
        .await
        .map_err(|e| op_error(format!("Failed to generate key: {e}")))?
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      let alg = AlgorithmDict::new(&name).with_named_curve(curve_str);
      let (pub_us, priv_us) = pair_usages_ec(&name, &usages);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Private(key_data.clone().into_boxed_slice()),
        priv_raw: RawKeyData::Private(key_data.into_boxed_slice()),
        extractable,
      })
    }
    GenerateKeyAlgorithm::Aes { name, length } => {
      let allowed: &[&str] = if name == "AES-KW" {
        &["wrapKey", "unwrapKey"]
      } else {
        &["encrypt", "decrypt", "wrapKey", "unwrapKey"]
      };
      check_usages(&usages, allowed)?;
      let bytes =
        generate_aes(length as usize).map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      Ok(GenerateKeyOutput::Symmetric {
        algorithm_name: name,
        length: Some(length),
        hash_name: None,
        bytes,
        usages,
        extractable,
      })
    }
    GenerateKeyAlgorithm::Hmac { hash, length } => {
      check_usages(&usages, &["sign", "verify"])?;
      // Spec: a literal `length: 0` is OperationError.
      if length == Some(0) {
        return Err(op_error("Invalid length".into()));
      }
      let hash_name = sha_name(hash);
      let bytes = generate_hmac(hash, length.map(|l| l as usize))
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      let length_bits = (bytes.len() * 8) as u32;
      Ok(GenerateKeyOutput::Symmetric {
        algorithm_name: "HMAC".to_string(),
        length: Some(length_bits),
        hash_name: Some(hash_name.to_string()),
        bytes,
        usages,
        extractable,
      })
    }
    GenerateKeyAlgorithm::ChaCha20Poly1305 => {
      check_usages(&usages, &["encrypt", "decrypt", "wrapKey", "unwrapKey"])?;
      let bytes = generate_aes(256)
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      Ok(GenerateKeyOutput::Symmetric {
        algorithm_name: "ChaCha20-Poly1305".to_string(),
        length: None,
        hash_name: None,
        bytes,
        usages,
        extractable,
      })
    }
    GenerateKeyAlgorithm::Ed25519 => {
      check_usages(&usages, &["sign", "verify"])?;
      let mut pkey = [0u8; 32];
      let mut pubkey = [0u8; 32];
      if !generate_ed25519_keypair(&mut pkey, &mut pubkey) {
        return Err(op_error("Failed to generate key".into()));
      }
      let alg = AlgorithmDict::new("Ed25519");
      let (pub_us, priv_us) = pair_usages_ed25519(&usages);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Raw(pubkey.to_vec().into_boxed_slice()),
        priv_raw: RawKeyData::Raw(pkey.to_vec().into_boxed_slice()),
        extractable,
      })
    }
    GenerateKeyAlgorithm::X25519 => {
      check_usages(&usages, &["deriveKey", "deriveBits"])?;
      let mut pkey = [0u8; 32];
      let mut pubkey = [0u8; 32];
      generate_x25519_keypair(&mut pkey, &mut pubkey);
      let alg = AlgorithmDict::new("X25519");
      let (pub_us, priv_us) = pair_usages_xcurve(&usages);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Raw(pubkey.to_vec().into_boxed_slice()),
        priv_raw: RawKeyData::Raw(pkey.to_vec().into_boxed_slice()),
        extractable,
      })
    }
    GenerateKeyAlgorithm::X448 => {
      check_usages(&usages, &["deriveKey", "deriveBits"])?;
      let mut pkey = [0u8; 56];
      let mut pubkey = [0u8; 56];
      generate_x448_keypair(&mut pkey, &mut pubkey);
      let alg = AlgorithmDict::new("X448");
      let (pub_us, priv_us) = pair_usages_xcurve(&usages);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Raw(pubkey.to_vec().into_boxed_slice()),
        priv_raw: RawKeyData::Raw(pkey.to_vec().into_boxed_slice()),
        extractable,
      })
    }
    GenerateKeyAlgorithm::MlKem(variant) => {
      check_usages(
        &usages,
        &[
          "encapsulateKey",
          "encapsulateBits",
          "decapsulateKey",
          "decapsulateBits",
        ],
      )?;
      // Generate a fresh 64-byte FIPS 203 seed.
      let mut seed = vec![0u8; 64];
      crate::rand::thread_rng().fill(&mut seed[..]);
      let res = crate::mlkem::from_seed(variant, &seed)
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      let alg = AlgorithmDict::new(ml_kem_name(variant));
      let pub_us = filter_usages(&usages, &["encapsulateKey", "encapsulateBits"]);
      let priv_us = filter_usages(&usages, &["decapsulateKey", "decapsulateBits"]);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Raw(res.public_key.into_boxed_slice()),
        priv_raw: RawKeyData::SeededPrivate {
          seed: Some(seed.into_boxed_slice()),
          private_key: res.private_key.into_boxed_slice(),
        },
        extractable,
      })
    }
    GenerateKeyAlgorithm::MlDsa(variant, name) => {
      check_usages(&usages, &["sign", "verify"])?;
      let mut seed = vec![0u8; 32];
      crate::rand::thread_rng().fill(&mut seed[..]);
      let res = crate::mldsa::from_seed(variant, &seed)
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      let alg = AlgorithmDict::new(&name);
      let pub_us = filter_usages(&usages, &["verify"]);
      let priv_us = filter_usages(&usages, &["sign"]);
      Ok(GenerateKeyOutput::Pair {
        algorithm: alg,
        pub_usages: pub_us,
        priv_usages: priv_us,
        pub_raw: RawKeyData::Raw(res.public_key.into_boxed_slice()),
        priv_raw: RawKeyData::SeededPrivate {
          seed: Some(seed.into_boxed_slice()),
          private_key: res.private_key.into_boxed_slice(),
        },
        extractable,
      })
    }
  }
}

use crate::rand::Rng;

fn check_usages(
  usages: &[String],
  allowed: &[&str],
) -> Result<(), CryptoError> {
  for u in usages {
    if !allowed.iter().any(|a| *a == u.as_str()) {
      return Err(CryptoError::Other(JsErrorBox::new(
        "DOMExceptionSyntaxError",
        "Invalid key usage",
      )));
    }
  }
  Ok(())
}

fn filter_usages(usages: &[String], allowed: &[&str]) -> Vec<String> {
  usages
    .iter()
    .filter(|u| allowed.iter().any(|a| *a == u.as_str()))
    .cloned()
    .collect()
}

fn usages_for_rsa(name: &str) -> Vec<&'static str> {
  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => vec!["sign", "verify"],
    "RSA-OAEP" => vec!["encrypt", "decrypt", "wrapKey", "unwrapKey"],
    _ => vec![],
  }
}

fn pair_usages_rsa(name: &str, usages: &[String]) -> (Vec<String>, Vec<String>) {
  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => (
      filter_usages(usages, &["verify"]),
      filter_usages(usages, &["sign"]),
    ),
    "RSA-OAEP" => (
      filter_usages(usages, &["encrypt", "wrapKey"]),
      filter_usages(usages, &["decrypt", "unwrapKey"]),
    ),
    _ => (vec![], vec![]),
  }
}

fn usages_for_ec(name: &str) -> Vec<&'static str> {
  match name {
    "ECDSA" => vec!["sign", "verify"],
    "ECDH" => vec!["deriveKey", "deriveBits"],
    _ => vec![],
  }
}

fn pair_usages_ec(name: &str, usages: &[String]) -> (Vec<String>, Vec<String>) {
  match name {
    "ECDSA" => (
      filter_usages(usages, &["verify"]),
      filter_usages(usages, &["sign"]),
    ),
    "ECDH" => (
      vec![],
      filter_usages(usages, &["deriveKey", "deriveBits"]),
    ),
    _ => (vec![], vec![]),
  }
}

fn pair_usages_ed25519(usages: &[String]) -> (Vec<String>, Vec<String>) {
  (
    filter_usages(usages, &["verify"]),
    filter_usages(usages, &["sign"]),
  )
}

fn pair_usages_xcurve(usages: &[String]) -> (Vec<String>, Vec<String>) {
  (vec![], filter_usages(usages, &["deriveKey", "deriveBits"]))
}

fn ec_curve_str(curve: EcNamedCurve) -> &'static str {
  match curve {
    EcNamedCurve::P256 => "P-256",
    EcNamedCurve::P384 => "P-384",
    EcNamedCurve::P521 => "P-521",
  }
}

fn ml_kem_name(variant: crate::mlkem::MlKemVariant) -> &'static str {
  match variant {
    crate::mlkem::MlKemVariant::MlKem512 => "ML-KEM-512",
    crate::mlkem::MlKemVariant::MlKem768 => "ML-KEM-768",
    crate::mlkem::MlKemVariant::MlKem1024 => "ML-KEM-1024",
  }
}

fn sha_name(h: ShaHash) -> &'static str {
  match h {
    ShaHash::Sha1 => "SHA-1",
    ShaHash::Sha256 => "SHA-256",
    ShaHash::Sha384 => "SHA-384",
    ShaHash::Sha512 => "SHA-512",
    ShaHash::Sha3_256 => "SHA3-256",
    ShaHash::Sha3_384 => "SHA3-384",
    ShaHash::Sha3_512 => "SHA3-512",
  }
}

fn sha_from_name(s: &str) -> Option<ShaHash> {
  Some(match s {
    "SHA-1" => ShaHash::Sha1,
    "SHA-256" => ShaHash::Sha256,
    "SHA-384" => ShaHash::Sha384,
    "SHA-512" => ShaHash::Sha512,
    "SHA3-256" => ShaHash::Sha3_256,
    "SHA3-384" => ShaHash::Sha3_384,
    "SHA3-512" => ShaHash::Sha3_512,
    _ => return None,
  })
}

fn op_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionOperationError", msg))
}

fn make_err(
  prefix: Cow<'static, str>,
  context: ContextFn<'_>,
  msg: &str,
) -> WebIdlError {
  WebIdlError::other(prefix, context, JsErrorBox::type_error(msg.to_string()))
}

fn read_string_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<String> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, key.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  Some(v.to_rust_string_lossy(scope))
}

fn read_u32_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<u32> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, key.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  v.uint32_value(scope)
}

fn read_buffer_bytes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, key.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(v) {
    let mut out = vec![0u8; view.byte_length()];
    let n = view.copy_contents(&mut out);
    out.truncate(n);
    return Some(out);
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(v) {
    let len = ab.byte_length();
    let mut out = Vec::with_capacity(len);
    if len > 0 {
      // SAFETY: ArrayBuffer.data valid for byte_length bytes.
      unsafe {
        let src = ab.data().unwrap().as_ptr() as *const u8;
        std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
        out.set_len(len);
      }
    }
    return Some(out);
  }
  None
}

fn read_hash_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<String> {
  let key = v8::String::new_from_one_byte(
    scope,
    b"hash",
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, key.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  if v.is_string() {
    return Some(v.to_rust_string_lossy(scope));
  }
  let hash_obj = v8::Local::<v8::Object>::try_from(v).ok()?;
  let name_key = v8::String::new_from_one_byte(
    scope,
    b"name",
    v8::NewStringType::Internalized,
  )?;
  let name_val = hash_obj.get(scope, name_key.into())?;
  Some(name_val.to_string(scope)?.to_rust_string_lossy(scope))
}

fn public_exponent_from_pkcs1(pkcs1_der: &[u8]) -> Vec<u8> {
  use rsa::pkcs1::DecodeRsaPrivateKey;
  rsa::RsaPrivateKey::from_pkcs1_der(pkcs1_der)
    .map(|k| {
      use rsa::traits::PublicKeyParts;
      let e = k.e();
      e.to_bytes_be()
    })
    .unwrap_or_else(|_| vec![1, 0, 1])
}
