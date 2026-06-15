// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.importKey()` body in Rust.
//!
//! Replaces the legacy JS `importKeyInner` dispatcher and every per-algorithm
//! helper (`importKeyAES`, `importKeyHMAC`, `importKeyChaCha20Poly1305`,
//! `importKeyKdf`, `importKeyOkp`, `importKeyEC`, `importKeyRSA`,
//! `importKeyMlKem`, `importKeyMlDsa`). All JWK validation, format-specific
//! parsing, and key construction happens inside Rust; the result is the
//! v8 `CryptoKey` minted via [`crate::make_key::make_crypto_key`].
//!
//! The runner returns a `v8::Global<v8::Object>` rather than a Rust struct
//! because the spec-mandated `algorithm` slot is a v8 object stamped with
//! per-algorithm fields (e.g. RSA's `modulusLength` + `publicExponent`,
//! HMAC's `hash` dictionary, AES-*'s `length`); building it once here in
//! Rust avoids the extra round-trip to a `ToV8` derivation step.

use std::borrow::Cow;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;

/// JWK base64url decoding per the WebCrypto / JWA spec must accept inputs
/// whose trailing bit count isn't a multiple of 8 (e.g. `"xxx"` → 2 bytes
/// with 2 stray bits) and must tolerate `=` padding even on a "no-pad"
/// alphabet. The default `BASE64_URL_SAFE_NO_PAD` engine rejects both.
/// Mirrors [`crate::import_key::BASE64_URL_SAFE_FORGIVING`] -- a separate
/// definition because the existing helper is private to that legacy module.
const BASE64_JWK_FORGIVING: base64::engine::general_purpose::GeneralPurpose =
  base64::engine::general_purpose::GeneralPurpose::new(
    &base64::alphabet::URL_SAFE,
    base64::engine::general_purpose::GeneralPurposeConfig::new()
      .with_decode_allow_trailing_bits(true)
      .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
  );
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::make_key::AlgorithmDict;
use crate::make_key::make_crypto_key;
use crate::shared::RawKeyData;
use crate::subtle_export_key::KeyFormat;

const ALL_USAGES: &[&str] = &[
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "deriveKey",
  "deriveBits",
  "wrapKey",
  "unwrapKey",
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
];

/// Argument-coerced view of the algorithm dictionary the user passed.
/// Extracts every per-algorithm slot (`hash`, `length`, `namedCurve`,
/// `modulusLength`, `publicExponent`) up front, so the import-path
/// dispatch can run off the v8 stack — needed by `deriveKey`'s
/// `spawn_blocking` and by the structured-clone resurrection path. The
/// optional `jwk_alg` slot is the raw `alg` member off the user-supplied
/// algorithm dictionary when it itself names an algorithm (it almost
/// never does; included so importKey's "hash" sub-normalization works).
#[derive(Clone)]
pub struct ImportAlgorithm {
  pub name: String,
  pub hash_name: Option<String>,
  pub length: Option<u32>,
  pub named_curve: Option<String>,
  #[allow(
    dead_code,
    reason = "captured by the WebIDL converter for RSA `algorithm` slot \
              parity; consumed by the RSA importKey path in \
              future work that strips RSA importKey JWK extraction"
  )]
  pub modulus_length: Option<u32>,
  #[allow(
    dead_code,
    reason = "captured by the WebIDL converter for RSA `algorithm` slot \
              parity; consumed by the RSA importKey path in \
              future work that strips RSA importKey JWK extraction"
  )]
  pub public_exponent: Option<Vec<u8>>,
}

impl<'a> WebIdlConverter<'a> for ImportAlgorithm {
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
    let canonical = crate::algorithm::canonical_name_for("importKey", &name)
      .map(str::to_string)
      .unwrap_or(name);
    let hash_name = obj.as_ref().and_then(|o| read_hash_name(scope, *o));
    let length = obj
      .as_ref()
      .and_then(|o| read_u32_member(scope, *o, b"length"));
    let named_curve = obj
      .as_ref()
      .and_then(|o| read_string_member(scope, *o, b"namedCurve"));
    let modulus_length = obj
      .as_ref()
      .and_then(|o| read_u32_member(scope, *o, b"modulusLength"));
    let public_exponent = obj
      .as_ref()
      .and_then(|o| read_buffer_source_bytes(scope, *o, b"publicExponent"));
    Ok(ImportAlgorithm {
      name: canonical,
      hash_name,
      length,
      named_curve,
      modulus_length,
      public_exponent,
    })
  }
}

fn read_buffer_source_bytes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(val) {
    let mut out = vec![0u8; view.byte_length()];
    let n = view.copy_contents(&mut out);
    out.truncate(n);
    return Some(out);
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(val) {
    let len = ab.byte_length();
    let mut out = Vec::with_capacity(len);
    if len > 0 {
      // SAFETY: ArrayBuffer.data is valid for byte_length bytes.
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

/// Carries either the BufferSource bytes (for `raw`/`raw-*`/`spki`/`pkcs8`)
/// or the JSON object (for `jwk`). For `raw-*`/`spki`/`pkcs8` formats
/// `keyData` must be a BufferSource; for `jwk` it must be a JsonWebKey
/// object. The pair of formats is mutually exclusive per the spec.
pub enum ImportKeyData {
  Buffer(Vec<u8>),
  Jwk(v8::Global<v8::Object>),
}

impl ImportKeyData {
  pub fn from_v8<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    format: KeyFormat,
  ) -> Result<Self, CryptoError> {
    if format == KeyFormat::Jwk {
      if v8::Local::<v8::ArrayBufferView>::try_from(value).is_ok()
        || v8::Local::<v8::ArrayBuffer>::try_from(value).is_ok()
      {
        return Err(CryptoError::Other(JsErrorBox::type_error(
          "Cannot import key: 'keyData' is not a JsonWebKey",
        )));
      }
      let obj = v8::Local::<v8::Object>::try_from(value).map_err(|_| {
        CryptoError::Other(JsErrorBox::type_error(
          "Cannot import key: 'keyData' is not a JsonWebKey",
        ))
      })?;
      Ok(Self::Jwk(v8::Global::new(scope, obj)))
    } else if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
      let mut out = vec![0u8; view.byte_length()];
      let n = view.copy_contents(&mut out);
      out.truncate(n);
      Ok(Self::Buffer(out))
    } else if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
      let len = ab.byte_length();
      let mut out = Vec::with_capacity(len);
      if len > 0 {
        // SAFETY: ArrayBuffer.data is valid for byte_length bytes.
        unsafe {
          let src = ab.data().unwrap().as_ptr() as *const u8;
          std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
          out.set_len(len);
        }
      }
      Ok(Self::Buffer(out))
    } else {
      Err(CryptoError::Other(JsErrorBox::type_error(
        "Cannot import key: 'keyData' is a JsonWebKey",
      )))
    }
  }
}

