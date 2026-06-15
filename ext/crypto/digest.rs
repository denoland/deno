// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.digest()` — algorithm parsing, BufferSource coercion and
//! the dispatch onto the `aws_lc_rs` / `sha3` backends. All of the logic
//! that used to live in the `digest(algorithm, data)` method body in
//! `ext/crypto/00_crypto.js` is now here.

use std::borrow::Cow;

use aws_lc_rs::digest;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;
use serde::Deserialize;
use tiny_keccak::Hasher;

use crate::CryptoError;
use crate::key::CryptoHash;

/// Parameter dictionary for the SHA-3-family extendable-output digest
/// algorithms (cSHAKE / TurboSHAKE) used by [`DigestAlgorithm::Xof`].
/// Carries the `outputLength`-and-optional-trim parameters validated in
/// [`run_xof`] rather than at converter time so the eventual error is a
/// `DOMExceptionOperationError`, not a `WebIdlError`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "name")]
pub enum SubtleDigestXof {
  #[serde(rename = "cSHAKE128", rename_all = "camelCase")]
  CShake128 {
    output_length: u32,
    #[serde(with = "serde_bytes", default)]
    function_name: Option<Vec<u8>>,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
  #[serde(rename = "cSHAKE256", rename_all = "camelCase")]
  CShake256 {
    output_length: u32,
    #[serde(with = "serde_bytes", default)]
    function_name: Option<Vec<u8>>,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
  #[serde(rename = "TurboSHAKE128", rename_all = "camelCase")]
  TurboShake128 {
    output_length: u32,
    domain_separation: Option<u8>,
  },
  #[serde(rename = "TurboSHAKE256", rename_all = "camelCase")]
  TurboShake256 {
    output_length: u32,
    domain_separation: Option<u8>,
  },
  #[serde(rename = "KT128", rename_all = "camelCase")]
  Kt128 {
    output_length: u32,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
  #[serde(rename = "KangarooTwelve", rename_all = "camelCase")]
  KangarooTwelve {
    output_length: u32,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
}

/// The `WebIdlConverter` for `AlgorithmIdentifier`-restricted-to-digest,
/// canonicalized into the variant the dispatch code needs. Mirrors what
/// `normalizeAlgorithm(algorithm, "digest")` produced in JS.
///
/// Unrecognized algorithm names are kept as [`DigestAlgorithm::Unknown`] so
/// the dispatch in [`run`] can throw the WebCrypto-spec-mandated
/// `NotSupportedError` `DOMException` (not the WebIDL `TypeError` that a
/// converter-level error would produce). The WPT `digest.https.any.html`
/// "AES-GCM/RSA-OAEP/PBKDF2/AES-KW with empty/short/medium/long" subtests
/// hardcode that error name.
pub enum DigestAlgorithm {
  Sha(CryptoHash),
  /// cSHAKE / TurboSHAKE — variable-length output with extra dictionary
  /// parameters. The validation that the JS body performed against the
  /// raw dictionary (multiple-of-8 outputLength, non-zero TurboSHAKE
  /// outputLength, domainSeparation range) is deferred until [`run`] so
  /// the `WebIdlError` path stays clean.
  Xof(SubtleDigestXof),
  /// Algorithm identifier whose `name` resolved (it was a string or had a
  /// `.name` DOMString) but isn't in the digest registry. Holds the input
  /// spelling so the eventual `NotSupportedError` carries it.
  Unknown(String),
}

impl<'a> WebIdlConverter<'a> for DigestAlgorithm {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    // 1. Resolve the AlgorithmIdentifier union: a `DOMString` is treated
    //    as `{ name: <string> }`; otherwise the input must be an object
    //    carrying a `.name` field. The original dictionary (if any) is
    //    kept around so the XOF arms can pluck `outputLength`,
    //    `functionName`, `customization` and `domainSeparation` off it.
    //
    //    A missing `name` member -> `TypeError` (WebIDL required-member
    //    semantics), exercised by the WPT `digest({}, ...)` "empty
    //    algorithm object" subtest.
    let (name_str, maybe_obj) =
      extract_name_and_obj(scope, value, prefix.clone(), context.borrowed())?;

    // 2. Canonical (case-insensitive) name lookup -- the WebCrypto
    //    registry treats algorithm identifiers as case-insensitive but
    //    every other call site relies on the canonical spelling. If the
    //    name isn't in the digest registry, defer the `NotSupportedError`
    //    to [`run`] so the right error class is thrown.
    let Some(canonical) = canonical_digest_name(&name_str) else {
      return Ok(Self::Unknown(name_str));
    };

    // 3. Build the dispatch variant. SHA / SHA3 do not consult the dict;
    //    XOF variants pull their extra parameters out.
    match canonical {
      "SHA-1" => Ok(Self::Sha(CryptoHash::Sha1)),
      "SHA-256" => Ok(Self::Sha(CryptoHash::Sha256)),
      "SHA-384" => Ok(Self::Sha(CryptoHash::Sha384)),
      "SHA-512" => Ok(Self::Sha(CryptoHash::Sha512)),
      "SHA3-256" => Ok(Self::Sha(CryptoHash::Sha3_256)),
      "SHA3-384" => Ok(Self::Sha(CryptoHash::Sha3_384)),
      "SHA3-512" => Ok(Self::Sha(CryptoHash::Sha3_512)),
      "cSHAKE128" | "cSHAKE256" | "TurboSHAKE128" | "TurboSHAKE256"
      | "KT128" | "KangarooTwelve" => {
        let obj = maybe_obj.ok_or_else(|| {
          WebIdlError::other(
            prefix.clone(),
            context.borrowed(),
            JsErrorBox::type_error(format!(
              "'{canonical}' requires a parameter dictionary"
            )),
          )
        })?;
        let xof = parse_xof_dict(
          scope,
          obj,
          canonical,
          prefix.clone(),
          context.borrowed(),
        )?;
        Ok(Self::Xof(xof))
      }
      _ => unreachable!("canonical_digest_name returned an unknown variant"),
    }
  }
}

fn canonical_digest_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &[
    "SHA-1",
    "SHA-256",
    "SHA-384",
    "SHA-512",
    "SHA3-256",
    "SHA3-384",
    "SHA3-512",
    "cSHAKE128",
    "cSHAKE256",
    "TurboSHAKE128",
    "TurboSHAKE256",
    "KT128",
    "KangarooTwelve",
  ];
  NAMES
    .iter()
    .copied()
    .find(|canon| canon.eq_ignore_ascii_case(name))
}

fn extract_name_and_obj<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  prefix: Cow<'static, str>,
  context: ContextFn<'b>,
) -> Result<(String, Option<v8::Local<'a, v8::Object>>), WebIdlError> {
  if value.is_string() {
    let s = value.to_rust_string_lossy(scope);
    return Ok((s, None));
  }
  if let Ok(obj) = v8::Local::<v8::Object>::try_from(value) {
    let name_key = v8_str(scope, "name");
    let name_val = obj
      .get(scope, name_key.into())
      .unwrap_or_else(|| v8::undefined(scope).into());
    // WebIDL `Algorithm` dictionary requires `name` -- WPT
    // `digest({}, ...)` "empty algorithm object" checks the resulting
    // error name is `TypeError`, not `NotSupportedError`.
    if name_val.is_undefined() {
      return Err(WebIdlError::other(
        prefix,
        context,
        JsErrorBox::type_error("required member 'name' is undefined"),
      ));
    }
    let s = name_val
      .to_string(scope)
      .ok_or_else(|| {
        WebIdlError::other(
          prefix.clone(),
          context.borrowed(),
          JsErrorBox::type_error(
            "algorithm.name is not convertible to DOMString",
          ),
        )
      })?
      .to_rust_string_lossy(scope);
    return Ok((s, Some(obj)));
  }
  Err(WebIdlError::new(
    prefix,
    context,
    WebIdlErrorKind::ConvertToConverterType("AlgorithmIdentifier"),
  ))
}

fn parse_xof_dict<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  canonical: &'static str,
  prefix: Cow<'static, str>,
  context: ContextFn<'b>,
) -> Result<SubtleDigestXof, WebIdlError> {
  let output_length =
    read_required_u32(scope, obj, "outputLength", prefix.clone(), &context)?;
  match canonical {
    "cSHAKE128" => Ok(SubtleDigestXof::CShake128 {
      output_length,
      function_name: read_optional_buffer(
        scope,
        obj,
        "functionName",
        prefix.clone(),
        &context,
      )?,
      customization: read_optional_buffer(
        scope,
        obj,
        "customization",
        prefix.clone(),
        &context,
      )?,
    }),
    "cSHAKE256" => Ok(SubtleDigestXof::CShake256 {
      output_length,
      function_name: read_optional_buffer(
        scope,
        obj,
        "functionName",
        prefix.clone(),
        &context,
      )?,
      customization: read_optional_buffer(
        scope,
        obj,
        "customization",
        prefix.clone(),
        &context,
      )?,
    }),
    "TurboSHAKE128" => Ok(SubtleDigestXof::TurboShake128 {
      output_length,
      domain_separation: read_optional_u8(
        scope,
        obj,
        "domainSeparation",
        prefix.clone(),
        &context,
      )?,
    }),
    "TurboSHAKE256" => Ok(SubtleDigestXof::TurboShake256 {
      output_length,
      domain_separation: read_optional_u8(
        scope,
        obj,
        "domainSeparation",
        prefix.clone(),
        &context,
      )?,
    }),
    "KT128" => Ok(SubtleDigestXof::Kt128 {
      output_length,
      customization: read_optional_buffer(
        scope,
        obj,
        "customization",
        prefix.clone(),
        &context,
      )?,
    }),
    "KangarooTwelve" => Ok(SubtleDigestXof::KangarooTwelve {
      output_length,
      customization: read_optional_buffer(
        scope,
        obj,
        "customization",
        prefix.clone(),
        &context,
      )?,
    }),
    _ => unreachable!(),
  }
}

