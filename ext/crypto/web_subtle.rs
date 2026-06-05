// Copyright 2018-2026 the Deno authors. MIT license.

//! The WebCrypto `Crypto` and `SubtleCrypto` interfaces, implemented as
//! deno_core cppgc (`GarbageCollected`) objects — modelled on the webgpu cppgc
//! objects (see `ext/webgpu/buffer.rs` / `ext/webgpu/queue.rs`).
//!
//! This replaces the JS `Crypto` and `SubtleCrypto` classes (and the
//! `webidl.createBranded` / `webidl.configureInterface` machinery) that used to
//! live in `ext/crypto/00_crypto.js`.
//!
//! `Crypto` is fully implemented here: `getRandomValues`, `randomUUID` and the
//! `subtle` getter (which lazily constructs and memoizes a `SubtleCrypto`).
//!
//! The following `SubtleCrypto` methods are implemented here as cppgc async
//! `#[op2] impl` methods (no JS orchestration): `digest`, `encrypt`, `decrypt`,
//! `sign`, `verify`, `deriveBits`, `encapsulateBits`, `decapsulateBits`. Each
//! takes the raw algorithm + key via `FromV8` argument types (see
//! `web_keyutil.rs`) that extract owned, normalized parameters in the
//! synchronous arg-conversion prelude (an async op body has no `scope`), then
//! reuses the per-algorithm crypto in `web_cipher` / `web_signature` /
//! `web_derive` / `web_wrap_kem`.
//!
//! The methods that construct or return `CryptoKey`s are implemented as
//! synchronous cppgc methods here (a cppgc object needs a `scope`, unavailable
//! in an async op body): `importKeySync`, `exportKeySync` and
//! `constructGeneratedKey`. The actual per-algorithm parsing/serialization +
//! `CryptoKey` construction live in `web_keymaker.rs`. The JS side
//! (`00_crypto.js`) keeps only thin `async`/Promise wrappers (`importKey`,
//! `exportKey`, `generateKey`, `deriveKey`, `wrapKey`, `unwrapKey`,
//! `encapsulateKey`, `decapsulateKey`) that do the spec's webidl argument
//! conversion + orchestration and delegate to the sync methods.

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use rand::Rng;
use rand::rngs::StdRng;
use rand::thread_rng;

use crate::CryptoError;
use crate::DigestAlgorithm;
use crate::web_cipher::WebCipherArg;
use crate::web_cipher::WebCipherError;
use crate::web_keymaker as km;
use crate::web_keyutil as ku;

/// `{ ciphertext: ArrayBuffer, sharedKey: ArrayBuffer }` returned by
/// `encapsulateBits`.
struct EncapsulateBitsResult {
  ciphertext: Vec<u8>,
  shared_key: Vec<u8>,
}

impl<'a> ToV8<'a> for EncapsulateBitsResult {
  type Error = std::convert::Infallible;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let obj = v8::Object::new(scope);
    let ct = ArrayBufferResult(self.ciphertext).to_v8(scope)?;
    let k = v8::String::new(scope, "ciphertext").unwrap();
    obj.set(scope, k.into(), ct);
    let sk = ArrayBufferResult(self.shared_key).to_v8(scope)?;
    let k = v8::String::new(scope, "sharedKey").unwrap();
    obj.set(scope, k.into(), sk);
    Ok(obj.into())
  }
}

/// A `Vec<u8>` that serializes to a JS `ArrayBuffer` (not a typed-array view).
/// `SubtleCrypto` methods that resolve to `ArrayBuffer` per spec use this.
pub(crate) struct ArrayBufferResult(pub Vec<u8>);