/// Dispatcher entry point for `SubtleCrypto.importKey()`.
pub fn run<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  format: KeyFormat,
  algorithm: &ImportAlgorithm,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let name = algorithm.name.as_str();
  match name {
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" => import_key_aes(
      scope,
      name,
      format,
      key_data,
      extractable,
      usages,
      &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
    ),
    "AES-KW" => import_key_aes(
      scope,
      name,
      format,
      key_data,
      extractable,
      usages,
      &["wrapKey", "unwrapKey"],
    ),
    "ChaCha20-Poly1305" => {
      import_key_chacha20(scope, format, key_data, extractable, usages)
    }
    "HMAC" => import_key_hmac(
      scope,
      algorithm.hash_name.as_deref(),
      algorithm.length,
      format,
      key_data,
      extractable,
      usages,
    ),
    "KMAC128" | "KMAC256" => import_key_kmac(
      scope,
      name,
      algorithm.length,
      format,
      key_data,
      extractable,
      usages,
    ),
    "HKDF" | "PBKDF2" | "Argon2i" | "Argon2d" | "Argon2id" => {
      import_key_kdf(scope, name, format, key_data, extractable, usages)
    }
    "Ed25519" => import_key_okp(
      scope,
      OkpKind::Ed25519,
      format,
      key_data,
      extractable,
      usages,
    ),
    "X25519" => import_key_okp(
      scope,
      OkpKind::X25519,
      format,
      key_data,
      extractable,
      usages,
    ),
    "X448" => import_key_okp(
      scope,
      OkpKind::X448,
      format,
      key_data,
      extractable,
      usages,
    ),
    "ECDSA" | "ECDH" => import_key_ec(
      scope,
      name,
      algorithm.named_curve.as_deref(),
      format,
      key_data,
      extractable,
      usages,
    ),
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => import_key_rsa(
      scope,
      name,
      algorithm.hash_name.as_deref(),
      format,
      key_data,
      extractable,
      usages,
    ),
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      import_key_ml_kem(scope, name, format, key_data, extractable, usages)
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      import_key_ml_dsa(scope, name, format, key_data, extractable, usages)
    }
    _ if crate::slhdsa::variant_from_name(name).is_some() => {
      import_key_slh_dsa(scope, name, format, key_data, extractable, usages)
    }
    // Spec: any algorithm not recognized for `importKey` triggers
    // `normalizeAlgorithm` to throw `NotSupportedError: Unrecognized
    // algorithm name`. node_compat test-crypto-key-objects-to-crypto-key.js
    // and the WebCrypto WPT suite both rely on the exact message.
    _ => Err(not_supported("Unrecognized algorithm name".into())),
  }
}

fn import_key_rsa<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  hash_name: Option<&str>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  use crate::import_key::ImportKeyOptions;
  use crate::import_key::ImportKeyResult;
  use crate::import_key::KeyData;
  use crate::import_key::op_crypto_import_key_inner;
  let pub_usages: &[&str] = match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => &["verify"],
    "RSA-OAEP" => &["encrypt", "wrapKey"],
    _ => unreachable!(),
  };
  let priv_usages: &[&str] = match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => &["sign"],
    "RSA-OAEP" => &["decrypt", "unwrapKey"],
    _ => unreachable!(),
  };
  let jwk_use = match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => "sig",
    "RSA-OAEP" => "enc",
    _ => unreachable!(),
  };
  let hash = hash_name
    .ok_or_else(|| data_error("RSA import requires 'hash'".into()))?
    .to_string();
  let opts = match name {
    "RSASSA-PKCS1-v1_5" => ImportKeyOptions::RsassaPkcs1v15 {},
    "RSA-PSS" => ImportKeyOptions::RsaPss {},
    "RSA-OAEP" => ImportKeyOptions::RsaOaep {},
    _ => unreachable!(),
  };
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);

  let build = |scope: &mut v8::PinScope<'s, '_>,
               key_type: CryptoKeyType,
               modulus_length: u32,
               public_exponent: Vec<u8>,
               rkd: crate::shared::RustRawKeyData|
   -> v8::Local<'s, v8::Object> {
    let alg = AlgorithmDict::new(name)
      .with_hash(&hash)
      .with_modulus_length(modulus_length)
      .with_public_exponent(public_exponent);
    make_crypto_key(
      scope,
      key_type,
      extractable,
      &allowed_usages,
      alg,
      raw_key_data_to_raw(rkd),
    )
  };

  match (format, key_data) {
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, priv_usages)?;
      let result = op_crypto_import_key_inner(opts, KeyData::Pkcs8(b))
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
      let ImportKeyResult::Rsa {
        raw_data,
        modulus_length,
        public_exponent,
      } = result
      else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(build(
        scope,
        CryptoKeyType::Private,
        modulus_length as u32,
        public_exponent.as_ref().to_vec(),
        raw_data,
      ))
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, pub_usages)?;
      let result = op_crypto_import_key_inner(opts, KeyData::Spki(b))
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
      let ImportKeyResult::Rsa {
        raw_data,
        modulus_length,
        public_exponent,
      } = result
      else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(build(
        scope,
        CryptoKeyType::Public,
        modulus_length as u32,
        public_exponent.as_ref().to_vec(),
        raw_data,
      ))
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      let is_priv = read_string_member(scope, jwk, b"d").is_some();
      let want = if is_priv { priv_usages } else { pub_usages };
      check_usages_subset(usages, want)?;
      let kty_upper =
        read_string_member(scope, jwk, b"kty").map(|s| s.to_ascii_uppercase());
      if kty_upper.as_deref() != Some("RSA") {
        return Err(data_error(
          "'kty' property of JsonWebKey must be 'RSA'".into(),
        ));
      }
      if !usages.is_empty()
        && let Some(use_) = read_string_member(scope, jwk, b"use")
        && use_.to_ascii_lowercase() != jwk_use
      {
        return Err(data_error(format!(
          "'use' property of JsonWebKey must be '{jwk_use}'"
        )));
      }
      if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
        for u in &key_ops {
          if !ALL_USAGES.contains(&u.as_str()) {
            return Err(data_error(
              "'key_ops' property of JsonWebKey is invalid".into(),
            ));
          }
        }
        for u in usages {
          if !key_ops.iter().any(|k| k == u) {
            return Err(data_error(
              "'key_ops' property of JsonWebKey is invalid".into(),
            ));
          }
        }
      }
      if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
        return Err(data_error(
          "'ext' property of JsonWebKey must not be false if extractable is true"
            .into(),
        ));
      }
      if let Some(alg_s) = read_string_member(scope, jwk, b"alg") {
        let want_hash = rsa_jwk_alg_to_hash(name, &alg_s)?;
        if want_hash != hash {
          return Err(data_error(format!(
            "'alg' property of JsonWebKey must be '{name}': received {alg_s}"
          )));
        }
      }
      if is_priv {
        // Per spec, optimized private keys (with p, q, dp, dq, qi) are
        // required.
        let p = read_string_member(scope, jwk, b"p");
        let q = read_string_member(scope, jwk, b"q");
        let dp = read_string_member(scope, jwk, b"dp");
        let dq = read_string_member(scope, jwk, b"dq");
        let qi = read_string_member(scope, jwk, b"qi");
        let any_present = p.is_some()
          || q.is_some()
          || dp.is_some()
          || dq.is_some()
          || qi.is_some();
        if !any_present {
          return Err(not_supported(
            "Only optimized private keys are supported".into(),
          ));
        }
        if p.is_none() {
          return Err(data_error(
            "'p' property of JsonWebKey is required for private keys".into(),
          ));
        }
        if q.is_none() {
          return Err(data_error(
            "'q' property of JsonWebKey is required for private keys".into(),
          ));
        }
        if dp.is_none() {
          return Err(data_error(
            "'dp' property of JsonWebKey is required for private keys".into(),
          ));
        }
        if dq.is_none() {
          return Err(data_error(
            "'dq' property of JsonWebKey is required for private keys".into(),
          ));
        }
        if qi.is_none() {
          return Err(data_error(
            "'qi' property of JsonWebKey is required for private keys".into(),
          ));
        }
        if read_string_member(scope, jwk, b"oth").is_some() {
          return Err(not_supported(
            "'oth' property of JsonWebKey is not supported".into(),
          ));
        }
        let n = read_string_member(scope, jwk, b"n").ok_or_else(|| {
          data_error("'n' property of JsonWebKey is required".into())
        })?;
        let e = read_string_member(scope, jwk, b"e").ok_or_else(|| {
          data_error("'e' property of JsonWebKey is required".into())
        })?;
        let d = read_string_member(scope, jwk, b"d").unwrap();
        let result = op_crypto_import_key_inner(
          opts,
          KeyData::JwkPrivateRsa {
            n,
            e,
            d,
            p: p.unwrap(),
            q: q.unwrap(),
            dp: dp.unwrap(),
            dq: dq.unwrap(),
            qi: qi.unwrap(),
          },
        )
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
        let ImportKeyResult::Rsa {
          raw_data,
          modulus_length,
          public_exponent,
        } = result
        else {
          return Err(data_error("Invalid key data".into()));
        };
        Ok(build(
          scope,
          CryptoKeyType::Private,
          modulus_length as u32,
          public_exponent.as_ref().to_vec(),
          raw_data,
        ))
      } else {
        let n = read_string_member(scope, jwk, b"n").ok_or_else(|| {
          data_error(
            "'n' property of JsonWebKey is required for public keys".into(),
          )
        })?;
        let e = read_string_member(scope, jwk, b"e").ok_or_else(|| {
          data_error(
            "'e' property of JsonWebKey is required for public keys".into(),
          )
        })?;
        let result =
          op_crypto_import_key_inner(opts, KeyData::JwkPublicRsa { n, e })
            .map_err(|e| {
              CryptoError::Other(deno_error::JsErrorBox::from_err(e))
            })?;
        let ImportKeyResult::Rsa {
          raw_data,
          modulus_length,
          public_exponent,
        } = result
        else {
          return Err(data_error("Invalid key data".into()));
        };
        Ok(build(
          scope,
          CryptoKeyType::Public,
          modulus_length as u32,
          public_exponent.as_ref().to_vec(),
          raw_data,
        ))
      }
    }
    _ => Err(not_supported("Not implemented".into())),
  }
}