fn read_required_u32<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  key: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<u32, WebIdlError> {
  let key_v8 = v8_str(scope, key);
  let val = obj
    .get(scope, key_v8.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("required dictionary member '{key}'")),
    ));
  }
  val.uint32_value(scope).ok_or_else(|| {
    WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("'{key}' must be convertible to u32")),
    )
  })
}

fn read_optional_u8<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  key: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Option<u8>, WebIdlError> {
  let key_v8 = v8_str(scope, key);
  let val = obj
    .get(scope, key_v8.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() || val.is_null() {
    return Ok(None);
  }
  // Read the full u32 and reject values outside `[0, 0xFF]` so a stray
  // `0x101` does not wrap to `0x01` and slip past the caller's
  // [1, 0x7F] domain-separation range check (TurboSHAKE edge).
  let u = val.uint32_value(scope).unwrap_or(0);
  if u > u8::MAX as u32 {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("'{key}' must be in the range [0, 0xFF]")),
    ));
  }
  Ok(Some(u as u8))
}

fn read_optional_buffer<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  key: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Option<Vec<u8>>, WebIdlError> {
  let key_v8 = v8_str(scope, key);
  let val = obj
    .get(scope, key_v8.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() || val.is_null() {
    return Ok(None);
  }
  // SAB and SAB-backed views are rejected to match the JS
  // `webidl.converters.BufferSource` contract (cSHAKE
  // `functionName`/`customization`), and non-BufferSource values raise
  // `TypeError` instead of silently materializing as an empty `Vec`.
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(val) {
    if let Some(ab) = view.buffer(scope) {
      let ab_val: v8::Local<v8::Value> = ab.into();
      if ab_val.is_shared_array_buffer() {
        return Err(WebIdlError::other(
          prefix,
          context.borrowed(),
          JsErrorBox::type_error(format!(
            "'{key}' is a view on a SharedArrayBuffer, which is not allowed"
          )),
        ));
      }
    }
    return Ok(Some(value_to_byte_vec(scope, val)));
  }
  if val.is_shared_array_buffer() {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!(
        "'{key}' is a SharedArrayBuffer, which is not allowed"
      )),
    ));
  }
  if v8::Local::<v8::ArrayBuffer>::try_from(val).is_ok() {
    return Ok(Some(value_to_byte_vec(scope, val)));
  }
  Err(WebIdlError::other(
    prefix,
    context.borrowed(),
    JsErrorBox::type_error(format!("'{key}' is not a BufferSource")),
  ))
}

