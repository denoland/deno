// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.wrapKey()` / `unwrapKey()` bodies in Rust.
//!
//! Composes [`crate::subtle_export_key::run`] with the AES-KW wrap/unwrap
//! transforms (inlined to avoid the `op_crypto_wrap_key` / `op_crypto_unwrap_key`
//! op boundary) or the encrypt/decrypt paths
//! ([`crate::subtle_encrypt::run`] / [`crate::subtle_decrypt::run`]).
//! The unwrap branch then mints the unwrapped `CryptoKey` via
//! [`crate::subtle_import_key::run`].

use std::borrow::Cow;

use aes_kw::KekAes128;
use aes_kw::KekAes192;
use aes_kw::KekAes256;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::subtle_decrypt::SubtleDecryptParams;
use crate::subtle_decrypt::run as run_decrypt;
use crate::subtle_encrypt::SubtleEncryptParams;
use crate::subtle_encrypt::run as run_encrypt;
use crate::subtle_export_key::ExportKeyOutput;
use crate::subtle_export_key::KeyFormat;
use crate::subtle_export_key::run as run_export_key;
use crate::subtle_import_key::ImportAlgorithm;
use crate::subtle_import_key::ImportKeyData;
use crate::subtle_import_key::run as run_import_key;
use crate::subtle_key::SubtleKey;

/// Common shape for either side of the wrap/unwrap pair: the "transform"
/// algorithm name + parameters, captured as one of two concrete forms —
/// an AES-KW pass (which only uses `key.algorithm.name` + the symmetric
/// `wrappingKey`) or a full encrypt/decrypt pass (with the per-algorithm
/// params already coerced).
pub enum WrapParams {
  AesKw,
  Encrypt(SubtleEncryptParams),
}

pub enum UnwrapParams {
  AesKw,
  Decrypt(SubtleDecryptParams),
}

/// WebIDL converter for the `wrapAlgorithm` argument: tries AES-KW first
/// (the only `wrapKey`-registered algorithm), falls back to the full
/// encrypt converter.
pub struct WrapAlgorithm {
  pub name: String,
  pub params: WrapParams,
}

impl<'a> WebIdlConverter<'a> for WrapAlgorithm {
  type Options = ();
  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name, _obj) = crate::subtle_encrypt::extract_name_and_obj(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
    )?;
    if name.eq_ignore_ascii_case("AES-KW") {
      return Ok(Self {
        name: "AES-KW".to_string(),
        params: WrapParams::AesKw,
      });
    }
    let enc = SubtleEncryptParams::convert(scope, value, prefix, context, &())?;
    Ok(Self {
      name: enc.canonical_name().to_string(),
      params: WrapParams::Encrypt(enc),
    })
  }
}

/// WebIDL converter for the `unwrapAlgorithm` argument.
pub struct UnwrapAlgorithm {
  pub name: String,
  pub params: UnwrapParams,
}

impl<'a> WebIdlConverter<'a> for UnwrapAlgorithm {
  type Options = ();
  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name, _obj) = crate::subtle_encrypt::extract_name_and_obj(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
    )?;
    if name.eq_ignore_ascii_case("AES-KW") {
      return Ok(Self {
        name: "AES-KW".to_string(),
        params: UnwrapParams::AesKw,
      });
    }
    let dec = SubtleDecryptParams::convert(scope, value, prefix, context, &())?;
    Ok(Self {
      name: dec.canonical_name().to_string(),
      params: UnwrapParams::Decrypt(dec),
    })
  }
}

/// Body of `SubtleCrypto.wrapKey(format, key, wrappingKey, wrapAlgorithm)`.
/// Returns the wrapped bytes as a `Vec<u8>` that the cppgc impl turns
/// into an `ArrayBuffer` via the op2 `#[arraybuffer]` return shape.
pub fn run_wrap_key(
  format: KeyFormat,
  key: SubtleKey,
  wrapping_algorithm_name: &str,
  wrapping_key: SubtleKey,
  wrap_params: WrapParams,
) -> Result<Vec<u8>, CryptoError> {
  if wrapping_algorithm_name != wrapping_key.algorithm_name {
    return Err(invalid_access(
      "Wrapping algorithm does not match key algorithm".into(),
    ));
  }
  if !wrapping_key.has_usage("wrapKey") {
    return Err(invalid_access(
      "The requested operation is not valid for the provided key".into(),
    ));
  }
  if !key.extractable {
    return Err(invalid_access("Key is not extractable".into()));
  }
  let exported = run_export_key(format, key)?;
  let bytes = match exported {
    ExportKeyOutput::Bytes(b) => b,
    ExportKeyOutput::Jwk(jwk) => jwk_to_utf8(&jwk),
  };
  match wrap_params {
    WrapParams::AesKw => aes_kw_wrap(&wrapping_key, &bytes),
    WrapParams::Encrypt(params) => {
      // The legacy JS constructed a fresh `CryptoKey` with `["encrypt"]`
      // usages to satisfy the encrypt path; in Rust the encrypt path
      // takes a `SubtleKey` so we just override the captured usages list.
      let mut encrypting_key = wrapping_key;
      encrypting_key.usages = vec!["encrypt".to_string()];
      run_encrypt(params, encrypting_key, bytes)
    }
  }
}