fn rsa_jwk_alg_to_hash(
  name: &str,
  alg: &str,
) -> Result<&'static str, CryptoError> {
  Ok(match (name, alg) {
    ("RSASSA-PKCS1-v1_5", "RS1") => "SHA-1",
    ("RSASSA-PKCS1-v1_5", "RS256") => "SHA-256",
    ("RSASSA-PKCS1-v1_5", "RS384") => "SHA-384",
    ("RSASSA-PKCS1-v1_5", "RS512") => "SHA-512",
    ("RSASSA-PKCS1-v1_5", "RS3-256") => "SHA3-256",
    ("RSASSA-PKCS1-v1_5", "RS3-384") => "SHA3-384",
    ("RSASSA-PKCS1-v1_5", "RS3-512") => "SHA3-512",
    ("RSA-PSS", "PS1") => "SHA-1",
    ("RSA-PSS", "PS256") => "SHA-256",
    ("RSA-PSS", "PS384") => "SHA-384",
    ("RSA-PSS", "PS512") => "SHA-512",
    ("RSA-PSS", "PS3-256") => "SHA3-256",
    ("RSA-PSS", "PS3-384") => "SHA3-384",
    ("RSA-PSS", "PS3-512") => "SHA3-512",
    ("RSA-OAEP", "RSA-OAEP") => "SHA-1",
    ("RSA-OAEP", "RSA-OAEP-256") => "SHA-256",
    ("RSA-OAEP", "RSA-OAEP-384") => "SHA-384",
    ("RSA-OAEP", "RSA-OAEP-512") => "SHA-512",
    ("RSA-OAEP", "RSA-OAEP3-256") => "SHA3-256",
    ("RSA-OAEP", "RSA-OAEP3-384") => "SHA3-384",
    ("RSA-OAEP", "RSA-OAEP3-512") => "SHA3-512",
    _ => {
      return Err(data_error(format!(
        "'alg' property of JsonWebKey unrecognized for {name}: {alg}"
      )));
    }
  })
}