fn value_to_byte_vec<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
) -> Vec<u8> {
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
    let byte_offset = view.byte_offset();
    let byte_length = view.byte_length();
    if byte_length == 0 {
      return Vec::new();
    }
    let ab = view.buffer(scope).unwrap();
    // SAFETY: V8 guarantees byte_offset + byte_length is within the
    // backing store, and a non-detached buffer has a non-null data ptr.
    unsafe {
      let base = ab.data().unwrap().as_ptr() as *const u8;
      std::slice::from_raw_parts(base.add(byte_offset), byte_length).to_vec()
    }
  } else if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
    let byte_length = ab.byte_length();
    if byte_length == 0 {
      return Vec::new();
    }
    // SAFETY: as above.
    unsafe {
      let base = ab.data().unwrap().as_ptr() as *const u8;
      std::slice::from_raw_parts(base, byte_length).to_vec()
    }
  } else {
    Vec::new()
  }
}

/// `WebIdlConverter` matching the WebCrypto `BufferSource` union
/// (`ArrayBufferView` or `ArrayBuffer`). Always materializes the bytes
/// into an owned `Vec<u8>` so the data is safe to hold across `.await`.
///
/// Rejects `SharedArrayBuffer` and `ArrayBufferView`s whose backing buffer is
/// a `SharedArrayBuffer`, mirroring the WebIDL `BufferSource` converter --
/// the WebCrypto spec uses `BufferSource` without `[AllowShared]`, so the
/// SAB-backed `subtle.digest('SHA-256', new Uint8Array(new SharedArrayBuffer))`
/// call required by the `crypto-subtle-cross-realm` node compat test must
/// reject with a `TypeError`.
pub struct BufferSource(pub Vec<u8>);