impl<'a> ToV8<'a> for ArrayBufferResult {
  type Error = std::convert::Infallible;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let ab = if self.0.is_empty() {
      v8::ArrayBuffer::new(scope, 0)
    } else {
      let store =
        v8::ArrayBuffer::new_backing_store_from_vec(self.0).make_shared();
      v8::ArrayBuffer::with_backing_store(scope, &store)
    };
    Ok(ab.into())
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SubtleError {
  #[class(type)]
  #[error("Illegal constructor")]
  #[property("code" = "ERR_ILLEGAL_CONSTRUCTOR")]
  IllegalConstructor,
  #[class("DOMExceptionTypeMismatchError")]
  #[error("The provided value is not an integer-type TypedArray")]
  NotIntegerArray,
  #[class(inherit)]
  #[error(transparent)]
  Crypto(
    #[from]
    #[inherit]
    CryptoError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Cipher(
    #[from]
    #[inherit]
    WebCipherError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Normalize(
    #[from]
    #[inherit]
    crate::web_params::NormalizeAlgorithmError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Signature(
    #[from]
    #[inherit]
    crate::web_signature::WebSignatureError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Derive(
    #[from]
    #[inherit]
    crate::web_derive::WebDeriveError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  WrapKem(
    #[from]
    #[inherit]
    crate::web_wrap_kem::WebWrapKemError,
  ),
  // DOMExceptions raised by the per-algorithm validation that used to live in
  // the JS sign/verify/deriveBits methods.
  #[class("DOMExceptionInvalidAccessError")]
  #[error("{0}")]
  InvalidAccess(String),
  #[class("DOMExceptionNotSupportedError")]
  #[error("{0}")]
  NotSupported(String),
  #[class("DOMExceptionSyntaxError")]
  #[error("{0}")]
  Syntax(String),
  #[class(inherit)]
  #[error(transparent)]
  KeyMaker(
    #[from]
    #[inherit]
    crate::web_keymaker::KeyMakerError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Js(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
}

/// The `Crypto` interface (`globalThis.crypto`).
pub struct Crypto {
  subtle: SameObject<SubtleCrypto>,
}

impl Default for Crypto {
  fn default() -> Self {
    Self {
      subtle: SameObject::new(),
    }
  }
}

impl WebIdlInterfaceConverter for Crypto {
  const NAME: &'static str = "Crypto";
}

// SAFETY: only holds a SameObject which traces its child via the GC roots.
unsafe impl GarbageCollected for Crypto {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Crypto"
  }
}

#[op2]
impl Crypto {
  #[constructor]
  #[cppgc]
  fn constructor() -> Result<Crypto, SubtleError> {
    Err(SubtleError::IllegalConstructor)
  }

  #[required(1)]
  fn get_random_values<'a>(
    &self,
    state: &mut OpState,
    scope: &mut v8::PinScope<'a, '_>,
    array: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Value>, SubtleError> {
    // Per spec, the array must be an integer-type TypedArray. `Float16Array`,
    // `Float32Array`, `Float64Array` and `DataView` are rejected with a
    // `TypeMismatchError`.
    let is_integer_typed_array = array.is_typed_array()
      && !array.is_float32_array()
      && !array.is_float64_array();
    if !is_integer_typed_array {
      return Err(SubtleError::NotIntegerArray);
    }
    let view = v8::Local::<v8::ArrayBufferView>::try_from(array)
      .map_err(|_| SubtleError::NotIntegerArray)?;

    let byte_length = view.byte_length();
    if byte_length > 65536 {
      return Err(SubtleError::Crypto(
        CryptoError::ArrayBufferViewLengthExceeded(byte_length),
      ));
    }
    if byte_length == 0 {
      return Ok(array);
    }

    let byte_offset = view.byte_offset();
    let ab = view.buffer(scope).unwrap();
    // SAFETY: Pointer is non-null, and V8 guarantees that byte_offset +
    // byte_length is within the backing store.
    let out = unsafe {
      let ptr = ab.data().unwrap().as_ptr().add(byte_offset) as *mut u8;
      std::slice::from_raw_parts_mut(ptr, byte_length)
    };

    if let Some(seeded_rng) = state.try_borrow_mut::<StdRng>() {
      seeded_rng.fill(out);
    } else {
      thread_rng().fill(out);
    }

    Ok(array)
  }

  #[rename("randomUUID")]
  #[string]
  fn random_uuid(&self, state: &mut OpState) -> String {
    let mut bytes = [0u8; 16];
    if let Some(seeded_rng) = state.try_borrow_mut::<StdRng>() {
      seeded_rng.fill(&mut bytes);
    } else {
      thread_rng().fill(&mut bytes);
    }
    crate::fast_uuid_v4(&mut bytes)
  }

  #[getter]
  fn subtle(&self, scope: &mut v8::PinScope<'_, '_>) -> v8::Global<v8::Object> {
    self.subtle.get(scope, |_| SubtleCrypto)
  }
}

/// Construct the singleton `Crypto` object for `globalThis.crypto`. `Crypto`'s
/// public constructor throws `IllegalConstructor`, so this internal op is used
/// to create the global instance (mirroring webgpu's `op_create_gpu`).
#[op2]
#[cppgc]
pub fn op_crypto_create_crypto() -> Crypto {
  Crypto::default()
}

/// The `SubtleCrypto` interface (`globalThis.crypto.subtle`).
#[derive(Default)]
pub struct SubtleCrypto;

impl WebIdlInterfaceConverter for SubtleCrypto {
  const NAME: &'static str = "SubtleCrypto";
}

// SAFETY: holds no GC-managed state.
unsafe impl GarbageCollected for SubtleCrypto {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"SubtleCrypto"
  }
}

#[op2]
impl SubtleCrypto {
  #[constructor]
  #[cppgc]
  fn constructor() -> Result<SubtleCrypto, SubtleError> {
    Err(SubtleError::IllegalConstructor)
  }

  // The algorithm normalization (case-insensitive name canonicalization), XOF
  // parameter validation, and SHA/XOF dispatch are all performed in Rust by
  // `crate::digest_web`.
  #[required(2)]
  async fn digest(
    &self,
    #[serde] algorithm: DigestAlgorithm,
    #[anybuffer(copy)] data: Vec<u8>,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let out = crate::digest_web(algorithm, &data).await?;
    Ok(ArrayBufferResult(out.0.into_vec()))
  }

  #[required(3)]
  async fn encrypt(
    &self,
    #[scoped] algorithm: ku::EncryptAlg,
    #[scoped] key: ku::KeySnapshot,
    #[anybuffer(copy)] data: Vec<u8>,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let arg = cipher_arg(algorithm.0, &key);
    let name = crate::web_cipher::canonical_name(&arg.algorithm)?;
    crate::web_cipher::check_name_and_usage(
      name,
      &arg,
      "encrypt",
      &format!("Encryption algorithm '{name}' does not match key algorithm"),
    )?;
    let alg = crate::web_cipher::build_encrypt_algorithm(name, &arg, &data)?;
    let key_data = key.raw.to_owned_raw_key_data();
    let buf = spawn_blocking(move || {
      crate::encrypt::encrypt_compute(&key_data, alg, &data)
    })
    .await
    .unwrap()
    .map_err(WebCipherError::from)?;
    Ok(ArrayBufferResult(buf))
  }

  #[required(3)]
  async fn decrypt(
    &self,
    #[scoped] algorithm: ku::DecryptAlg,
    #[scoped] key: ku::KeySnapshot,
    #[anybuffer(copy)] data: Vec<u8>,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let arg = cipher_arg(algorithm.0, &key);
    let name = crate::web_cipher::canonical_name(&arg.algorithm)?;
    crate::web_cipher::check_name_and_usage(
      name,
      &arg,
      "decrypt",
      &format!("Decryption algorithm \"{name}\" does not match key algorithm"),
    )?;
    let alg = crate::web_cipher::build_decrypt_algorithm(name, &arg, &data)?;
    let key_data = key.raw.to_owned_raw_key_data();
    let buf = spawn_blocking(move || {
      crate::decrypt::decrypt_compute(&key_data, alg, &data)
    })
    .await
    .unwrap()
    .map_err(WebCipherError::from)?;
    Ok(ArrayBufferResult(buf))
  }

  #[required(3)]
  async fn sign(
    &self,
    #[scoped] algorithm: ku::SignAlg,
    #[scoped] key: ku::SignKey,
    #[anybuffer(copy)] data: Vec<u8>,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let p = algorithm.0;
    validate_sign_verify(&p.name, &key, "sign", "private")?;
    let (hash, named_curve, salt_length, context) =
      sign_verify_params(&key, &p, "sign")?;
    let sig = crate::web_signature::sign_web_compute(
      p.name,
      web_key_type(key.effective_key_type()),
      hash,
      salt_length,
      named_curve,
      key.key_bytes,
      data,
      context,
    )
    .await?;
    Ok(ArrayBufferResult(sig))
  }

  async fn derive_bits(
    &self,
    #[scoped] algorithm: ku::DeriveAlg,
    #[scoped] base_key: ku::DeriveBaseSnapshot,
    #[scoped] length: ku::DeriveLength,
    // When true, this is the internal derive-bits abstract operation invoked by
    // `deriveKey` (which already validated the `deriveKey` usage), so the
    // `deriveBits` usage post-condition is skipped. Defaults to false for direct
    // `deriveBits()` calls.
    internal: bool,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let length = length.0;
    let p = algorithm.0;
    let base = crate::web_derive::DeriveBase {
      key_type: web_derive_key_type(&base_key.key_type),
      data: base_key.data,
      algorithm: base_key.algorithm_name.clone(),
      named_curve: base_key.named_curve.as_deref().and_then(crypto_named_curve),
    };
    let public = match &p.public {
      Some(pk) => crate::web_derive::DerivePublic {
        data_type: Some(web_derive_key_type(&pk.key_type)),
        key_type: Some(web_derive_key_type(&pk.outer_key_type)),
        data: Some(pk.data.clone()),
        algorithm: Some(pk.algorithm_name.clone()),
        named_curve: pk.named_curve.as_deref().and_then(crypto_named_curve),
      },
      None => crate::web_derive::DerivePublic {
        data_type: None,
        key_type: None,
        data: None,
        algorithm: None,
        named_curve: None,
      },
    };
    let hash = p.hash.as_deref().and_then(derive_hash);
    let result = crate::web_derive::derive_bits_compute(
      p.name.clone(),
      base,
      public,
      hash,
      p.iterations,
      length,
      p.salt.clone(),
      p.info.clone(),
    )
    .await?;
    // Post-conditions (JS checks these after derivation).
    if p.name != base_key.algorithm_name {
      return Err(SubtleError::InvalidAccess(
        "Invalid algorithm name".to_string(),
      ));
    }
    if !internal && !base_key.usages.iter().any(|u| u == "deriveBits") {
      return Err(SubtleError::InvalidAccess(
        "'baseKey' usages does not contain 'deriveBits'".to_string(),
      ));
    }
    Ok(ArrayBufferResult(result))
  }

  async fn encapsulate_bits(
    &self,
    #[scoped] algorithm: ku::EncapsulateAlg,
    #[scoped] key: ku::KemKeySnapshot,
  ) -> Result<EncapsulateBitsResult, SubtleError> {
    let arg = crate::web_wrap_kem::KemArg {
      algorithm: algorithm.0,
      key_type: key.key_type,
      key_usages: key.usages,
      key_algorithm_name: key.algorithm_name,
    };
    let (ciphertext, shared_key) = crate::web_wrap_kem::encapsulate_compute(
      arg,
      "encapsulateBits",
      &key.key_data,
    )?;
    Ok(EncapsulateBitsResult {
      ciphertext,
      shared_key,
    })
  }

  async fn decapsulate_bits(
    &self,
    #[scoped] algorithm: ku::DecapsulateAlg,
    #[scoped] key: ku::KemKeySnapshot,
    #[anybuffer(copy)] ciphertext: Vec<u8>,
  ) -> Result<ArrayBufferResult, SubtleError> {
    let arg = crate::web_wrap_kem::KemArg {
      algorithm: algorithm.0,
      key_type: key.key_type,
      key_usages: key.usages,
      key_algorithm_name: key.algorithm_name,
    };
    let shared = crate::web_wrap_kem::decapsulate_compute(
      arg,
      "decapsulateBits",
      &key.key_data,
      &ciphertext,
    )?;
    Ok(ArrayBufferResult(shared))
  }

  #[required(4)]
  async fn verify(
    &self,
    #[scoped] algorithm: ku::VerifyAlg,
    #[scoped] key: ku::SignKey,
    #[anybuffer(copy)] signature: Vec<u8>,
    #[anybuffer(copy)] data: Vec<u8>,
  ) -> Result<bool, SubtleError> {
    let p = algorithm.0;
    validate_sign_verify(&p.name, &key, "verify", "public")?;
    let (hash, named_curve, salt_length, context) =
      sign_verify_params(&key, &p, "verify")?;
    let ok = crate::web_signature::verify_web_compute(
      p.name,
      web_key_type(key.effective_key_type()),
      hash,
      salt_length,
      named_curve,
      key.key_bytes,
      data,
      signature,
      context,
    )
    .await?;
    Ok(ok)
  }

  /// `importKey(format, keyData, algorithm, extractable, keyUsages)`. The
  /// per-algorithm parsing + CryptoKey construction happen here synchronously
  /// (a cppgc object needs `scope`); the JS wrapper only adds the Promise.
  #[rename("importKeySync")]
  #[required(4)]
  fn import_key_sync<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[string] format: String,
    key_data: v8::Local<'a, v8::Value>,
    algorithm: v8::Local<'a, v8::Value>,
    extractable: bool,
    #[serde] key_usages: Vec<String>,
  ) -> Result<v8::Local<'a, v8::Value>, SubtleError> {
    let params = build_import_params(
      scope,
      &format,
      key_data,
      algorithm,
      extractable,
      key_usages,
    )?;
    let desc = km::import_key_compute(&params)?;
    // Step 9: secret/private keys must have at least one usage.
    if (desc.key_type == "private" || desc.key_type == "secret")
      && params.key_usages.is_empty()
    {
      return Err(SubtleError::Syntax("Invalid key usage".to_string()));
    }
    Ok(km::build_one(scope, desc).into())
  }

  /// `exportKey(format, key)`. Returns an `ArrayBuffer` (raw/spki/pkcs8) or a
  /// JWK object. Synchronous; the JS wrapper adds the Promise.
  #[rename("exportKeySync")]
  #[required(2)]
  fn export_key_sync<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[string] format: String,
    #[scoped] key: km::ExportKeySnapshot,
  ) -> Result<v8::Local<'a, v8::Value>, SubtleError> {
    let result = km::export_key_compute(&format, key)?;
    Ok(match result {
      km::ExportResult::Buffer(b) => ArrayBufferResult(b).to_v8(scope).unwrap(),
      km::ExportResult::Jwk(v) => deno_core::serde_v8::to_v8(scope, v)
        .map_err(|e| SubtleError::Js(JsErrorBox::generic(e.to_string())))?,
    })
  }

  /// `generateKey(algorithm, extractable, keyUsages)`. The key generation runs
  /// in the async op (`op_crypto_generate_key_web`) on the JS side; this
  /// synchronous method only builds the CryptoKey(s) from the generated bytes.
  #[rename("constructGeneratedKey")]
  fn construct_generated_key<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[scoped] params: km::GenerateBuildParams,
  ) -> Result<v8::Local<'a, v8::Value>, SubtleError> {
    let result = km::generate_build(params)?;
    Ok(km::build_result(scope, result))
  }
}

