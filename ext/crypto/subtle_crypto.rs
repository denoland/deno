// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto `SubtleCrypto` as a cppgc-wrapped Rust object.
//!
//! Registered on the extension via `objects = [SubtleCrypto]` so the class
//! identity lives in Rust. The singleton instance reachable as
//! `globalThis.crypto.subtle` is minted by [`op_create_subtle_crypto`].

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::CryptoError;
use crate::algorithm::check_support_for_algorithm;
use crate::algorithm::compute_key_length;
use crate::algorithm::registered_algorithm;
use crate::digest::BufferSource;
use crate::digest::DigestAlgorithm;
use crate::digest::run as run_digest;
use crate::shared::SharedError;
use crate::subtle_decrypt::SubtleDecryptParams;
use crate::subtle_decrypt::run as run_decrypt;
use crate::subtle_derive_bits::SubtleDeriveBitsParams;
use crate::subtle_derive_bits::run as run_derive_bits;
use crate::subtle_encapsulate::EncapsulateBitsOutput;
use crate::subtle_encapsulate::SubtleEncapsulateParams;
use crate::subtle_encapsulate::run_decapsulate_bits;
use crate::subtle_encapsulate::run_encapsulate_bits;
use crate::subtle_encrypt::SubtleEncryptParams;
use crate::subtle_encrypt::run as run_encrypt;
use crate::subtle_encrypt::v8_str;
use crate::subtle_key::SubtleKey;
use crate::subtle_sign::SubtleSignParams;
use crate::subtle_sign::run as run_sign;
use crate::subtle_verify::SubtleVerifyParams;
use crate::subtle_verify::run as run_verify;

pub struct SubtleCrypto;

impl WebIdlInterfaceConverter for SubtleCrypto {
  const NAME: &'static str = "SubtleCrypto";
}

// SAFETY: zero-sized payload.
unsafe impl GarbageCollected for SubtleCrypto {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"SubtleCrypto"
  }
}

#[op2]
impl SubtleCrypto {
  /// `new SubtleCrypto()` is illegal per the WebCrypto spec.
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<SubtleCrypto, SharedError> {
    Err(SharedError::IllegalConstructor)
  }