impl<'a> WebIdlConverter<'a> for BufferSource {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
      if let Some(ab) = view.buffer(scope) {
        let ab_val: v8::Local<v8::Value> = ab.into();
        if ab_val.is_shared_array_buffer() {
          return Err(WebIdlError::other(
            prefix,
            context,
            JsErrorBox::type_error(
              "is a view on a SharedArrayBuffer, which is not allowed",
            ),
          ));
        }
      }
      return Ok(BufferSource(value_to_byte_vec(scope, value)));
    }
    if value.is_shared_array_buffer() {
      return Err(WebIdlError::other(
        prefix,
        context,
        JsErrorBox::type_error("is not an ArrayBuffer or a view on one"),
      ));
    }
    if v8::Local::<v8::ArrayBuffer>::try_from(value).is_ok() {
      return Ok(BufferSource(value_to_byte_vec(scope, value)));
    }
    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType("BufferSource"),
    ))
  }
}

fn v8_str<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  s: &str,
) -> v8::Local<'s, v8::String> {
  v8::String::new_from_one_byte(scope, s.as_bytes(), v8::NewStringType::Normal)
    .unwrap()
}

/// Execute the `SubtleCrypto.digest()` body for the already-normalized
/// algorithm + copied data. Mirrors the JS dispatch in
/// `00_crypto.js: SubtleCrypto.prototype.digest`.
pub fn run(
  algorithm: DigestAlgorithm,
  data: Vec<u8>,
) -> Result<Vec<u8>, CryptoError> {
  match algorithm {
    DigestAlgorithm::Sha(hash) => {
      Ok(digest::digest(hash.into(), &data).as_ref().to_vec())
    }
    DigestAlgorithm::Xof(xof) => run_xof(xof, &data),
    DigestAlgorithm::Unknown(name) => {
      Err(CryptoError::UnsupportedDigestAlgorithm(name))
    }
  }
}