/// Build the [`km::ImportKeyParams`] from the raw `importKey` arguments,
/// performing the algorithm normalization and the format/data coercion the JS
/// `importKey` used to do.
fn build_import_params<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  format: &str,
  key_data: v8::Local<'a, v8::Value>,
  algorithm: v8::Local<'a, v8::Value>,
  extractable: bool,
  key_usages: Vec<String>,
) -> Result<km::ImportKeyParams, SubtleError> {
  let normalized =
    crate::web_params::normalize_algorithm(scope, "importKey", algorithm)
      .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
  let name = ku::get_string(scope, normalized, "name").unwrap_or_default();
  let hash = ku::get_hash_name(scope, normalized);
  let named_curve = ku::get_string(scope, normalized, "namedCurve");
  let length = ku::get_usize(scope, normalized, "length");

  // Coerce key data: jwk format expects an object, otherwise a BufferSource.
  let (buffer, jwk) = if format == "jwk" {
    if key_data.is_array_buffer() || key_data.is_array_buffer_view() {
      return Err(SubtleError::Js(JsErrorBox::type_error(
        "Cannot import key: 'keyData' is not a JsonWebKey",
      )));
    }
    let v: Value = deno_core::serde_v8::from_v8(scope, key_data)
      .map_err(|e| JsErrorBox::type_error(e.to_string()))?;
    (None, Some(v))
  } else {
    if !(key_data.is_array_buffer() || key_data.is_array_buffer_view()) {
      return Err(SubtleError::Js(JsErrorBox::type_error(
        "Cannot import key: 'keyData' is a JsonWebKey",
      )));
    }
    let buf: deno_core::JsBuffer =
      deno_core::serde_v8::from_v8(scope, key_data)
        .map_err(|e| JsErrorBox::type_error(e.to_string()))?;
    (Some(buf), None)
  };

  Ok(km::ImportKeyParams {
    format: format.to_string(),
    name,
    hash,
    named_curve,
    length,
    extractable,
    key_usages,
    buffer,
    jwk,
  })
}