  /// `SubtleCrypto.supports(operation, algorithm, lengthOrHash?)` from the
  /// WICG modern-algos spec. Both overloads are implemented here in Rust:
  /// the third argument is sniffed as either a numeric `length` (ignored,
  /// per spec) or as an additional `AlgorithmIdentifier`. The
  /// `deriveKey` / `unwrapKey` / `wrapKey` / `encapsulateKey` /
  /// `decapsulateKey` paperwork that the spec layers on the second
  /// overload runs in this method body without re-entering JS.
  #[fast]
  #[required(2)]
  #[static_method]
  fn supports<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    #[string] operation: String,
    algorithm: v8::Local<'s, v8::Value>,
    length_or_hash: Option<v8::Local<'s, v8::Value>>,
  ) -> bool {
    supports_inner(scope, &operation, algorithm, length_or_hash)
      .unwrap_or(false)
  }

  /// `SubtleCrypto.digest(algorithm, data)` — compute a one-shot
  /// cryptographic hash. The `WebIdlConverter` for `DigestAlgorithm`
  /// performs the `AlgorithmIdentifier` coercion + canonical name lookup
  /// that the JS body used to do via `normalizeAlgorithm`, and
  /// `BufferSource` copies the input bytes upfront so we satisfy the
  /// spec "get a copy of the bytes" before any async work.
  #[required(2)]
  #[arraybuffer]
  async fn digest(
    &self,
    #[webidl] algorithm: DigestAlgorithm,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_digest(algorithm, data.0)).await?
  }

  /// `SubtleCrypto.encrypt(algorithm, key, data)` — apply the requested
  /// encryption algorithm to `data` using `key`. The per-algorithm
  /// dictionary parsing (`label`, `iv`, `counter`, `length`, `tagLength`,
  /// `additionalData`) is done by the `SubtleEncryptParams`
  /// `WebIdlConverter`; the `SubtleKey` `WebIdlConverter` snapshots the
  /// `CryptoKey` slots (`algorithm.name`, `algorithm.length`,
  /// `algorithm.hash`, `usages`, `type`, and the underlying
  /// [`crate::key_store::CryptoKeyHandle`] data) so the dispatch can run
  /// off the v8 stack inside `spawn_blocking`.
  #[required(3)]
  #[arraybuffer]
  async fn encrypt(
    &self,
    #[webidl] algorithm: SubtleEncryptParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_encrypt(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.decrypt(algorithm, key, data)` — inverse of
  /// [`encrypt`](Self::encrypt). Same converter/dispatch shape; the JS
  /// shim used to enforce that an algorithm-name/key mismatch raises
  /// `OperationError` (whereas `encrypt` raised `InvalidAccessError`),
  /// which [`crate::subtle_decrypt::run`] preserves.
  #[required(3)]
  #[arraybuffer]
  async fn decrypt(
    &self,
    #[webidl] algorithm: SubtleDecryptParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_decrypt(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.sign(algorithm, key, data)` — produce a signature
  /// of `data` under `key`. The `SubtleSignParams` `WebIdlConverter`
  /// handles the per-algorithm parameter dictionary (`saltLength` for
  /// RSA-PSS, `hash` for ECDSA, optional `context` for ML-DSA), and
  /// `SubtleKey` snapshots the `CryptoKey` slots so the dispatch runs
  /// off the v8 stack inside `spawn_blocking`. Ed25519 stays a sync
  /// fastcall in the spawned closure.
  #[required(3)]
  #[arraybuffer]
  async fn sign(
    &self,
    #[webidl] algorithm: SubtleSignParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_sign(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.verify(algorithm, key, signature, data)` — mirror of
  /// [`sign`](Self::sign). The signature `BufferSource` is copied
  /// upfront so the spawned closure sees stable bytes; per-algorithm
  /// dispatch lives in [`crate::subtle_verify::run`].
  #[required(4)]
  async fn verify(
    &self,
    #[webidl] algorithm: SubtleVerifyParams,
    #[webidl] key: SubtleKey,
    #[webidl] signature: BufferSource,
    #[webidl] data: BufferSource,
  ) -> Result<bool, CryptoError> {
    spawn_blocking(move || run_verify(algorithm, key, signature.0, data.0))
      .await?
  }

  /// `SubtleCrypto.deriveBits(algorithm, baseKey, length?)` —
  /// algorithm-specific key derivation (PBKDF2 / HKDF / ECDH /
  /// X25519 / X448). The `SubtleDeriveBitsParams` `WebIdlConverter`
  /// pulls the algorithm dictionary (`hash` / `salt` / `iterations`
  /// for PBKDF2; `hash` / `salt` / `info` for HKDF; peer `public`
  /// `CryptoKey` for the ECDH / X-curve variants) and `SubtleKey`
  /// snapshots `baseKey`; dispatch lives in
  /// [`crate::subtle_derive_bits::run`].
  #[arraybuffer]
  #[rename("deriveBits")]
  async fn derive_bits(
    &self,
    #[webidl] algorithm: SubtleDeriveBitsParams,
    #[webidl] base_key: SubtleKey,
    length: Option<f64>,
  ) -> Result<Vec<u8>, CryptoError> {
    if !base_key.has_usage("deriveBits") {
      return Err(CryptoError::Other(deno_error::JsErrorBox::new(
        "DOMExceptionInvalidAccessError",
        "'baseKey' usages does not contain 'deriveBits'",
      )));
    }
    spawn_blocking(move || run_derive_bits(algorithm, base_key, length)).await?
  }

  /// Internal entry point used by `SubtleCrypto.prototype.deriveKey`
  /// (still in JS) to derive raw bits without re-checking the
  /// `deriveBits` usage on the base key. The JS shim has already
  /// enforced the `deriveKey` usage and algorithm match before calling
  /// this method.
  #[arraybuffer]
  #[rename("__deriveBitsInternal")]
  async fn derive_bits_internal(
    &self,
    #[webidl] algorithm: SubtleDeriveBitsParams,
    #[webidl] base_key: SubtleKey,
    length: Option<f64>,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_derive_bits(algorithm, base_key, length)).await?
  }

  /// `SubtleCrypto.encapsulateBits(algorithm, encapsulationKey)` from the
  /// WICG modern-algos spec -- ML-KEM-only. Returns a dict with the KEM
  /// `ciphertext` and the raw `sharedKey` bytes as `ArrayBuffer`s.
  ///
  /// https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-encapsulateBits
  #[rename("encapsulateBits")]
  async fn encapsulate_bits(
    &self,
    #[webidl] algorithm: SubtleEncapsulateParams,
    #[webidl] encapsulation_key: SubtleKey,
  ) -> Result<EncapsulateBitsOutput, CryptoError> {
    spawn_blocking(move || run_encapsulate_bits(algorithm, encapsulation_key))
      .await?
  }

  /// `SubtleCrypto.decapsulateBits(algorithm, decapsulationKey, ciphertext)`
  /// from the WICG modern-algos spec -- ML-KEM-only. Returns the raw
  /// shared-secret bytes recovered from the KEM ciphertext.
  ///
  /// https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-decapsulateBits
  #[arraybuffer]
  #[rename("decapsulateBits")]
  async fn decapsulate_bits(
    &self,
    #[webidl] algorithm: SubtleEncapsulateParams,
    #[webidl] decapsulation_key: SubtleKey,
    #[webidl] ciphertext: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || {
      run_decapsulate_bits(algorithm, decapsulation_key, ciphertext.0)
    })
    .await?
  }
}