fn run_xof(
  algorithm: SubtleDigestXof,
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  use sha3::digest::ExtendableOutput;
  use sha3::digest::Update;
  use sha3::digest::XofReader;
  use sha3::digest::core_api::CoreWrapper;

  // Validate `outputLength` and `domainSeparation` per the JS body. The
  // JS code threw `OperationError` DOMExceptions for these; the
  // `InvalidXofParameters` variant maps to the same class.
  let output_length = match &algorithm {
    SubtleDigestXof::CShake128 { output_length, .. }
    | SubtleDigestXof::CShake256 { output_length, .. }
    | SubtleDigestXof::TurboShake128 { output_length, .. }
    | SubtleDigestXof::TurboShake256 { output_length, .. }
    | SubtleDigestXof::Kt128 { output_length, .. }
    | SubtleDigestXof::KangarooTwelve { output_length, .. } => *output_length,
  };
  if !output_length.is_multiple_of(8) {
    return Err(CryptoError::InvalidXofParameters);
  }
  let is_turbo = matches!(
    algorithm,
    SubtleDigestXof::TurboShake128 { .. }
      | SubtleDigestXof::TurboShake256 { .. }
  );
  let is_kangaroo = matches!(
    algorithm,
    SubtleDigestXof::Kt128 { .. }
      | SubtleDigestXof::KangarooTwelve { .. }
  );
  if (is_turbo || is_kangaroo) && output_length == 0 {
    return Err(CryptoError::InvalidXofParameters);
  }
  if let SubtleDigestXof::TurboShake128 {
    domain_separation, ..
  }
  | SubtleDigestXof::TurboShake256 {
    domain_separation, ..
  } = &algorithm
    && let Some(d) = domain_separation
    && !(0x01..=0x7F).contains(d)
  {
    return Err(CryptoError::InvalidXofParameters);
  }

  let out_len = (output_length / 8) as usize;
  let mut out = vec![0u8; out_len];

  match algorithm {
    SubtleDigestXof::CShake128 {
      function_name,
      customization,
      ..
    } => {
      let core = sha3::CShake128Core::new_with_function_name(
        function_name.as_deref().unwrap_or(&[]),
        customization.as_deref().unwrap_or(&[]),
      );
      let mut h: sha3::CShake128 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::CShake256 {
      function_name,
      customization,
      ..
    } => {
      let core = sha3::CShake256Core::new_with_function_name(
        function_name.as_deref().unwrap_or(&[]),
        customization.as_deref().unwrap_or(&[]),
      );
      let mut h: sha3::CShake256 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::TurboShake128 {
      domain_separation, ..
    } => {
      let d = domain_separation.unwrap_or(0x1F);
      let core = sha3::TurboShake128Core::new(d);
      let mut h: sha3::TurboShake128 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::TurboShake256 {
      domain_separation, ..
    } => {
      let d = domain_separation.unwrap_or(0x1F);
      let core = sha3::TurboShake256Core::new(d);
      let mut h: sha3::TurboShake256 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::Kt128 { customization, .. }
    | SubtleDigestXof::KangarooTwelve { customization, .. } => {
      let mut h =
        tiny_keccak::KangarooTwelve::new(customization.unwrap_or_default());
      h.update(data);
      h.finalize(&mut out);
    }
  }

  Ok(out)
}