fn import_key_ml_kem<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let variant = match name {
    "ML-KEM-512" => crate::mlkem::MlKemVariant::MlKem512,
    "ML-KEM-768" => crate::mlkem::MlKemVariant::MlKem768,
    "ML-KEM-1024" => crate::mlkem::MlKemVariant::MlKem1024,
    _ => unreachable!(),
  };
  let pub_usages: &[&str] = &["encapsulateKey", "encapsulateBits"];
  let priv_usages: &[&str] = &["decapsulateKey", "decapsulateBits"];
  let pub_size = variant.public_key_size();
  let alg = AlgorithmDict::new(name);
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);

  let make_public = |scope: &mut v8::PinScope<'s, '_>,
                     bytes: Vec<u8>|
   -> Result<v8::Local<'s, v8::Object>, CryptoError> {
    Ok(make_crypto_key(
      scope,
      CryptoKeyType::Public,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::Raw(bytes.into_boxed_slice()),
    ))
  };
  let make_private = |scope: &mut v8::PinScope<'s, '_>,
                      seed: Option<Vec<u8>>,
                      private_key: Vec<u8>|
   -> Result<v8::Local<'s, v8::Object>, CryptoError> {
    Ok(make_crypto_key(
      scope,
      CryptoKeyType::Private,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::SeededPrivate {
        seed: seed.map(|s| s.into_boxed_slice()),
        private_key: private_key.into_boxed_slice(),
      },
    ))
  };
  let _ = alg;
  match (format, key_data) {
    (KeyFormat::RawPublic, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, pub_usages)?;
      if b.len() != pub_size {
        return Err(data_error("Invalid key data".into()));
      }
      if !crate::mlkem::validate_public_key(variant, &b) {
        return Err(data_error("Invalid key data".into()));
      }
      make_public(scope, b)
    }
    (KeyFormat::RawSeed, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, priv_usages)?;
      if b.len() != 64 {
        return Err(data_error("Invalid key data".into()));
      }
      let res = crate::mlkem::from_seed(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      make_private(scope, Some(b.clone()), res.private_key)
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, pub_usages)?;
      let res = crate::mlkem::import_spki(&b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      if res.variant != variant {
        return Err(data_error("Imported key algorithm does not match".into()));
      }
      make_public(scope, res.public_key)
    }
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, priv_usages)?;
      let res =
        crate::mlkem::import_pkcs8_native(&b).map_err(|e| match e {
          crate::mlkem::MlKemError::UnsupportedPkcs8Format => not_supported(
            "ML-KEM 'expandedKey' PKCS#8 format is not supported; only the seed form is supported"
              .into(),
          ),
          _ => data_error("Invalid key data".into()),
        })?;
      if res.variant != variant {
        return Err(data_error("Imported key algorithm does not match".into()));
      }
      make_private(scope, Some(res.seed), res.private_key)
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      let wants_private = read_string_member(scope, jwk, b"priv").is_some();
      let expected = if wants_private {
        priv_usages
      } else {
        pub_usages
      };
      check_usages_subset(usages, expected)?;
      validate_jwk_akp(scope, jwk, name, "enc", usages, extractable)?;
      if wants_private {
        let priv_s = read_string_member(scope, jwk, b"priv").unwrap();
        let seed = BASE64_URL_SAFE_NO_PAD
          .decode(priv_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid private key data".into()))?;
        if seed.len() != 64 {
          return Err(data_error("Invalid private key data".into()));
        }
        let res = crate::mlkem::from_seed(variant, &seed)
          .map_err(|_| data_error("Invalid private key data".into()))?;
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let pub_bytes = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if pub_bytes != res.public_key {
          return Err(data_error("Invalid public key data".into()));
        }
        make_private(scope, Some(seed), res.private_key)
      } else {
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let pub_bytes = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if pub_bytes.len() != pub_size {
          return Err(data_error("Invalid public key data".into()));
        }
        if !crate::mlkem::validate_public_key(variant, &pub_bytes) {
          return Err(data_error("Invalid public key data".into()));
        }
        make_public(scope, pub_bytes)
      }
    }
    _ => Err(not_supported("Unsupported key format for ML-KEM".into())),
  }
}

fn import_key_ml_dsa<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let variant = match name {
    "ML-DSA-44" => 0u8,
    "ML-DSA-65" => 1,
    "ML-DSA-87" => 2,
    _ => unreachable!(),
  };
  let pub_len = mldsa_public_key_len(variant);
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  let make_public = |scope: &mut v8::PinScope<'s, '_>,
                     bytes: Vec<u8>|
   -> v8::Local<'s, v8::Object> {
    make_crypto_key(
      scope,
      CryptoKeyType::Public,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::Raw(bytes.into_boxed_slice()),
    )
  };
  let make_private = |scope: &mut v8::PinScope<'s, '_>,
                      seed: Option<Vec<u8>>,
                      private_key: Vec<u8>|
   -> v8::Local<'s, v8::Object> {
    make_crypto_key(
      scope,
      CryptoKeyType::Private,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::SeededPrivate {
        seed: seed.map(|s| s.into_boxed_slice()),
        private_key: private_key.into_boxed_slice(),
      },
    )
  };
  match (format, key_data) {
    (KeyFormat::RawSeed, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["sign"])?;
      if b.len() != 32 {
        return Err(data_error("Invalid key data".into()));
      }
      let res = crate::mldsa::from_seed(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      Ok(make_private(scope, Some(b.clone()), res.private_key))
    }
    (KeyFormat::RawPublic, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["verify"])?;
      if b.len() != pub_len {
        return Err(data_error("Invalid key data".into()));
      }
      Ok(make_public(scope, b))
    }
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["sign"])?;
      let res = crate::mldsa::from_pkcs8_native(variant, &b).map_err(|e| match e {
        crate::mldsa::MlDsaError::UnsupportedPkcs8Format => not_supported(
          "ML-DSA 'expandedKey' PKCS#8 format is not supported; only the seed form is supported"
            .into(),
        ),
        _ => data_error("Invalid key data".into()),
      })?;
      Ok(make_private(scope, res.seed, res.private_key))
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["verify"])?;
      let pub_bytes = crate::mldsa::from_spki(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      Ok(make_public(scope, pub_bytes))
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      let wants_private = read_string_member(scope, jwk, b"priv").is_some();
      let expected: &[&str] = if wants_private {
        &["sign"]
      } else {
        &["verify"]
      };
      check_usages_subset(usages, expected)?;
      validate_jwk_akp(scope, jwk, name, "sig", usages, extractable)?;
      if wants_private {
        let priv_s = read_string_member(scope, jwk, b"priv").unwrap();
        let seed = BASE64_URL_SAFE_NO_PAD
          .decode(priv_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid private key data".into()))?;
        if seed.len() != 32 {
          return Err(data_error("Invalid private key data".into()));
        }
        let res = crate::mldsa::from_seed(variant, &seed)
          .map_err(|_| data_error("Invalid private key data".into()))?;
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let pub_bytes = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if pub_bytes != res.public_key {
          return Err(data_error("Invalid public key data".into()));
        }
        Ok(make_private(scope, Some(seed), res.private_key))
      } else {
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let pub_bytes = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if pub_bytes.len() != pub_len {
          return Err(data_error("Invalid public key data".into()));
        }
        Ok(make_public(scope, pub_bytes))
      }
    }
    _ => Err(not_supported("Unsupported key format for ML-DSA".into())),
  }
}

