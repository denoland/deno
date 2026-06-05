// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust port of the WebCrypto algorithm-parameter WebIDL dictionaries and the
//! `normalizeAlgorithm` abstract operation that used to live in
//! `ext/crypto/00_crypto.js`.
//!
//! See <https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm>.
//!
//! The WebIDL dictionaries are modelled with `#[derive(WebIDL)]
//! #[webidl(dictionary)]` and the per-op `supportedAlgorithms` registry plus
//! the `simpleAlgorithmDictionaries` BufferSource/HashAlgorithmIdentifier
//! member handling are reproduced exactly. The single entry point is the
//! `op_crypto_normalize_algorithm` op, which takes the op name + the raw
//! algorithm value and returns the normalized algorithm as a JS object with the
//! same shape the JS `normalizeAlgorithm` produced (so all the downstream
//! `SubtleCrypto` JS dispatch keeps reading `normalizedAlgorithm.hash.name`,
//! `.iv`, `.saltLength`, etc.).

use std::borrow::Cow;

use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NormalizeAlgorithmError {
  #[class(inherit)]
  #[error(transparent)]
  WebIdl(#[from] WebIdlError),
  #[class("DOMExceptionNotSupportedError")]
  #[error("Unrecognized algorithm name")]
  UnrecognizedAlgorithm,
}

const PREFIX: &str = "Failed to normalize algorithm";

fn context() -> Cow<'static, str> {
  "passed algorithm".into()
}

/// A `BufferSource` dictionary member: accepts `ArrayBuffer` / `ArrayBufferView`
/// (typed array or `DataView`) and copies the bytes out (matching the JS
/// `copyBuffer`). Stored as an owned `Vec<u8>`.
pub struct BufferSource(pub Vec<u8>);

impl<'a> WebIdlConverter<'a> for BufferSource {
  type Options = ();

  fn convert<'b, 'i>(
    _scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    // ArrayBufferView (TypedArray or DataView).
    if value.is_array_buffer_view() {
      let view: v8::Local<v8::ArrayBufferView> = value.try_into().unwrap();
      let len = view.byte_length();
      let mut buf = vec![0u8; len];
      view.copy_contents(&mut buf);
      return Ok(BufferSource(buf));
    }
    // ArrayBuffer.
    if value.is_array_buffer() {
      let ab: v8::Local<v8::ArrayBuffer> = value.try_into().unwrap();
      let len = ab.byte_length();
      let mut buf = vec![0u8; len];
      if let Some(data) = ab.data() {
        // SAFETY: `data` points to `len` initialized bytes owned by the
        // ArrayBuffer; we copy them into our freshly-allocated buffer.
        unsafe {
          std::ptr::copy_nonoverlapping(
            data.as_ptr() as *const u8,
            buf.as_mut_ptr(),
            len,
          );
        }
      }
      return Ok(BufferSource(buf));
    }
    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType("BufferSource"),
    ))
  }
}

/// `HashAlgorithmIdentifier`: a string or `{ name }` object naming a digest
/// algorithm. Normalized against the `digest` registry (case-insensitive,
/// canonicalized). Produced back to JS as a `{ name }` object — matching the JS
/// recursion `normalizeAlgorithm(value, "digest")`.
pub struct HashAlgorithmIdentifier(pub String);

const DIGEST_ALGORITHMS: &[&str] = &[
  "SHA-1",
  "SHA-256",
  "SHA-384",
  "SHA-512",
  "SHA3-256",
  "SHA3-384",
  "SHA3-512",
  "SHAKE128",
  "SHAKE256",
  "cSHAKE128",
  "cSHAKE256",
  "TurboSHAKE128",
  "TurboSHAKE256",
];

fn extract_name<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  prefix: Cow<'static, str>,
  ctx: ContextFn<'_>,
) -> Result<String, WebIdlError> {
  if value.is_string() {
    return String::convert(scope, value, prefix, ctx, &Default::default());
  }
  // Object: read the (required) `name` member as a DOMString.
  let obj: v8::Local<v8::Object> = value.try_into().map_err(|_| {
    WebIdlError::new(
      prefix.clone(),
      ctx.borrowed(),
      WebIdlErrorKind::ConvertToConverterType("dictionary"),
    )
  })?;
  let key = v8::String::new(scope, "name").unwrap();
  match obj.get(scope, key.into()) {
    Some(v) if !v.is_undefined() => {
      String::convert(scope, v, prefix, ctx, &Default::default())
    }
    _ => Err(WebIdlError::new(
      prefix,
      ctx,
      WebIdlErrorKind::DictionaryCannotConvertKey {
        converter: "Algorithm",
        key: "name",
      },
    )),
  }
}