/// Mint the singleton `SubtleCrypto` instance reachable as
/// `globalThis.crypto.subtle`.
#[op2]
#[cppgc]
pub fn op_create_subtle_crypto() -> SubtleCrypto {
  SubtleCrypto
}

/// Body of `SubtleCrypto.supports()` — kept out of the macro `impl` block so
/// `?` short-circuits can use plain `Option`. Returns `None` on any
/// recoverable input/extract failure, which the caller turns into the
/// `false` the spec mandates.
fn supports_inner<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  operation: &str,
  algorithm: v8::Local<'s, v8::Value>,
  length_or_hash: Option<v8::Local<'s, v8::Value>>,
) -> Option<bool> {
  // 1. The primary AlgorithmIdentifier — string-or-{name: string} per the
  // WebIDL converter the JS shim used to apply before calling op_supports.
  let (algorithm_name, algorithm_obj) = extract_alg_name(scope, algorithm)?;

  // 2. Decide which overload was invoked. `lengthOrHash` is `number |
  // AlgorithmIdentifier`; a `null` / `undefined` is the 1-arg overload, and
  // any non-numeric, non-null value is the 2-arg AlgorithmIdentifier.
  let mut length: Option<u32> = None;
  let mut additional_algorithm: Option<v8::Local<'s, v8::Value>> = None;
  if let Some(v) = length_or_hash
    && !v.is_undefined()
    && !v.is_null()
  {
    if v.is_number() {
      // Per the JS shim, coerce with `>>> 0`. Use `uint32_value` so a NaN /
      // negative double folds to a value the rest of the dispatch can ignore.
      length = v.uint32_value(scope);
    } else {
      additional_algorithm = Some(v);
    }
  }

  // 3. Second-overload paperwork — extra registry checks on the additional
  // AlgorithmIdentifier and, for `deriveKey`, a `get key length` lookup.
  if let Some(additional) = additional_algorithm {
    let (additional_name, additional_obj) =
      extract_alg_name(scope, additional)?;
    let additional_check_op = match operation {
      "deriveKey" | "unwrapKey" | "encapsulateKey" | "decapsulateKey" => {
        Some("importKey")
      }
      "wrapKey" => Some("exportKey"),
      _ => None,
    };
    if let Some(check_op) = additional_check_op
      && !check_support_for_algorithm(check_op, &additional_name)
    {
      return Some(false);
    }

    if operation == "deriveKey" {
      let registered = registered_algorithm("get key length", &additional_name);
      let Some((canonical_name, _)) = registered else {
        return Some(false);
      };
      let dict_length =
        additional_obj.and_then(|obj| read_u32_member(scope, obj, b"length"));
      let dict_hash_name = additional_obj
        .and_then(|obj| read_hash_name_member(scope, obj))
        .unwrap_or_default();
      let derived_len = match compute_key_length(
        canonical_name,
        dict_length,
        Some(dict_hash_name.as_str()),
      ) {
        Ok(len) => len,
        Err(_) => return Some(false),
      };
      return Some(supports_check(
        scope,
        "deriveBits",
        &algorithm_name,
        algorithm_obj,
        derived_len,
      ));
    }
  }

  Some(supports_check(
    scope,
    operation,
    &algorithm_name,
    algorithm_obj,
    length,
  ))
}