fn web_derive_key_type(t: &str) -> crate::web_derive::WebKeyType {
  match t {
    "private" => crate::web_derive::WebKeyType::Private,
    "public" => crate::web_derive::WebKeyType::Public,
    _ => crate::web_derive::WebKeyType::Secret,
  }
}

fn derive_hash(name: &str) -> Option<crate::key::CryptoHash> {
  crypto_hash(name)
}

fn web_key_type(t: &str) -> crate::web_signature::WebKeyType {
  match t {
    "private" => crate::web_signature::WebKeyType::Private,
    "public" => crate::web_signature::WebKeyType::Public,
    _ => crate::web_signature::WebKeyType::Secret,
  }
}

fn crypto_hash(name: &str) -> Option<crate::key::CryptoHash> {
  use crate::key::CryptoHash::*;
  Some(match name {
    "SHA-1" => Sha1,
    "SHA-256" => Sha256,
    "SHA-384" => Sha384,
    "SHA-512" => Sha512,
    "SHA3-256" => Sha3_256,
    "SHA3-384" => Sha3_384,
    "SHA3-512" => Sha3_512,
    _ => return None,
  })
}

fn crypto_named_curve(name: &str) -> Option<crate::key::CryptoNamedCurve> {
  use crate::key::CryptoNamedCurve::*;
  Some(match name {
    "P-256" => P256,
    "P-384" => P384,
    "P-521" => P521,
    _ => return None,
  })
}

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