fn import_key_slh_dsa<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let variant = crate::slhdsa::variant_from_name(name).unwrap();
  let params = crate::slhdsa::params(variant);
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  let make_public = |scope: &mut v8::PinScope<'s, '_>,
                     bytes: Vec<u8>|
   -> v8::Local<'s, v8::Object> {
    make_crypto_key(
      scope,
      CryptoKeyType::Public,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::Raw(bytes.into_boxed_slice()),
    )
  };
  let make_private = |scope: &mut v8::PinScope<'s, '_>,
                      private_key: Vec<u8>|
   -> v8::Local<'s, v8::Object> {
    make_crypto_key(
      scope,
      CryptoKeyType::Private,
      extractable,
      &allowed_usages,
      AlgorithmDict::new(name),
      RawKeyData::SeededPrivate {
        seed: None,
        private_key: private_key.into_boxed_slice(),
      },
    )
  };
  match (format, key_data) {
    (KeyFormat::RawPublic, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["verify"])?;
      if b.len() != params.public_key_len {
        return Err(data_error("Invalid key data".into()));
      }
      Ok(make_public(scope, b))
    }
    (KeyFormat::RawPrivate, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["sign"])?;
      if b.len() != params.private_key_len {
        return Err(data_error("Invalid key data".into()));
      }
      crate::slhdsa::public_from_private(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      Ok(make_private(scope, b))
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["verify"])?;
      let public_key = crate::slhdsa::import_spki(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      Ok(make_public(scope, public_key))
    }
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &["sign"])?;
      let private_key = crate::slhdsa::import_pkcs8(variant, &b)
        .map_err(|_| data_error("Invalid key data".into()))?;
      Ok(make_private(scope, private_key))
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      let wants_private = read_string_member(scope, jwk, b"priv").is_some();
      let expected: &[&str] = if wants_private {
        &["sign"]
      } else {
        &["verify"]
      };
      check_usages_subset(usages, expected)?;
      validate_jwk_akp(scope, jwk, name, "sig", usages, extractable)?;
      if wants_private {
        let priv_s = read_string_member(scope, jwk, b"priv").unwrap();
        let private_key = BASE64_URL_SAFE_NO_PAD
          .decode(priv_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid private key data".into()))?;
        if private_key.len() != params.private_key_len {
          return Err(data_error("Invalid private key data".into()));
        }
        let expected_public =
          crate::slhdsa::public_from_private(variant, &private_key)
            .map_err(|_| data_error("Invalid private key data".into()))?;
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let public_key = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if public_key != expected_public {
          return Err(data_error("Invalid public key data".into()));
        }
        Ok(make_private(scope, private_key))
      } else {
        let pub_s = read_string_member(scope, jwk, b"pub")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let public_key = BASE64_URL_SAFE_NO_PAD
          .decode(pub_s.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if public_key.len() != params.public_key_len {
          return Err(data_error("Invalid public key data".into()));
        }
        Ok(make_public(scope, public_key))
      }
    }
    _ => Err(not_supported("Unsupported key format for SLH-DSA".into())),
  }
}

fn mldsa_public_key_len(variant: u8) -> usize {
  match variant {
    0 => 1312,
    1 => 1952,
    2 => 2592,
    _ => 0,
  }
}

fn validate_jwk_akp<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  jwk: v8::Local<'s, v8::Object>,
  algorithm_name: &str,
  expected_use: &str,
  usages: &[String],
  extractable: bool,
) -> Result<(), CryptoError> {
  let kty = read_string_member(scope, jwk, b"kty");
  if kty.as_deref() != Some("AKP") {
    return Err(data_error("Invalid key type".into()));
  }
  let alg = read_string_member(scope, jwk, b"alg");
  if alg.as_deref() != Some(algorithm_name) {
    return Err(data_error("Invalid algorithm".into()));
  }
  if !usages.is_empty()
    && let Some(use_) = read_string_member(scope, jwk, b"use")
    && use_ != expected_use
  {
    return Err(data_error("Invalid key usage".into()));
  }
  if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
    for u in &key_ops {
      if !ALL_USAGES.contains(&u.as_str()) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
    for u in usages {
      if !key_ops.iter().any(|k| k == u) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
  }
  if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
    return Err(data_error("Invalid key extractability".into()));
  }
  Ok(())
}

fn import_key_ec<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  named_curve: Option<&str>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let named_curve = named_curve
    .ok_or_else(|| data_error("EC import requires 'namedCurve'".into()))?
    .to_string();
  if !matches!(named_curve.as_str(), "P-256" | "P-384" | "P-521") {
    return Err(data_error("Invalid namedCurve".into()));
  }
  let curve = match named_curve.as_str() {
    "P-256" => crate::shared::EcNamedCurve::P256,
    "P-384" => crate::shared::EcNamedCurve::P384,
    "P-521" => crate::shared::EcNamedCurve::P521,
    _ => unreachable!(),
  };
  let pub_usages = match name {
    "ECDSA" => vec!["verify"],
    "ECDH" => vec![],
    _ => unreachable!(),
  };
  let priv_usages = match name {
    "ECDSA" => vec!["sign"],
    "ECDH" => vec!["deriveKey", "deriveBits"],
    _ => unreachable!(),
  };
  let jwk_use = match name {
    "ECDSA" => "sig",
    "ECDH" => "enc",
    _ => unreachable!(),
  };

  let alg = AlgorithmDict::new(name).with_named_curve(&named_curve);
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);

  use crate::import_key::ImportKeyOptions;
  use crate::import_key::ImportKeyResult;
  use crate::import_key::KeyData;
  use crate::import_key::op_crypto_import_key_inner;

  let opts = match name {
    "ECDSA" => ImportKeyOptions::Ecdsa { named_curve: curve },
    "ECDH" => ImportKeyOptions::Ecdh { named_curve: curve },
    _ => unreachable!(),
  };

  match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawPublic, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &pub_usages)?;
      let result = op_crypto_import_key_inner(opts, KeyData::Raw(b))
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
      let ImportKeyResult::Ec { raw_data } = result else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        extractable,
        &allowed_usages,
        alg,
        raw_key_data_to_raw(raw_data),
      ))
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      if name == "ECDSA" {
        check_usages_subset(usages, &pub_usages)?;
      } else if !usages.is_empty() {
        return Err(syntax_error("Key usage must be empty".into()));
      }
      let result = op_crypto_import_key_inner(opts, KeyData::Spki(b))
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
      let ImportKeyResult::Ec { raw_data } = result else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        extractable,
        &allowed_usages,
        alg,
        raw_key_data_to_raw(raw_data),
      ))
    }
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, &priv_usages)?;
      let result = op_crypto_import_key_inner(opts, KeyData::Pkcs8(b))
        .map_err(|e| CryptoError::Other(deno_error::JsErrorBox::from_err(e)))?;
      let ImportKeyResult::Ec { raw_data } = result else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Private,
        extractable,
        &allowed_usages,
        alg,
        raw_key_data_to_raw(raw_data),
      ))
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      let is_priv = read_string_member(scope, jwk, b"d").is_some();
      let usages_set = if is_priv { &priv_usages } else { &pub_usages };
      check_usages_subset(usages, usages_set)?;
      if read_string_member(scope, jwk, b"kty").as_deref() != Some("EC") {
        return Err(data_error(
          "'kty' property of JsonWebKey must be 'EC'".into(),
        ));
      }
      if !usages.is_empty()
        && let Some(use_) = read_string_member(scope, jwk, b"use")
        && use_ != jwk_use
      {
        return Err(data_error(format!(
          "'use' property of JsonWebKey must be '{jwk_use}'"
        )));
      }
      if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
        for u in &key_ops {
          if !ALL_USAGES.contains(&u.as_str()) {
            return Err(data_error(
              "'key_ops' member of JsonWebKey is invalid".into(),
            ));
          }
        }
        for u in usages {
          if !key_ops.iter().any(|k| k == u) {
            return Err(data_error(
              "'key_ops' member of JsonWebKey is invalid".into(),
            ));
          }
        }
      }
      if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
        return Err(data_error(
          "'ext' property of JsonWebKey must not be false if extractable is true"
            .into(),
        ));
      }
      if name == "ECDSA"
        && let Some(alg_s) = read_string_member(scope, jwk, b"alg")
      {
        let want = match alg_s.as_str() {
          "ES256" => "P-256",
          "ES384" => "P-384",
          "ES512" => "P-521",
          _ => {
            return Err(data_error("Curve algorithm not supported".into()));
          }
        };
        if want != named_curve {
          return Err(data_error("Mismatched curve algorithm".into()));
        }
      }
      let x = read_string_member(scope, jwk, b"x").ok_or_else(|| {
        data_error("'x' property of JsonWebKey is required for EC keys".into())
      })?;
      let y = read_string_member(scope, jwk, b"y").ok_or_else(|| {
        data_error("'y' property of JsonWebKey is required for EC keys".into())
      })?;
      let key_type = if is_priv {
        CryptoKeyType::Private
      } else {
        CryptoKeyType::Public
      };
      let result = if is_priv {
        let d = read_string_member(scope, jwk, b"d").unwrap();
        op_crypto_import_key_inner(opts, KeyData::JwkPrivateEc { x, y, d })
          .map_err(|e| {
            CryptoError::Other(deno_error::JsErrorBox::from_err(e))
          })?
      } else {
        op_crypto_import_key_inner(opts, KeyData::JwkPublicEc { x, y })
          .map_err(|e| {
            CryptoError::Other(deno_error::JsErrorBox::from_err(e))
          })?
      };
      let ImportKeyResult::Ec { raw_data } = result else {
        return Err(data_error("Invalid key data".into()));
      };
      Ok(make_crypto_key(
        scope,
        key_type,
        extractable,
        &allowed_usages,
        alg,
        raw_key_data_to_raw(raw_data),
      ))
    }
    _ => Err(not_supported("Not implemented".into())),
  }
}