impl<'a> WebIdlConverter<'a> for HashAlgorithmIdentifier {
  type Options = ();

  fn convert<'b, 'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let name = extract_name(scope, value, prefix, context)?;
    match DIGEST_ALGORITHMS
      .iter()
      .copied()
      .find(|n| n.eq_ignore_ascii_case(&name))
    {
      Some(canonical) => Ok(HashAlgorithmIdentifier(canonical.to_string())),
      // Matches `normalizeAlgorithm(value, "digest")` throwing
      // NotSupportedError for an unknown hash. WebIdlError can't carry a
      // DOMException class, so we surface a generic conversion error here; the
      // hash members in practice are always valid SHA-family names, and an
      // invalid hash is independently rejected downstream.
      None => Err(WebIdlError::new(
        PREFIX.into(),
        context_fn(),
        WebIdlErrorKind::ConvertToConverterType("HashAlgorithmIdentifier"),
      )),
    }
  }
}

fn context_fn<'a>() -> ContextFn<'a> {
  ContextFn::new(context)
}

// ===== Dictionary structs =====
// Field names auto-convert to camelCase JS member names via the derive.

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct Algorithm {
  pub name: String,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct RsaHashedKeyGenParams {
  #[options(enforce_range = true)]
  pub modulus_length: u32,
  pub public_exponent: BufferSource,
  pub hash: HashAlgorithmIdentifier,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct RsaHashedImportParams {
  pub hash: HashAlgorithmIdentifier,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct RsaOaepParams {
  pub label: Option<BufferSource>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct RsaPssParams {
  #[options(enforce_range = true)]
  pub salt_length: u32,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct EcKeyGenParams {
  pub named_curve: String,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct EcKeyImportParams {
  pub named_curve: String,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct EcdsaParams {
  pub hash: HashAlgorithmIdentifier,
}

// EcdhKeyDeriveParams's only member, `public`, is a CryptoKey (a JS interface
// object). It is read directly off the raw value in `build_output` so the JS
// object identity is preserved (no Rust round trip).

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct AesKeyGenParams {
  #[options(enforce_range = true)]
  pub length: u16,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct AesDerivedKeyParams {
  #[options(enforce_range = true)]
  pub length: u32,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct AesCtrParams {
  pub counter: BufferSource,
  #[options(enforce_range = true)]
  pub length: u16,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct AesCbcParams {
  pub iv: BufferSource,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct AesGcmParams {
  pub iv: BufferSource,
  #[options(enforce_range = true)]
  pub tag_length: Option<u32>,
  pub additional_data: Option<BufferSource>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct HmacKeyGenParams {
  pub hash: HashAlgorithmIdentifier,
  #[options(enforce_range = true)]
  pub length: Option<u32>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct HmacImportParams {
  pub hash: HashAlgorithmIdentifier,
  #[options(enforce_range = true)]
  pub length: Option<u32>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct HkdfParams {
  pub hash: HashAlgorithmIdentifier,
  pub salt: BufferSource,
  pub info: BufferSource,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct Pbkdf2Params {
  pub hash: HashAlgorithmIdentifier,
  #[options(enforce_range = true)]
  pub iterations: u32,
  pub salt: BufferSource,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct ShakeParams {
  #[options(enforce_range = true)]
  pub output_length: u32,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct CShakeParams {
  #[options(enforce_range = true)]
  pub output_length: u32,
  pub function_name: Option<BufferSource>,
  pub customization: Option<BufferSource>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct TurboShakeParams {
  #[options(enforce_range = true)]
  pub output_length: u32,
  #[options(enforce_range = true)]
  pub domain_separation: Option<u8>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct MlDsaParams {
  pub context: Option<BufferSource>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct ChaCha20Poly1305Params {
  // Per https://wicg.github.io/webcrypto-modern-algos AEAD params use `iv`.
  pub iv: BufferSource,
  pub additional_data: Option<BufferSource>,
}

// ===== supportedAlgorithms registry =====
//
// `(canonical name, dictionary type)`. `None` means no params dictionary (the
// JS `null` fast-path returning `{ name }`).

#[derive(Clone, Copy, PartialEq, Eq)]
enum DictType {
  None,
  RsaHashedKeyGenParams,
  RsaHashedImportParams,
  RsaOaepParams,
  RsaPssParams,
  EcKeyGenParams,
  EcKeyImportParams,
  EcdsaParams,
  EcdhKeyDeriveParams,
  AesKeyGenParams,
  AesDerivedKeyParams,
  AesCtrParams,
  AesCbcParams,
  AesGcmParams,
  HmacKeyGenParams,
  HmacImportParams,
  HkdfParams,
  Pbkdf2Params,
  ShakeParams,
  CShakeParams,
  TurboShakeParams,
  MlDsaParams,
  ChaCha20Poly1305Params,
}

use DictType as D;

fn registry(op: &str) -> Option<&'static [(&'static str, DictType)]> {
  Some(match op {
    "digest" => &[
      ("SHA-1", D::None),
      ("SHA-256", D::None),
      ("SHA-384", D::None),
      ("SHA-512", D::None),
      ("SHA3-256", D::None),
      ("SHA3-384", D::None),
      ("SHA3-512", D::None),
      ("SHAKE128", D::ShakeParams),
      ("SHAKE256", D::ShakeParams),
      ("cSHAKE128", D::CShakeParams),
      ("cSHAKE256", D::CShakeParams),
      ("TurboSHAKE128", D::TurboShakeParams),
      ("TurboSHAKE256", D::TurboShakeParams),
    ],
    "generateKey" => &[
      ("RSASSA-PKCS1-v1_5", D::RsaHashedKeyGenParams),
      ("RSA-PSS", D::RsaHashedKeyGenParams),
      ("RSA-OAEP", D::RsaHashedKeyGenParams),
      ("ECDSA", D::EcKeyGenParams),
      ("ECDH", D::EcKeyGenParams),
      ("AES-CTR", D::AesKeyGenParams),
      ("AES-CBC", D::AesKeyGenParams),
      ("AES-GCM", D::AesKeyGenParams),
      ("AES-OCB", D::AesKeyGenParams),
      ("AES-KW", D::AesKeyGenParams),
      ("HMAC", D::HmacKeyGenParams),
      ("ChaCha20-Poly1305", D::None),
      ("X25519", D::None),
      ("X448", D::None),
      ("Ed25519", D::None),
      ("ML-KEM-512", D::None),
      ("ML-KEM-768", D::None),
      ("ML-KEM-1024", D::None),
      ("ML-DSA-44", D::None),
      ("ML-DSA-65", D::None),
      ("ML-DSA-87", D::None),
    ],
    "sign" => &[
      ("RSASSA-PKCS1-v1_5", D::None),
      ("RSA-PSS", D::RsaPssParams),
      ("ECDSA", D::EcdsaParams),
      ("HMAC", D::None),
      ("Ed25519", D::None),
      ("ML-DSA-44", D::MlDsaParams),
      ("ML-DSA-65", D::MlDsaParams),
      ("ML-DSA-87", D::MlDsaParams),
    ],
    "verify" => &[
      ("RSASSA-PKCS1-v1_5", D::None),
      ("RSA-PSS", D::RsaPssParams),
      ("ECDSA", D::EcdsaParams),
      ("HMAC", D::None),
      ("Ed25519", D::None),
      ("ML-DSA-44", D::MlDsaParams),
      ("ML-DSA-65", D::MlDsaParams),
      ("ML-DSA-87", D::MlDsaParams),
    ],
    "importKey" => &[
      ("RSASSA-PKCS1-v1_5", D::RsaHashedImportParams),
      ("RSA-PSS", D::RsaHashedImportParams),
      ("RSA-OAEP", D::RsaHashedImportParams),
      ("ECDSA", D::EcKeyImportParams),
      ("ECDH", D::EcKeyImportParams),
      ("HMAC", D::HmacImportParams),
      ("HKDF", D::None),
      ("PBKDF2", D::None),
      ("AES-CTR", D::None),
      ("AES-CBC", D::None),
      ("AES-GCM", D::None),
      ("AES-OCB", D::None),
      ("AES-KW", D::None),
      ("ChaCha20-Poly1305", D::None),
      ("Ed25519", D::None),
      ("X25519", D::None),
      ("X448", D::None),
      ("ML-KEM-512", D::None),
      ("ML-KEM-768", D::None),
      ("ML-KEM-1024", D::None),
      ("ML-DSA-44", D::None),
      ("ML-DSA-65", D::None),
      ("ML-DSA-87", D::None),
    ],
    "encapsulate" => &[
      ("ML-KEM-512", D::None),
      ("ML-KEM-768", D::None),
      ("ML-KEM-1024", D::None),
    ],
    "decapsulate" => &[
      ("ML-KEM-512", D::None),
      ("ML-KEM-768", D::None),
      ("ML-KEM-1024", D::None),
    ],
    "deriveBits" => &[
      ("HKDF", D::HkdfParams),
      ("PBKDF2", D::Pbkdf2Params),
      ("ECDH", D::EcdhKeyDeriveParams),
      ("X25519", D::EcdhKeyDeriveParams),
      ("X448", D::EcdhKeyDeriveParams),
    ],
    "encrypt" => &[
      ("RSA-OAEP", D::RsaOaepParams),
      ("AES-CBC", D::AesCbcParams),
      ("AES-GCM", D::AesGcmParams),
      ("AES-OCB", D::AesGcmParams),
      ("AES-CTR", D::AesCtrParams),
      ("ChaCha20-Poly1305", D::ChaCha20Poly1305Params),
    ],
    "decrypt" => &[
      ("RSA-OAEP", D::RsaOaepParams),
      ("AES-CBC", D::AesCbcParams),
      ("AES-GCM", D::AesGcmParams),
      ("AES-OCB", D::AesGcmParams),
      ("AES-CTR", D::AesCtrParams),
      ("ChaCha20-Poly1305", D::ChaCha20Poly1305Params),
    ],
    "get key length" => &[
      ("AES-CBC", D::AesDerivedKeyParams),
      ("AES-CTR", D::AesDerivedKeyParams),
      ("AES-GCM", D::AesDerivedKeyParams),
      ("AES-KW", D::AesDerivedKeyParams),
      ("HMAC", D::HmacImportParams),
      ("HKDF", D::None),
      ("PBKDF2", D::None),
      ("ChaCha20-Poly1305", D::None),
    ],
    "wrapKey" => &[("AES-KW", D::None)],
    "unwrapKey" => &[("AES-KW", D::None)],
    _ => return None,
  })
}

// ===== Output-object construction helpers =====

fn set<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  key: &str,
  value: v8::Local<'a, v8::Value>,
) {
  let key = v8::String::new(scope, key).unwrap();
  obj.set(scope, key.into(), value);
}

fn u32_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  v: u32,
) -> v8::Local<'a, v8::Value> {
  v8::Number::new(scope, v as f64).into()
}

fn buffer_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  bytes: &[u8],
) -> v8::Local<'a, v8::Value> {
  let len = bytes.len();
  let ab = v8::ArrayBuffer::new(scope, len);
  if len > 0
    && let Some(data) = ab.data()
  {
    // SAFETY: ArrayBuffer of `len` bytes; copy our data into its store.
    unsafe {
      std::ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        data.as_ptr() as *mut u8,
        len,
      );
    }
  }
  v8::Uint8Array::new(scope, ab, 0, len).unwrap().into()
}

fn hash_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  name: &str,
) -> v8::Local<'a, v8::Value> {
  let obj = v8::Object::new(scope);
  let n = v8::String::new(scope, name).unwrap();
  set(scope, obj, "name", n.into());
  obj.into()
}

/// Build the normalized-algorithm output object, setting `name` plus the
/// dictionary's parsed members (with the BufferSource / HashAlgorithmIdentifier
/// member coercion already applied during WebIDL parsing).
fn build_output<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  name: &str,
  dict: DictType,
  value: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Object>, NormalizeAlgorithmError> {
  let obj = v8::Object::new(scope);
  let name_v = v8::String::new(scope, name).unwrap();
  set(scope, obj, "name", name_v.into());

  macro_rules! parse {
    ($ty:ty) => {
      <$ty>::convert(
        scope,
        value,
        PREFIX.into(),
        context_fn(),
        &Default::default(),
      )?
    };
  }

  match dict {
    D::None => {}
    D::RsaHashedKeyGenParams => {
      let p = parse!(RsaHashedKeyGenParams);
      let v = u32_val(scope, p.modulus_length);
      set(scope, obj, "modulusLength", v);
      let v = buffer_val(scope, &p.public_exponent.0);
      set(scope, obj, "publicExponent", v);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
    }
    D::RsaHashedImportParams => {
      let p = parse!(RsaHashedImportParams);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
    }
    D::RsaOaepParams => {
      let p = parse!(RsaOaepParams);
      if let Some(label) = p.label {
        let v = buffer_val(scope, &label.0);
        set(scope, obj, "label", v);
      }
    }
    D::RsaPssParams => {
      let p = parse!(RsaPssParams);
      let v = u32_val(scope, p.salt_length);
      set(scope, obj, "saltLength", v);
    }
    D::EcKeyGenParams => {
      let p = parse!(EcKeyGenParams);
      let v = v8::String::new(scope, &p.named_curve).unwrap();
      set(scope, obj, "namedCurve", v.into());
    }
    D::EcKeyImportParams => {
      let p = parse!(EcKeyImportParams);
      let v = v8::String::new(scope, &p.named_curve).unwrap();
      set(scope, obj, "namedCurve", v.into());
    }
    D::EcdsaParams => {
      let p = parse!(EcdsaParams);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
    }
    D::EcdhKeyDeriveParams => {
      // `public` (a CryptoKey) is required; pass the raw value through to keep
      // JS object identity.
      let key = v8::String::new(scope, "public").unwrap();
      let public = value
        .try_cast::<v8::Object>()
        .ok()
        .and_then(|o| o.get(scope, key.into()))
        .filter(|v| !v.is_undefined());
      match public {
        Some(v) => set(scope, obj, "public", v),
        None => {
          return Err(NormalizeAlgorithmError::WebIdl(WebIdlError::new(
            PREFIX.into(),
            context_fn(),
            WebIdlErrorKind::DictionaryCannotConvertKey {
              converter: "EcdhKeyDeriveParams",
              key: "public",
            },
          )));
        }
      }
    }
    D::AesKeyGenParams => {
      let p = parse!(AesKeyGenParams);
      let v = u32_val(scope, p.length as u32);
      set(scope, obj, "length", v);
    }
    D::AesDerivedKeyParams => {
      let p = parse!(AesDerivedKeyParams);
      let v = u32_val(scope, p.length);
      set(scope, obj, "length", v);
    }
    D::AesCtrParams => {
      let p = parse!(AesCtrParams);
      let v = buffer_val(scope, &p.counter.0);
      set(scope, obj, "counter", v);
      let v = u32_val(scope, p.length as u32);
      set(scope, obj, "length", v);
    }
    D::AesCbcParams => {
      let p = parse!(AesCbcParams);
      let v = buffer_val(scope, &p.iv.0);
      set(scope, obj, "iv", v);
    }
    D::AesGcmParams => {
      let p = parse!(AesGcmParams);
      let v = buffer_val(scope, &p.iv.0);
      set(scope, obj, "iv", v);
      if let Some(tag_length) = p.tag_length {
        let v = u32_val(scope, tag_length);
        set(scope, obj, "tagLength", v);
      }
      if let Some(ad) = p.additional_data {
        let v = buffer_val(scope, &ad.0);
        set(scope, obj, "additionalData", v);
      }
    }
    D::HmacKeyGenParams => {
      let p = parse!(HmacKeyGenParams);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
      if let Some(length) = p.length {
        let v = u32_val(scope, length);
        set(scope, obj, "length", v);
      }
    }
    D::HmacImportParams => {
      let p = parse!(HmacImportParams);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
      if let Some(length) = p.length {
        let v = u32_val(scope, length);
        set(scope, obj, "length", v);
      }
    }
    D::HkdfParams => {
      let p = parse!(HkdfParams);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
      let v = buffer_val(scope, &p.salt.0);
      set(scope, obj, "salt", v);
      let v = buffer_val(scope, &p.info.0);
      set(scope, obj, "info", v);
    }
    D::Pbkdf2Params => {
      let p = parse!(Pbkdf2Params);
      let v = hash_val(scope, &p.hash.0);
      set(scope, obj, "hash", v);
      let v = u32_val(scope, p.iterations);
      set(scope, obj, "iterations", v);
      let v = buffer_val(scope, &p.salt.0);
      set(scope, obj, "salt", v);
    }
    D::ShakeParams => {
      let p = parse!(ShakeParams);
      let v = u32_val(scope, p.output_length);
      set(scope, obj, "length", v);
    }
    D::CShakeParams => {
      let p = parse!(CShakeParams);
      let v = u32_val(scope, p.output_length);
      set(scope, obj, "length", v);
      if let Some(fc) = p.function_name {
        let v = buffer_val(scope, &fc.0);
        set(scope, obj, "functionName", v);
      }
      if let Some(c) = p.customization {
        let v = buffer_val(scope, &c.0);
        set(scope, obj, "customization", v);
      }
    }
    D::TurboShakeParams => {
      let p = parse!(TurboShakeParams);
      let v = u32_val(scope, p.output_length);
      set(scope, obj, "length", v);
      if let Some(ds) = p.domain_separation {
        let v = u32_val(scope, ds as u32);
        set(scope, obj, "domainSeparation", v);
      }
    }
    D::MlDsaParams => {
      let p = parse!(MlDsaParams);
      if let Some(c) = p.context {
        let v = buffer_val(scope, &c.0);
        set(scope, obj, "context", v);
      }
    }
    D::ChaCha20Poly1305Params => {
      let p = parse!(ChaCha20Poly1305Params);
      let v = buffer_val(scope, &p.iv.0);
      set(scope, obj, "nonce", v);
      if let Some(ad) = p.additional_data {
        let v = buffer_val(scope, &ad.0);
        set(scope, obj, "additionalData", v);
      }
    }
  }

  Ok(obj)
}

/// Port of `normalizeAlgorithm(algorithm, op)`.
pub fn normalize_algorithm<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  op: &str,
  value: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Object>, NormalizeAlgorithmError> {
  // 1. registry for this op.
  let registered = registry(op).unwrap_or(&[]);

  // 2.-3. Parse as an Algorithm (extracting `name`).
  let initial = if value.is_string() {
    let name = String::convert(
      scope,
      value,
      PREFIX.into(),
      context_fn(),
      &Default::default(),
    )?;
    Algorithm { name }
  } else {
    Algorithm::convert(
      scope,
      value,
      PREFIX.into(),
      context_fn(),
      &Default::default(),
    )?
  };

  // 4.-5. Case-insensitive canonicalization.
  let mut canonical: Option<(&'static str, DictType)> = None;
  for (key, ty) in registered {
    if key.eq_ignore_ascii_case(&initial.name) {
      canonical = Some((*key, *ty));
    }
  }
  let (alg_name, dict) =
    canonical.ok_or(NormalizeAlgorithmError::UnrecognizedAlgorithm)?;

  build_output(scope, alg_name, dict, value)
}

#[op2]
pub fn op_crypto_normalize_algorithm<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[string] op: String,
  algorithm: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Object>, NormalizeAlgorithmError> {
  normalize_algorithm(scope, &op, algorithm)
}

/// Case-insensitive lookup of an algorithm `name` in the `supportedAlgorithms`
/// registry for `op`. Backs the `SubtleCrypto.supports()` feature-detection
/// primitive (WICG modern algorithms), which must answer without validating
/// operation-specific parameter dictionaries.
#[op2(fast)]
pub fn op_crypto_is_algorithm_registered(
  #[string] op: &str,
  #[string] name: &str,
) -> bool {
  registry(op).is_some_and(|entries| {
    entries
      .iter()
      .any(|(key, _)| key.eq_ignore_ascii_case(name))
  })
}