/// `check support for an algorithm` from the WICG modern-algos spec --
/// the registry membership test plus the parameter-shape validation that
/// rejects bogus members like `AES-GCM` `tagLength: 17` or `HKDF`
/// `length: 7`.
fn supports_check<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  operation: &str,
  algorithm_name: &str,
  algorithm_obj: Option<v8::Local<'s, v8::Object>>,
  length: Option<u32>,
) -> bool {
  if !check_support_for_algorithm(operation, algorithm_name) {
    return false;
  }
  let registered_op = match operation {
    "encapsulateKey" | "encapsulateBits" => "encapsulate",
    "decapsulateKey" | "decapsulateBits" => "decapsulate",
    "deriveKey" => "deriveBits",
    "exportKey" | "getPublicKey" => "importKey",
    "wrapKey" => {
      if registered_algorithm("wrapKey", algorithm_name).is_some() {
        "wrapKey"
      } else {
        "encrypt"
      }
    }
    "unwrapKey" => {
      if registered_algorithm("unwrapKey", algorithm_name).is_some() {
        "unwrapKey"
      } else {
        "decrypt"
      }
    }
    other => other,
  };
  supports_params_valid(
    scope,
    registered_op,
    algorithm_name,
    algorithm_obj,
    length,
  )
}

/// Per-operation parameter validation, mirroring the JS-side
/// `supportsParamsValid` helper. Missing parameters are tolerated -- this
/// is a feature-detection probe -- but any explicitly supplied member that
/// the operation steps would reject (e.g. a bogus `iv` / `tagLength` /
/// `length` / `namedCurve`, or an unknown `hash`) makes the probe return
/// `false`.
fn supports_params_valid<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  registered_op: &str,
  algorithm_name: &str,
  algorithm_obj: Option<v8::Local<'s, v8::Object>>,
  length: Option<u32>,
) -> bool {
  let upper = algorithm_name.to_ascii_uppercase();

  // HKDF / PBKDF2 length constraint applies regardless of whether the
  // algorithm argument was a string or a dictionary.
  if registered_op == "deriveBits" && (upper == "HKDF" || upper == "PBKDF2") {
    let Some(l) = length else { return false };
    if l == 0 || !l.is_multiple_of(8) {
      return false;
    }
  }

  // A bare string identifier carries no further parameters to validate.
  let Some(obj) = algorithm_obj else {
    return true;
  };

  // Any supplied `hash` must be a recognized digest algorithm.
  if let Some(hash_name) = read_optional_hash_name(scope, obj)
    && registered_algorithm("digest", &hash_name).is_none()
  {
    return false;
  }

  match registered_op {
    "encrypt" | "decrypt" => match upper.as_str() {
      "AES-CBC" => {
        if let Some(n) = read_buffer_source_byte_length(scope, obj, b"iv")
          && n != 16
        {
          return false;
        }
        true
      }
      "AES-CTR" => {
        if let Some(n) = read_buffer_source_byte_length(scope, obj, b"counter")
          && n != 16
        {
          return false;
        }
        if let Some(l) = read_u32_member(scope, obj, b"length")
          && (l == 0 || l > 128)
        {
          return false;
        }
        true
      }
      "AES-GCM" | "AES-OCB" => {
        if let Some(n) = read_buffer_source_byte_length(scope, obj, b"iv")
          && n != 12
          && n != 16
        {
          return false;
        }
        if let Some(l) = read_u32_member(scope, obj, b"tagLength")
          && !matches!(l, 32 | 64 | 96 | 104 | 112 | 120 | 128)
        {
          return false;
        }
        true
      }
      "CHACHA20-POLY1305" => {
        if let Some(n) = read_buffer_source_byte_length(scope, obj, b"iv")
          && n != 12
        {
          return false;
        }
        if let Some(l) = read_u32_member(scope, obj, b"tagLength")
          && l != 128
        {
          return false;
        }
        true
      }
      _ => true,
    },
    "generateKey" | "get key length" => match upper.as_str() {
      "AES-CBC" | "AES-CTR" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
        if let Some(l) = read_u32_member(scope, obj, b"length")
          && !matches!(l, 128 | 192 | 256)
        {
          return false;
        }
        true
      }
      "HMAC" => {
        if let Some(0) = read_u32_member(scope, obj, b"length") {
          return false;
        }
        true
      }
      "ECDSA" | "ECDH" => {
        if let Some(curve) = read_string_member(scope, obj, b"namedCurve")
          && !matches!(curve.as_str(), "P-256" | "P-384" | "P-521")
        {
          return false;
        }
        true
      }
      _ => true,
    },
    _ => true,
  }
}