fn raw_key_data_to_raw(rkd: crate::shared::RustRawKeyData) -> RawKeyData {
  use crate::shared::RustRawKeyData;
  match rkd {
    RustRawKeyData::Public(b) => {
      RawKeyData::Public(b.as_ref().to_vec().into_boxed_slice())
    }
    RustRawKeyData::Private(b) => {
      RawKeyData::Private(b.as_ref().to_vec().into_boxed_slice())
    }
    RustRawKeyData::Secret(b) => {
      RawKeyData::Secret(b.as_ref().to_vec().into_boxed_slice())
    }
  }
}

#[derive(Copy, Clone)]
enum OkpKind {
  Ed25519,
  X25519,
  X448,
}

impl OkpKind {
  fn name(self) -> &'static str {
    match self {
      Self::Ed25519 => "Ed25519",
      Self::X25519 => "X25519",
      Self::X448 => "X448",
    }
  }
  fn key_len(self) -> usize {
    match self {
      Self::Ed25519 => 32,
      Self::X25519 => 32,
      Self::X448 => 56,
    }
  }
  fn pub_usages(self) -> &'static [&'static str] {
    match self {
      Self::Ed25519 => &["verify"],
      Self::X25519 | Self::X448 => &[],
    }
  }
  fn priv_usages(self) -> &'static [&'static str] {
    match self {
      Self::Ed25519 => &["sign"],
      Self::X25519 | Self::X448 => &["deriveKey", "deriveBits"],
    }
  }
  fn jwk_use(self) -> &'static str {
    match self {
      Self::Ed25519 => "sig",
      Self::X25519 | Self::X448 => "enc",
    }
  }
  fn import_spki(self, key_data: &[u8], out: &mut [u8]) -> bool {
    let oid = match self {
      Self::Ed25519 => crate::ed25519::ED25519_OID,
      Self::X25519 => crate::x25519::X25519_OID,
      Self::X448 => crate::x448::X448_OID,
    };
    let Ok(info) = spki::SubjectPublicKeyInfoRef::try_from(key_data) else {
      return false;
    };
    if info.algorithm.oid != oid || info.algorithm.parameters.is_some() {
      return false;
    }
    let bytes = info.subject_public_key.raw_bytes();
    if bytes.len() != out.len() {
      return false;
    }
    out.copy_from_slice(bytes);
    true
  }
  fn import_pkcs8(self, key_data: &[u8], out: &mut [u8]) -> bool {
    use elliptic_curve::pkcs8::PrivateKeyInfo;
    use elliptic_curve::pkcs8::der::Decode;
    let oid = match self {
      Self::Ed25519 => crate::ed25519::ED25519_OID,
      Self::X25519 => crate::x25519::X25519_OID,
      Self::X448 => crate::x448::X448_OID,
    };
    let Ok(pk_info) = PrivateKeyInfo::from_der(key_data) else {
      return false;
    };
    if pk_info.algorithm.oid != oid || pk_info.algorithm.parameters.is_some() {
      return false;
    }
    // CurvePrivateKey ::= OCTET STRING; the wrapper is the 2-byte DER
    // octet-string header followed by the raw private key bytes.
    let want_inner = out.len();
    let want_total = want_inner + 2;
    if pk_info.private_key.len() != want_total {
      return false;
    }
    out.copy_from_slice(&pk_info.private_key[2..]);
    true
  }
}