/// Per-algorithm validation shared by `sign`/`verify`: name match (8), usage
/// (9), and key-type check for the asymmetric algorithms.
fn validate_sign_verify(
  alg_name: &str,
  key: &ku::SignKey,
  usage: &str,
  required_type: &str,
) -> Result<(), SubtleError> {
  if alg_name != key.algorithm_name {
    return Err(SubtleError::InvalidAccess(format!(
      "{} algorithm does not match key algorithm",
      if usage == "sign" {
        "Signing"
      } else {
        "Verifying"
      }
    )));
  }
  if !key.usages.iter().any(|u| u == usage) {
    return Err(SubtleError::InvalidAccess(
      "The requested operation is not valid for the provided key".to_string(),
    ));
  }
  match alg_name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "ECDSA" | "Ed25519" | "ML-DSA-44"
    | "ML-DSA-65" | "ML-DSA-87"
      if key.key_type != required_type =>
    {
      return Err(SubtleError::InvalidAccess(
        "Key type not supported".to_string(),
      ));
    }
    _ => {}
  }
  Ok(())
}

#[allow(clippy::type_complexity)]
fn sign_verify_params(
  key: &ku::SignKey,
  p: &ku::SignParams,
  _usage: &str,
) -> Result<
  (
    Option<crate::key::CryptoHash>,
    Option<crate::key::CryptoNamedCurve>,
    Option<u32>,
    Option<Vec<u8>>,
  ),
  SubtleError,