/// First half of `SubtleCrypto.unwrapKey`: validate algorithm + usage,
/// then decrypt (or AES-KW unwrap) the wrapped bytes. Pure-CPU, no v8
/// scope — safe to run inside `spawn_blocking` so a large wrapped payload
/// does not block the event loop (matching the pre-port behavior where
/// `unwrapKey`'s JS body did `await this.decrypt(...)` and `decrypt`
/// already offloaded the heavy AES/RSA/ChaCha20 work to a worker thread).
pub fn unwrap_to_plaintext(
  wrapped_bytes: Vec<u8>,
  unwrapping_algorithm_name: &str,
  unwrapping_key: SubtleKey,
  unwrap_params: UnwrapParams,
) -> Result<Vec<u8>, CryptoError> {
  if unwrapping_algorithm_name != unwrapping_key.algorithm_name {
    return Err(invalid_access(
      "Unwrapping algorithm does not match key algorithm".into(),
    ));
  }
  if !unwrapping_key.has_usage("unwrapKey") {
    return Err(invalid_access(
      "The requested operation is not valid for the provided key".into(),
    ));
  }
  match unwrap_params {
    UnwrapParams::AesKw => aes_kw_unwrap(&unwrapping_key, &wrapped_bytes),
    UnwrapParams::Decrypt(params) => {
      let mut decrypting_key = unwrapping_key;
      decrypting_key.usages = vec!["decrypt".to_string()];
      run_decrypt(params, decrypting_key, wrapped_bytes)
    }
  }
}

/// Bundle the unwrap-step plaintext with the import-step params, returned
/// from the async `SubtleCrypto.unwrapKey` body. The `ToV8` impl runs the
/// import step (which needs the v8 scope to build the `CryptoKey` cppgc
/// instance) synchronously in the JS scope once `spawn_blocking` has
/// resolved the decrypt step. Mirrors `subtle_derive_key::DerivedKey`.
pub struct UnwrappedKey {
  pub format: KeyFormat,
  pub plain_bytes: Vec<u8>,
  pub unwrapped_key_algorithm: ImportAlgorithm,
  pub extractable: bool,
  pub usages: Vec<String>,
}

impl<'a> deno_core::ToV8<'a> for UnwrappedKey {
  type Error = deno_error::JsErrorBox;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let UnwrappedKey {
      format,
      plain_bytes,
      unwrapped_key_algorithm,
      extractable,
      usages,
    } = self;
    let import_data = match format {
      KeyFormat::Jwk => {
        let value =
          utf8_to_v8_json(scope, &plain_bytes).map_err(crypto_error_to_js)?;
        ImportKeyData::from_v8(scope, value, format)
          .map_err(crypto_error_to_js)?
      }
      _ => ImportKeyData::Buffer(plain_bytes),
    };
    let key = run_import_key(
      scope,
      format,
      &unwrapped_key_algorithm,
      import_data,
      extractable,
      &usages,
    )
    .map_err(crypto_error_to_js)?;
    // Spec step 16: private/secret + empty usages → SyntaxError.
    let key_type = deno_core::cppgc::try_unwrap_cppgc_object::<
      crate::crypto_key::CryptoKey,
    >(scope, key.into())
    .map(|p| p.key_type());
    if matches!(
      key_type,
      Some(CryptoKeyType::Private) | Some(CryptoKeyType::Secret)
    ) && usages.is_empty()
    {
      return Err(crypto_error_to_js(syntax_error("Invalid key type".into())));
    }
    Ok(key.into())
  }
}

fn crypto_error_to_js(err: CryptoError) -> deno_error::JsErrorBox {
  match err {
    CryptoError::Other(b) => b,
    other => deno_error::JsErrorBox::from_err(other),
  }
}

fn aes_kw_wrap(
  wrapping_key: &SubtleKey,
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  if !data.len().is_multiple_of(8) {
    return Err(CryptoError::DataInvalidSize);
  }
  let key = wrapping_key.raw.as_secret_key()?;
  let wrapped = match key.len() {
    16 => KekAes128::new(key.into()).wrap_vec(data),
    24 => KekAes192::new(key.into()).wrap_vec(data),
    32 => KekAes256::new(key.into()).wrap_vec(data),
    _ => return Err(CryptoError::InvalidKeyLength),
  }
  .map_err(|_| CryptoError::EncryptionError)?;
  Ok(wrapped)
}