fn import_key_okp<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  kind: OkpKind,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let alg = AlgorithmDict::new(kind.name());
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawPublic, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, kind.pub_usages())?;
      if b.len() != kind.key_len() {
        return Err(data_error("Invalid key data".into()));
      }
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        extractable,
        &allowed_usages,
        alg,
        RawKeyData::Raw(b.into_boxed_slice()),
      ))
    }
    (KeyFormat::Spki, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, kind.pub_usages())?;
      let mut out = vec![0u8; kind.key_len()];
      if !kind.import_spki(&b, &mut out) {
        return Err(data_error("Invalid key data".into()));
      }
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        extractable,
        &allowed_usages,
        alg,
        RawKeyData::Raw(out.into_boxed_slice()),
      ))
    }
    (KeyFormat::Pkcs8, ImportKeyData::Buffer(b)) => {
      check_usages_subset(usages, kind.priv_usages())?;
      let mut out = vec![0u8; kind.key_len()];
      if !kind.import_pkcs8(&b, &mut out) {
        return Err(data_error("Invalid key data".into()));
      }
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Private,
        extractable,
        &allowed_usages,
        alg,
        RawKeyData::Raw(out.into_boxed_slice()),
      ))
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      // RFC 8037: kty=OKP, crv=<curve name>.
      let kty = read_string_member(scope, jwk, b"kty");
      if kty.as_deref() != Some("OKP") {
        return Err(data_error("Invalid key type".into()));
      }
      let crv = read_string_member(scope, jwk, b"crv");
      if crv.as_deref() != Some(kind.name()) {
        return Err(data_error("Invalid curve".into()));
      }
      if !usages.is_empty()
        && let Some(use_) = read_string_member(scope, jwk, b"use")
        && use_ != kind.jwk_use()
      {
        return Err(data_error("Invalid key use".into()));
      }
      if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
        for u in &key_ops {
          if !ALL_USAGES.contains(&u.as_str()) {
            return Err(data_error(
              "'key_ops' property of JsonWebKey is invalid".into(),
            ));
          }
        }
        for u in usages {
          if !key_ops.iter().any(|k| k == u) {
            return Err(data_error(
              "'key_ops' property of JsonWebKey is invalid".into(),
            ));
          }
        }
      }
      if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
        return Err(data_error("Invalid key extractability".into()));
      }
      if let Some(d) = read_string_member(scope, jwk, b"d") {
        check_usages_subset(usages, kind.priv_usages())?;
        let bytes = BASE64_URL_SAFE_NO_PAD
          .decode(d.trim_end_matches('='))
          .map_err(|_| data_error("Invalid private key data".into()))?;
        if bytes.len() != kind.key_len() {
          return Err(data_error("Invalid private key data".into()));
        }
        Ok(make_crypto_key(
          scope,
          CryptoKeyType::Private,
          extractable,
          &allowed_usages,
          alg,
          RawKeyData::Raw(bytes.into_boxed_slice()),
        ))
      } else {
        if !usages.is_empty() {
          check_usages_subset(usages, kind.pub_usages())?;
        }
        let x = read_string_member(scope, jwk, b"x")
          .ok_or_else(|| data_error("Invalid public key data".into()))?;
        let bytes = BASE64_URL_SAFE_NO_PAD
          .decode(x.trim_end_matches('='))
          .map_err(|_| data_error("Invalid public key data".into()))?;
        if bytes.len() != kind.key_len() {
          return Err(data_error("Invalid public key data".into()));
        }
        Ok(make_crypto_key(
          scope,
          CryptoKeyType::Public,
          extractable,
          &allowed_usages,
          alg,
          RawKeyData::Raw(bytes.into_boxed_slice()),
        ))
      }
    }
    _ => Err(not_supported("Not implemented".into())),
  }
}

fn import_key_aes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  algorithm_name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
  supported_usages: &[&str],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  check_usages_subset(usages, supported_usages)?;
  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => {
      let bits = b.len() * 8;
      if !matches!(bits, 128 | 192 | 256) {
        return Err(data_error("Invalid key length".into()));
      }
      b
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "enc", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k").ok_or_else(|| {
        data_error("'k' property of JsonWebKey is required".into())
      })?;
      let bits = bytes.len() * 8;
      let expected = aes_jwk_alg(algorithm_name, bits)
        .ok_or_else(|| data_error("Invalid key length".into()))?;
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != expected
      {
        return Err(data_error(format!("Invalid algorithm: {alg}")));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let bits = (data.len() * 8) as u32;
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new(algorithm_name).with_length(bits),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_chacha20<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let supported = &["encrypt", "decrypt", "wrapKey", "unwrapKey"];
  check_usages_subset(usages, supported)?;
  let data = match (format, key_data) {
    (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => {
      if b.len() != 32 {
        return Err(data_error(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key".into(),
        ));
      }
      b
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "enc", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k").ok_or_else(|| {
        data_error("'k' property of JsonWebKey is required".into())
      })?;
      if bytes.len() != 32 {
        return Err(data_error(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key".into(),
        ));
      }
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != "C20P"
      {
        return Err(data_error(format!("Invalid algorithm: {alg}")));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new("ChaCha20-Poly1305"),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_hmac<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  hash_name: Option<&str>,
  length_override: Option<u32>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  check_usages_subset(usages, &["sign", "verify"])?;
  let hash_name = hash_name
    .map(str::to_string)
    .ok_or_else(|| data_error("HMAC import requires 'hash'".into()))?;

  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => b,
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "sig", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k").ok_or_else(|| {
        data_error("'k' property of JsonWebKey is required".into())
      })?;
      let expected = hmac_jwk_alg(&hash_name)
        .ok_or_else(|| not_supported("Hash algorithm not supported".into()))?;
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != expected
      {
        return Err(data_error(format!(
          "'alg' property of JsonWebKey must be '{expected}'"
        )));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let mut length = (data.len() * 8) as u32;
  if length == 0 {
    return Err(data_error("Key length is zero".into()));
  }
  if let Some(override_) = length_override {
    if override_ > length || override_ <= length.saturating_sub(8) {
      return Err(data_error("Key length is invalid".into()));
    }
    length = override_;
  }
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new("HMAC")
      .with_length(length)
      .with_hash(hash_name),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_kmac<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  length: Option<u32>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  check_usages_subset(usages, &["sign", "verify"])?;
  let data = match (format, key_data) {
    (KeyFormat::RawSecret | KeyFormat::Raw, ImportKeyData::Buffer(b)) => b,
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk = v8::Local::new(scope, &jwk_g);
      if read_string_member(scope, jwk, b"kty").as_deref() != Some("oct") {
        return Err(data_error("Invalid key type".into()));
      }
      let expected_alg = if name == "KMAC128" { "K128" } else { "K256" };
      if let Some(alg) = read_string_member(scope, jwk, b"alg")
        && alg != expected_alg
      {
        return Err(data_error("Invalid JWK alg".into()));
      }
      if !usages.is_empty()
        && let Some(use_) = read_string_member(scope, jwk, b"use")
        && use_ != "sig"
      {
        return Err(data_error("Invalid JWK use".into()));
      }
      if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
        for u in &key_ops {
          if !ALL_USAGES.contains(&u.as_str()) {
            return Err(data_error("Invalid JWK key_ops".into()));
          }
        }
        for u in usages {
          if !key_ops.iter().any(|k| k == u) {
            return Err(data_error("Invalid JWK key_ops".into()));
          }
        }
      }
      if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
        return Err(data_error("Invalid JWK ext".into()));
      }
      let k = read_string_member(scope, jwk, b"k")
        .ok_or_else(|| data_error("Missing JWK key data".into()))?;
      BASE64_JWK_FORGIVING
        .decode(k)
        .map_err(|_| data_error("Invalid JWK key data".into()))?
    }
    _ => return Err(not_supported("Unsupported key format for KMAC".into())),
  };
  let data_len_bits = (data.len() * 8) as u32;
  let length = if let Some(length) = length {
    if length > data_len_bits || length <= data_len_bits.saturating_sub(8) {
      return Err(data_error("Invalid KMAC key length".into()));
    }
    length
  } else {
    data_len_bits
  };
  if length == 0 || !length.is_multiple_of(8) {
    return Err(data_error("Invalid KMAC key length".into()));
  }
  let bytes = data[..(length / 8) as usize].to_vec();
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new(name).with_length(length),
    RawKeyData::Secret(bytes.into_boxed_slice()),
  ))
}