fn read_buffer_source_byte_length<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<usize> {
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
    return Some(view.byte_length());
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(val) {
    return Some(ab.byte_length());
  }
  None
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
  Some(val.to_string(scope)?.to_rust_string_lossy(scope))
}

fn read_optional_hash_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<String> {
  let key = v8_str(scope, "hash");
  // Mirror `ObjectHasOwn(algorithm, "hash") && algorithm.hash !== undefined`
  // - we need to differentiate "absent" (skip) from "present but invalid"
  // (signal via Some("__not_a_valid_hash_sentinel__"))? No: the JS code does
  // `normalizeAlgorithm(algorithm.hash, "digest")` which throws for non-
  // string-or-{name} inputs and for unknown names; the caller treats throw
  // as `false`. So we replicate by returning a string name (or `__invalid__`
  // sentinel) when present, `None` when absent.
  if !obj.has_own_property(scope, key.into())? {
    return None;
  }
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() {
    return None;
  }
  if val.is_string() {
    return Some(val.to_rust_string_lossy(scope));
  }
  let inner = v8::Local::<v8::Object>::try_from(val).ok()?;
  let name_key = v8_str(scope, "name");
  let name_val = inner.get(scope, name_key.into())?;
  Some(name_val.to_string(scope)?.to_rust_string_lossy(scope))
}

fn extract_alg_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> Option<(String, Option<v8::Local<'s, v8::Object>>)> {
  if value.is_string() {
    return Some((value.to_rust_string_lossy(scope), None));
  }
  let obj = v8::Local::<v8::Object>::try_from(value).ok()?;
  let name_key = v8_str(scope, "name");
  let name_val = obj.get(scope, name_key.into())?;
  if name_val.is_undefined() {
    return None;
  }
  let s = name_val.to_string(scope)?.to_rust_string_lossy(scope);
  Some((s, Some(obj)))
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

fn read_hash_name_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<String> {
  let key = v8_str(scope, "hash");
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if val.is_string() {
    return Some(val.to_rust_string_lossy(scope));
  }
  let hash_obj = v8::Local::<v8::Object>::try_from(val).ok()?;
  let name_key = v8_str(scope, "name");
  let name_val = hash_obj.get(scope, name_key.into())?;
  Some(name_val.to_string(scope)?.to_rust_string_lossy(scope))
}