fn aes_kw_unwrap(
  unwrapping_key: &SubtleKey,
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  if !data.len().is_multiple_of(8) {
    return Err(CryptoError::DataInvalidSize);
  }
  let key = unwrapping_key.raw.as_secret_key()?;
  let unwrapped = match key.len() {
    16 => KekAes128::new(key.into()).unwrap_vec(data),
    24 => KekAes192::new(key.into()).unwrap_vec(data),
    32 => KekAes256::new(key.into()).unwrap_vec(data),
    _ => return Err(CryptoError::InvalidKeyLength),
  }
  .map_err(|_| CryptoError::DecryptionError)?;
  Ok(unwrapped)
}

/// Stringify a JWK to UTF-8 bytes, matching the legacy JS
/// `new Uint8Array(JSON.stringify(jwk).split('').map(c => c.charCodeAt(0)))`
/// shape (which is just a UTF-8 encoding of the JSON for the spec-mandated
/// JWK members).
fn jwk_to_utf8(jwk: &crate::subtle_export_key::JsonWebKey) -> Vec<u8> {
  let mut out = String::new();
  out.push('{');
  let mut first = true;
  macro_rules! emit_str {
    ($k:expr, $v:expr) => {{
      if !first {
        out.push(',');
      }
      first = false;
      out.push('"');
      out.push_str($k);
      out.push('"');
      out.push(':');
      out.push('"');
      out.push_str(&json_escape($v));
      out.push('"');
    }};
  }
  emit_str!("kty", jwk.kty);
  if let Some(ref v) = jwk.alg {
    emit_str!("alg", v);
  }
  if let Some(ref v) = jwk.crv {
    emit_str!("crv", v);
  }
  if let Some(ref v) = jwk.k {
    emit_str!("k", v);
  }
  if let Some(ref v) = jwk.n {
    emit_str!("n", v);
  }
  if let Some(ref v) = jwk.e {
    emit_str!("e", v);
  }
  if let Some(ref v) = jwk.d {
    emit_str!("d", v);
  }
  if let Some(ref v) = jwk.p {
    emit_str!("p", v);
  }
  if let Some(ref v) = jwk.q {
    emit_str!("q", v);
  }
  if let Some(ref v) = jwk.dp {
    emit_str!("dp", v);
  }
  if let Some(ref v) = jwk.dq {
    emit_str!("dq", v);
  }
  if let Some(ref v) = jwk.qi {
    emit_str!("qi", v);
  }
  if let Some(ref v) = jwk.x {
    emit_str!("x", v);
  }
  if let Some(ref v) = jwk.y {
    emit_str!("y", v);
  }
  if let Some(ref v) = jwk.pub_field {
    emit_str!("pub", v);
  }
  if let Some(ref v) = jwk.priv_field {
    emit_str!("priv", v);
  }
  if !jwk.key_ops.is_empty() {
    if !first {
      out.push(',');
    }
    first = false;
    out.push_str(r#""key_ops":["#);
    for (i, op) in jwk.key_ops.iter().enumerate() {
      if i > 0 {
        out.push(',');
      }
      out.push('"');
      out.push_str(&json_escape(op));
      out.push('"');
    }
    out.push(']');
  }
  if !first {
    out.push(',');
  }
  out.push_str(r#""ext":"#);
  out.push_str(if jwk.ext { "true" } else { "false" });
  out.push('}');
  out.into_bytes()
}

fn json_escape(s: &str) -> String {
  // The JWK members the export path produces are base64url-encoded or
  // ASCII-only spec-defined identifiers; no characters need escaping in
  // practice, but quote/backslash/control-char escaping is implemented for
  // defensive correctness.
  let mut out = String::with_capacity(s.len());
  for c in s.chars() {
    match c {
      '"' => out.push_str(r#"\""#),
      '\\' => out.push_str(r"\\"),
      '\n' => out.push_str(r"\n"),
      '\r' => out.push_str(r"\r"),
      '\t' => out.push_str(r"\t"),
      c if (c as u32) < 0x20 => {
        out.push_str(&format!("\\u{:04x}", c as u32));
      }
      c => out.push(c),
    }
  }
  out
}

fn utf8_to_v8_json<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: &[u8],
) -> Result<v8::Local<'s, v8::Value>, CryptoError> {
  let s = std::str::from_utf8(bytes)
    .map_err(|_| data_error("Wrapped key is not valid UTF-8".into()))?;
  let v8_str = v8::String::new(scope, s)
    .ok_or_else(|| data_error("Failed to allocate v8 string".into()))?;
  let val = v8::json::parse(scope, v8_str)
    .ok_or_else(|| data_error("Wrapped key is not valid JSON".into()))?;
  Ok(val)
}

fn invalid_access(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionInvalidAccessError", msg))
}

fn data_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionDataError", msg))
}

fn syntax_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionSyntaxError", msg))
}