fn import_key_kdf<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  _extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  // 17.4 / 19.4: HKDF and PBKDF2 only accept "raw" / "raw-secret"; "jwk" is
  // not in the recognized formats.
  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => b,
    _ => return Err(not_supported("Not implemented".into())),
  };
  check_usages_subset(usages, &["deriveKey", "deriveBits"])?;
  if usages.is_empty() {
    return Err(syntax_error("Invalid key usage".into()));
  }
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  // Per spec, HKDF/PBKDF2 imported keys must not be extractable. The JS
  // caller's `extractable` argument is ignored.
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    false,
    &allowed_usages,
    AlgorithmDict::new(name),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn aes_jwk_alg(algorithm_name: &str, bits: usize) -> Option<&'static str> {
  let suffix = match bits {
    128 => "128",
    192 => "192",
    256 => "256",
    _ => return None,
  };
  let kind = match algorithm_name {
    "AES-CTR" => "CTR",
    "AES-CBC" => "CBC",
    "AES-GCM" => "GCM",
    "AES-KW" => "KW",
    "AES-OCB" => "OCB",
    _ => return None,
  };
  Some(match (suffix, kind) {
    ("128", "CTR") => "A128CTR",
    ("192", "CTR") => "A192CTR",
    ("256", "CTR") => "A256CTR",
    ("128", "CBC") => "A128CBC",
    ("192", "CBC") => "A192CBC",
    ("256", "CBC") => "A256CBC",
    ("128", "GCM") => "A128GCM",
    ("192", "GCM") => "A192GCM",
    ("256", "GCM") => "A256GCM",
    ("128", "KW") => "A128KW",
    ("192", "KW") => "A192KW",
    ("256", "KW") => "A256KW",
    ("128", "OCB") => "A128OCB",
    ("192", "OCB") => "A192OCB",
    ("256", "OCB") => "A256OCB",
    _ => return None,
  })
}

fn hmac_jwk_alg(hash_name: &str) -> Option<&'static str> {
  Some(match hash_name {
    "SHA-1" => "HS1",
    "SHA-256" => "HS256",
    "SHA-384" => "HS384",
    "SHA-512" => "HS512",
    "SHA3-256" => "HS3-256",
    "SHA3-384" => "HS3-384",
    "SHA3-512" => "HS3-512",
    _ => return None,
  })
}

pub fn check_usages_subset(
  usages: &[String],
  allowed: &[&str],
) -> Result<(), CryptoError> {
  for u in usages {
    if !allowed.contains(&u.as_str()) {
      return Err(syntax_error("Invalid key usage".into()));
    }
  }
  Ok(())
}

pub fn filter_usages<'a>(
  usages: &'a [String],
  allowed: &[&str],
) -> Vec<&'a str> {
  usages
    .iter()
    .map(String::as_str)
    .filter(|u| allowed.contains(u))
    .collect()
}

fn validate_jwk_oct<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  jwk: v8::Local<'s, v8::Object>,
  expected_use: &str,
  usages: &[String],
  extractable: bool,
) -> Result<(), CryptoError> {
  let kty = read_string_member(scope, jwk, b"kty");
  if kty.as_deref() != Some("oct") {
    return Err(data_error(
      "'kty' property of JsonWebKey must be 'oct'".into(),
    ));
  }
  if read_string_member(scope, jwk, b"k").is_none() {
    return Err(data_error(
      "'k' property of JsonWebKey must be present".into(),
    ));
  }
  if !usages.is_empty()
    && let Some(use_) = read_string_member(scope, jwk, b"use")
    && use_ != expected_use
  {
    return Err(data_error(format!(
      "'use' property of JsonWebKey must be '{expected_use}'"
    )));
  }
  if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
    for u in &key_ops {
      if !ALL_USAGES.contains(&u.as_str()) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
    for u in usages {
      if !key_ops.iter().any(|k| k == u) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
  }
  if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
    return Err(data_error(
      "'ext' property of JsonWebKey must not be false if extractable is true"
        .into(),
    ));
  }
  Ok(())
}

fn read_jwk_b64_field<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let s = read_string_member(scope, obj, field)?;
  BASE64_JWK_FORGIVING.decode(s).ok()
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
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let s = val.to_string(scope)?;
  Some(s.to_rust_string_lossy(scope))
}

fn read_bool_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<bool> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  Some(val.boolean_value(scope))
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
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  val.uint32_value(scope)
}

fn read_string_array_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<String>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let arr = v8::Local::<v8::Array>::try_from(val).ok()?;
  let len = arr.length();
  let mut out = Vec::with_capacity(len as usize);
  for i in 0..len {
    let item = arr.get_index(scope, i)?;
    let s = item.to_string(scope)?;
    out.push(s.to_rust_string_lossy(scope));
  }
  Some(out)
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
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if val.is_string() {
    return Some(val.to_rust_string_lossy(scope));
  }
  let hash_obj = v8::Local::<v8::Object>::try_from(val).ok()?;
  let name_key = v8::String::new_from_one_byte(
    scope,
    b"name",
    v8::NewStringType::Internalized,
  )?;
  let name_val = hash_obj.get(scope, name_key.into())?;
  Some(name_val.to_string(scope)?.to_rust_string_lossy(scope))
}

pub fn data_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionDataError", msg))
}

pub fn syntax_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionSyntaxError", msg))
}

pub fn not_supported(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionNotSupportedError", msg))
}

/// Stamp the standard `WebIdlErrorKind::ConvertToConverterType` shape onto
/// a converter failure. Saves a few lines per converter that needs it.
#[allow(dead_code, reason = "convenience for upcoming converters")]
pub fn convert_error<'b>(
  prefix: Cow<'static, str>,
  context: ContextFn<'b>,
  ty: &'static str,
) -> WebIdlError {
  WebIdlError::new(prefix, context, WebIdlErrorKind::ConvertToConverterType(ty))
}