> {
  match p.name.as_str() {
    "RSASSA-PKCS1-v1_5" => {
      Ok((key.hash.as_deref().and_then(crypto_hash), None, None, None))
    }
    "RSA-PSS" => Ok((
      key.hash.as_deref().and_then(crypto_hash),
      None,
      p.salt_length,
      None,
    )),
    "ECDSA" => {
      let nc = key.named_curve.as_deref().unwrap_or("");
      if !SUPPORTED_NAMED_CURVES.contains(&nc) {
        return Err(SubtleError::NotSupported(
          "Curve not supported".to_string(),
        ));
      }
      Ok((
        p.hash.as_deref().and_then(crypto_hash),
        crypto_named_curve(nc),
        None,
        None,
      ))
    }
    "HMAC" => Ok((key.hash.as_deref().and_then(crypto_hash), None, None, None)),
    "Ed25519" => Ok((None, None, None, None)),
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      Ok((None, None, None, p.context.clone()))
    }
    _ => Ok((None, None, None, None)),
  }
}

/// Combine an owned key snapshot + cipher params into the [`WebCipherArg`] the
/// `web_cipher` helpers consume.
fn cipher_arg(p: ku::CipherParams, key: &ku::KeySnapshot) -> WebCipherArg {
  WebCipherArg {
    key_type: key.key_type.clone(),
    key_usages: key.usages.clone(),
    key_algorithm_name: key.algorithm_name.clone(),
    key_length: key.length,
    key_hash: key.hash.clone(),
    algorithm: p.name,
    iv: p.iv,
    counter: p.counter,
    length: p.length,
    label: p.label,
    additional_data: p.additional_data,
    tag_length: p.tag_length,
    nonce: p.nonce,
  }
}
