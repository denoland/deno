// Copyright 2018-2026 the Deno authors. MIT license.

//! The "web" (RSA / EC / HMAC / AES / AES-KW / ChaCha20-Poly1305 / HKDF /
//! PBKDF2) branch of `SubtleCrypto.importKey` / `exportKey`, ported from the
//! `importKeyXXX` / `exportKeyXXX` JS helpers.
//!
//! [`import_key_web_inner`] and [`export_key_web_inner`] are plain `pub fn`s
//! (no `#[op2]`); they are called directly from the cppgc `SubtleCrypto`
//! `importKeySync` / `exportKeySync` methods (`web_keymaker.rs`), which build
//! the `CryptoKey` from the returned [`ImportKeyWebResult`] / serialize the
//! [`ExportKeyWebResult`]. The per-curve / post-quantum algorithms
//! (`Ed25519`, `X25519`, `X448`, `ML-DSA-*`, `ML-KEM-*`) are handled directly
//! in `web_keymaker.rs` via the `pub fn` cores in their respective modules.
//!
//! The actual key parsing/serialization is reused from `import_key.rs` /
//! `export_key.rs` (`import_key_inner` / `export_key_inner`), dispatched on the
//! same enums, so the DER/JWK number handling is not replicated.
//!
//! All `DOMException` class strings used here (`DataError`, `NotSupportedError`,
//! `SyntaxError` (registered), `OperationError`, `InvalidAccessError`) have a
//! registered builder in `runtime/js/99_main.js`.

use deno_core::JsBuffer;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use serde::Deserialize;

use crate::export_key::ExportKeyAlgorithm;
use crate::export_key::ExportKeyError;
use crate::export_key::ExportKeyFormat;
use crate::export_key::ExportKeyOptions;
use crate::export_key::ExportKeyResult;
use crate::export_key::export_key_inner;
use crate::import_key::ImportKeyError;
use crate::import_key::ImportKeyOptions;
use crate::import_key::ImportKeyResult;
use crate::import_key::KeyData;
use crate::import_key::import_key_inner;
use crate::shared::EcNamedCurve;
use crate::shared::RustRawKeyData;
use crate::shared::V8RawKeyData;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that mirror the `DOMException`s thrown by the JS `importKey` /
/// `exportKey` helpers. The underlying DER/JWK parsing surfaces
/// [`ImportKeyError`] (`DataError`) / [`ExportKeyError`].
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebImportExportError {
  #[class("DOMExceptionDataError")]
  #[error("{0}")]
  Data(String),
  #[class("DOMExceptionSyntaxError")]
  #[error("{0}")]
  Syntax(String),
  #[class("DOMExceptionNotSupportedError")]
  #[error("{0}")]
  NotSupported(String),
  #[class("DOMExceptionInvalidAccessError")]
  #[error("{0}")]
  InvalidAccess(String),
  #[class(type)]
  #[error("{0}")]
  Type(String),
  #[class(inherit)]
  #[error(transparent)]
  Import(
    #[from]
    #[inherit]
    ImportKeyError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Export(
    #[from]
    #[inherit]
    ExportKeyError,
  ),
}

type Res<T> = Result<T, WebImportExportError>;

fn data(msg: impl Into<String>) -> WebImportExportError {
  WebImportExportError::Data(msg.into())
}
fn syntax(msg: impl Into<String>) -> WebImportExportError {
  WebImportExportError::Syntax(msg.into())
}
fn not_supported(msg: impl Into<String>) -> WebImportExportError {
  WebImportExportError::NotSupported(msg.into())
}

// ---------------------------------------------------------------------------
// Shared constants (mirroring 00_crypto.js)
// ---------------------------------------------------------------------------

const RECOGNISED_USAGES: &[&str] = &[
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

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

/// `SUPPORTED_KEY_USAGES[name]` from 00_crypto.js.
struct SupportedKeyUsages {
  public: &'static [&'static str],
  private: &'static [&'static str],
  jwk_use: &'static str,
}

fn supported_key_usages(name: &str) -> Option<SupportedKeyUsages> {
  Some(match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => SupportedKeyUsages {
      public: &["verify"],
      private: &["sign"],
      jwk_use: "sig",
    },
    "RSA-OAEP" => SupportedKeyUsages {
      public: &["encrypt", "wrapKey"],
      private: &["decrypt", "unwrapKey"],
      jwk_use: "enc",
    },
    "ECDSA" => SupportedKeyUsages {
      public: &["verify"],
      private: &["sign"],
      jwk_use: "sig",
    },
    "ECDH" => SupportedKeyUsages {
      public: &[],
      private: &["deriveKey", "deriveBits"],
      jwk_use: "enc",
    },
    _ => return None,
  })
}

/// `aesJwkAlg[name][length]`.
fn aes_jwk_alg(name: &str, length: usize) -> Option<&'static str> {
  Some(match (name, length) {
    ("AES-CTR", 128) => "A128CTR",
    ("AES-CTR", 192) => "A192CTR",
    ("AES-CTR", 256) => "A256CTR",
    ("AES-CBC", 128) => "A128CBC",
    ("AES-CBC", 192) => "A192CBC",
    ("AES-CBC", 256) => "A256CBC",
    ("AES-GCM", 128) => "A128GCM",
    ("AES-GCM", 192) => "A192GCM",
    ("AES-GCM", 256) => "A256GCM",
    ("AES-KW", 128) => "A128KW",
    ("AES-KW", 192) => "A192KW",
    ("AES-KW", 256) => "A256KW",
    _ => return None,
  })
}

fn usage_intersection(a: &[String], b: &[&str]) -> Vec<String> {
  a.iter()
    .filter(|u| b.contains(&u.as_str()))
    .cloned()
    .collect()
}

/// `ArrayPrototypeFind(keyUsages, u => !includes(allowed, u)) !== undefined`
fn has_disallowed_usage(usages: &[String], allowed: &[&str]) -> bool {
  usages.iter().any(|u| !allowed.contains(&u.as_str()))
}

// ---------------------------------------------------------------------------
// JWK helpers
// ---------------------------------------------------------------------------

/// A minimal view over the incoming JWK. We deserialize to `serde_json::Value`
/// so optional members behave exactly like JS `=== undefined` checks (a missing
/// member is `None`, an explicit `null` is `Some(Value::Null)` which is treated
/// as present, matching the JS `!== undefined` semantics).
fn jwk_str<'a>(jwk: &'a Value, key: &str) -> Option<&'a str> {
  jwk.get(key).and_then(|v| v.as_str())
}

fn jwk_has(jwk: &Value, key: &str) -> bool {
  jwk.get(key).is_some()
}

/// `jwk.key_ops` validation shared by RSA/EC/AES/HMAC:
/// every entry must be a recognised usage, and every requested key usage must be
/// present in `key_ops`. The "are key usages a subset of key_ops" check uses the
/// `keyUsages` array for RSA/EC/AES/HMAC.
fn validate_key_ops(jwk: &Value, key_usages: &[String]) -> Res<()> {
  if let Some(key_ops) = jwk.get("key_ops") {
    let ops: Vec<String> = match key_ops.as_array() {
      Some(arr) => arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect(),
      // Non-array key_ops: treat entries as unrecognised -> invalid.
      None => return Err(data("'key_ops' property of JsonWebKey is invalid")),
    };
    if ops.iter().any(|u| !RECOGNISED_USAGES.contains(&u.as_str())) {
      return Err(data("'key_ops' property of JsonWebKey is invalid"));
    }
    if !key_usages.iter().all(|u| ops.contains(u)) {
      return Err(data("'key_ops' property of JsonWebKey is invalid"));
    }
  }
  Ok(())
}

fn validate_ext(jwk: &Value, extractable: bool) -> Res<()> {
  if jwk.get("ext") == Some(&Value::Bool(false)) && extractable {
    return Err(data(
      "'ext' property of JsonWebKey must not be false if extractable is true",
    ));
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// op input / output shapes
// ---------------------------------------------------------------------------

/// The (already `normalizeAlgorithm`-d) algorithm fields the JS stub forwards.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportKeyWebArg {
  /// "raw" | "spki" | "pkcs8" | "jwk".
  pub format: String,
  /// Normalized algorithm name (already canonical-cased by `normalizeAlgorithm`).
  pub name: String,
  /// `normalizedAlgorithm.hash.name` (RSA/HMAC).
  pub hash: Option<String>,
  /// `normalizedAlgorithm.namedCurve` (EC).
  pub named_curve: Option<String>,
  /// `normalizedAlgorithm.length` (HMAC import length override, in bits).
  pub length: Option<usize>,
  pub extractable: bool,
  pub key_usages: Vec<String>,
}

/// The raw key data: either a buffer (raw/spki/pkcs8) or a JWK object. The op
/// receives these as two separate (optional) arguments rather than an untagged
/// enum, because a zero-copy `JsBuffer` cannot be deserialized through serde's
/// untagged-enum content buffering. The JS stub passes exactly one.
pub enum ImportKeyWebData {
  Buffer(JsBuffer),
  Jwk(Value),
}

/// The result of a "web" key import. The cppgc `SubtleCrypto::importKey` method
/// builds a `CryptoKey` directly from these fields.
pub struct ImportKeyWebResult {
  /// "public" | "private" | "secret".
  pub key_type: String,
  /// Final (intersected) usages.
  pub usages: Vec<String>,
  /// The `algorithm` dict members to attach to the CryptoKey.
  pub algorithm: AlgorithmDict,
  /// `{ type, data }` key material: `data_type` is "secret"|"private"|"public",
  /// `data` is the raw key material.
  pub data_type: String,
  pub data: Vec<u8>,
}

/// Split a [`RustRawKeyData`] into the `(type, bytes)` pair.
fn split_raw(raw: RustRawKeyData) -> (String, Vec<u8>) {
  match raw {
    RustRawKeyData::Secret(b) => ("secret".to_string(), b.to_vec()),
    RustRawKeyData::Private(b) => ("private".to_string(), b.to_vec()),
    RustRawKeyData::Public(b) => ("public".to_string(), b.to_vec()),
  }
}

pub struct AlgorithmDict {
  pub name: String,
  pub modulus_length: Option<usize>,
  pub public_exponent: Option<Vec<u8>>,
  /// Hash *name* (RSA/HMAC).
  pub hash: Option<String>,
  pub named_curve: Option<String>,
  pub length: Option<usize>,
}

impl AlgorithmDict {
  fn name_only(name: &str) -> Self {
    AlgorithmDict {
      name: name.to_string(),
      modulus_length: None,
      public_exponent: None,
      hash: None,
      named_curve: None,
      length: None,
    }
  }
}

// ---------------------------------------------------------------------------
// importKey op
// ---------------------------------------------------------------------------

/// Canonicalize a WICG modern-algorithms key format into the legacy format the
/// per-algorithm import/export code understands.
///
/// * Existing symmetric algorithms (AES-*, HMAC, HKDF, PBKDF2): `raw` and
///   `raw-secret` are aliases, both mapped to `raw`.
/// * ChaCha20-Poly1305 (a new symmetric algorithm): only `raw-secret` is
///   recognized, mapped to `raw`; bare `raw` is rejected.
/// * Existing asymmetric algorithms (ECDSA/ECDH): `raw` and `raw-public` are
///   aliases for the public-key raw format, both mapped to `raw`.
///
/// Other formats (`spki`/`pkcs8`/`jwk`) pass through unchanged. An invalid
/// format for the algorithm class yields a `NotSupportedError`.
fn normalize_web_format(name: &str, format: &str) -> Res<String> {
  let canonical = match name {
    "ChaCha20-Poly1305" => match format {
      "raw-secret" => "raw",
      // A new symmetric algorithm does not recognize the legacy `raw`.
      "raw" => {
        return Err(not_supported(
          "ChaCha20-Poly1305 does not support the 'raw' format; use \
           'raw-secret'",
        ));
      }
      other => other,
    },
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW" | "HMAC"
    | "HKDF" | "PBKDF2" => match format {
      "raw-secret" => "raw",
      other => other,
    },
    "ECDSA" | "ECDH" => match format {
      "raw-public" => "raw",
      other => other,
    },
    _ => format,
  };
  Ok(canonical.to_string())
}

/// The "web" (RSA/EC/HMAC/AES/ChaCha/HKDF/PBKDF2) `importKey` dispatch, callable
/// directly from the cppgc `SubtleCrypto::importKey` method.
pub fn import_key_web_inner(
  mut arg: ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Result<ImportKeyWebResult, WebImportExportError> {
  arg.format = normalize_web_format(&arg.name, &arg.format)?;
  match arg.name.as_str() {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => import_rsa(&arg, key_data),
    "ECDSA" | "ECDH" => import_ec(&arg, key_data),
    "HMAC" => import_hmac(&arg, key_data),
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" => import_aes(
      &arg,
      key_data,
      &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
    ),
    "AES-KW" => import_aes(&arg, key_data, &["wrapKey", "unwrapKey"]),
    "ChaCha20-Poly1305" => import_chacha(&arg, key_data),
    "HKDF" => import_hkdf(&arg, key_data),
    "PBKDF2" => import_pbkdf2(&arg, key_data),
    _ => Err(not_supported("Not implemented")),
  }
}

/// Pull the buffer out for raw/spki/pkcs8 formats (JS asserts this before the
/// op runs; if it's a JWK in a non-jwk format the stub already threw a
/// `TypeError`). For the `jwk` format we expect a `Value`.
fn expect_buffer(key_data: ImportKeyWebData) -> Res<JsBuffer> {
  match key_data {
    ImportKeyWebData::Buffer(b) => Ok(b),
    ImportKeyWebData::Jwk(_) => {
      Err(WebImportExportError::Type("Expected a BufferSource".into()))
    }
  }
}

fn expect_jwk(key_data: ImportKeyWebData) -> Res<Value> {
  match key_data {
    ImportKeyWebData::Jwk(v) => Ok(v),
    // A typed array deserializes via the untagged enum to Buffer; JS guards
    // this, so this branch is effectively unreachable.
    ImportKeyWebData::Buffer(_) => {
      Err(WebImportExportError::Type("Expected a JsonWebKey".into()))
    }
  }
}

fn finish_rsa(
  res: ImportKeyResult,
  name: &str,
  hash: &str,
  key_type: &str,
  usages: Vec<String>,
) -> Res<ImportKeyWebResult> {
  let ImportKeyResult::Rsa {
    raw_data,
    modulus_length,
    public_exponent,
  } = res
  else {
    return Err(data("invalid key data"));
  };
  let (data_type, data) = split_raw(raw_data);
  Ok(ImportKeyWebResult {
    key_type: key_type.to_string(),
    usages,
    algorithm: AlgorithmDict {
      name: name.to_string(),
      modulus_length: Some(modulus_length),
      public_exponent: Some(public_exponent.to_vec()),
      hash: Some(hash.to_string()),
      named_curve: None,
      length: None,
    },
    data_type,
    data,
  })
}

// ----- RSA -----------------------------------------------------------------

fn import_rsa(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  let name = arg.name.as_str();
  let hash = arg.hash.as_deref().ok_or_else(|| data("missing hash"))?;
  let sk = supported_key_usages(name).unwrap();

  match arg.format.as_str() {
    "pkcs8" => {
      if has_disallowed_usage(&arg.key_usages, sk.private) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = expect_buffer(key_data)?;
      let res = import_key_inner(rsa_opts(name), KeyData::Pkcs8(buf))?;
      finish_rsa(
        res,
        name,
        hash,
        "private",
        usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
      )
    }
    "spki" => {
      if has_disallowed_usage(&arg.key_usages, sk.public) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = expect_buffer(key_data)?;
      let res = import_key_inner(rsa_opts(name), KeyData::Spki(buf))?;
      finish_rsa(
        res,
        name,
        hash,
        "public",
        usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
      )
    }
    "jwk" => import_rsa_jwk(arg, expect_jwk(key_data)?, hash, &sk),
    _ => Err(not_supported("Not implemented")),
  }
}

fn rsa_opts(name: &str) -> ImportKeyOptions {
  match name {
    "RSASSA-PKCS1-v1_5" => ImportKeyOptions::RsassaPkcs1v15 {},
    "RSA-PSS" => ImportKeyOptions::RsaPss {},
    _ => ImportKeyOptions::RsaOaep {},
  }
}

fn import_rsa_jwk(
  arg: &ImportKeyWebArg,
  jwk: Value,
  hash_name: &str,
  sk: &SupportedKeyUsages,
) -> Res<ImportKeyWebResult> {
  let name = arg.name.as_str();
  let is_private = jwk_has(&jwk, "d");

  // 2.
  if is_private {
    if has_disallowed_usage(&arg.key_usages, sk.private) {
      return Err(syntax("Invalid key usage"));
    }
  } else if has_disallowed_usage(&arg.key_usages, sk.public) {
    return Err(syntax("Invalid key usage"));
  }

  // 3.
  match jwk_str(&jwk, "kty") {
    Some(kty) if kty.eq_ignore_ascii_case("RSA") => {}
    _ => return Err(data("'kty' property of JsonWebKey must be 'RSA'")),
  }

  // 4.
  if !arg.key_usages.is_empty()
    && let Some(usev) = jwk_str(&jwk, "use")
    && usev.to_ascii_lowercase() != sk.jwk_use
  {
    return Err(data(format!(
      "'use' property of JsonWebKey must be '{}'",
      sk.jwk_use
    )));
  }

  // 5.
  validate_key_ops(&jwk, &arg.key_usages)?;

  // 6.
  validate_ext(&jwk, arg.extractable)?;

  // 7-9. `alg` -> hash, then compare to normalizedAlgorithm.hash.name.
  let alg = jwk_str(&jwk, "alg");
  let mapped_hash = match name {
    "RSASSA-PKCS1-v1_5" => map_alg(
      alg,
      &[
        ("RS1", "SHA-1"),
        ("RS256", "SHA-256"),
        ("RS384", "SHA-384"),
        ("RS512", "SHA-512"),
        ("RS3-256", "SHA3-256"),
        ("RS3-384", "SHA3-384"),
        ("RS3-512", "SHA3-512"),
      ],
      "'alg' property of JsonWebKey must be one of 'RS1', 'RS256', 'RS384', 'RS512', 'RS3-256', 'RS3-384', 'RS3-512'",
    )?,
    "RSA-PSS" => map_alg(
      alg,
      &[
        ("PS1", "SHA-1"),
        ("PS256", "SHA-256"),
        ("PS384", "SHA-384"),
        ("PS512", "SHA-512"),
        ("PS3-256", "SHA3-256"),
        ("PS3-384", "SHA3-384"),
        ("PS3-512", "SHA3-512"),
      ],
      "'alg' property of JsonWebKey must be one of 'PS1', 'PS256', 'PS384', 'PS512', 'PS3-256', 'PS3-384', 'PS3-512'",
    )?,
    _ => map_alg(
      alg,
      &[
        ("RSA-OAEP", "SHA-1"),
        ("RSA-OAEP-256", "SHA-256"),
        ("RSA-OAEP-384", "SHA-384"),
        ("RSA-OAEP-512", "SHA-512"),
        ("RSA-OAEP3-256", "SHA3-256"),
        ("RSA-OAEP3-384", "SHA3-384"),
        ("RSA-OAEP3-512", "SHA3-512"),
      ],
      "'alg' property of JsonWebKey must be one of 'RSA-OAEP', 'RSA-OAEP-256', 'RSA-OAEP-384', 'RSA-OAEP-512', 'RSA-OAEP3-256', 'RSA-OAEP3-384', or 'RSA-OAEP3-512'",
    )?,
  };
  if let Some(h) = mapped_hash
    && h != hash_name
  {
    return Err(data(format!(
      "'alg' property of JsonWebKey must be '{}': received {}",
      name,
      alg.unwrap_or("")
    )));
  }

  // 10.
  if is_private {
    let optimizations_present = jwk_has(&jwk, "p")
      || jwk_has(&jwk, "q")
      || jwk_has(&jwk, "dp")
      || jwk_has(&jwk, "dq")
      || jwk_has(&jwk, "qi");
    if optimizations_present {
      if !jwk_has(&jwk, "q") {
        return Err(data(
          "'q' property of JsonWebKey is required for private keys",
        ));
      }
      if !jwk_has(&jwk, "dp") {
        return Err(data(
          "'dp' property of JsonWebKey is required for private keys",
        ));
      }
      if !jwk_has(&jwk, "dq") {
        return Err(data(
          "'dq' property of JsonWebKey is required for private keys",
        ));
      }
      if !jwk_has(&jwk, "qi") {
        return Err(data(
          "'qi' property of JsonWebKey is required for private keys",
        ));
      }
      if jwk_has(&jwk, "oth") {
        return Err(not_supported(
          "'oth' property of JsonWebKey is not supported",
        ));
      }
    } else {
      return Err(not_supported("Only optimized private keys are supported"));
    }

    let res = import_key_inner(rsa_opts(name), jwk_private_rsa(&jwk)?)?;
    finish_rsa(
      res,
      name,
      hash_name,
      "private",
      usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    )
  } else {
    if !jwk_has(&jwk, "n") {
      return Err(data(
        "'n' property of JsonWebKey is required for public keys",
      ));
    }
    if !jwk_has(&jwk, "e") {
      return Err(data(
        "'e' property of JsonWebKey is required for public keys",
      ));
    }
    let res = import_key_inner(rsa_opts(name), jwk_public_rsa(&jwk)?)?;
    finish_rsa(
      res,
      name,
      hash_name,
      "public",
      usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    )
  }
}

/// JS step 8: map `jwk.alg` to a hash name. `None` alg -> `Ok(None)`.
fn map_alg(
  alg: Option<&str>,
  table: &[(&str, &str)],
  err_prefix: &str,
) -> Res<Option<String>> {
  match alg {
    None => Ok(None),
    Some(a) => match table.iter().find(|(k, _)| *k == a) {
      Some((_, h)) => Ok(Some((*h).to_string())),
      None => Err(data(format!("{err_prefix}: received {a}"))),
    },
  }
}

fn jwk_string(jwk: &Value, key: &str) -> Res<String> {
  jwk_str(jwk, key)
    .map(|s| s.to_string())
    .ok_or_else(|| data(format!("invalid '{key}'")))
}

fn jwk_public_rsa(jwk: &Value) -> Res<KeyData> {
  Ok(KeyData::JwkPublicRsa {
    n: jwk_string(jwk, "n")?,
    e: jwk_string(jwk, "e")?,
  })
}

fn jwk_private_rsa(jwk: &Value) -> Res<KeyData> {
  Ok(KeyData::JwkPrivateRsa {
    n: jwk_string(jwk, "n")?,
    e: jwk_string(jwk, "e")?,
    d: jwk_string(jwk, "d")?,
    p: jwk_string(jwk, "p")?,
    q: jwk_string(jwk, "q")?,
    dp: jwk_string(jwk, "dp")?,
    dq: jwk_string(jwk, "dq")?,
    qi: jwk_string(jwk, "qi")?,
  })
}

// ----- EC ------------------------------------------------------------------

fn ec_named_curve(s: &str) -> Res<EcNamedCurve> {
  Ok(match s {
    "P-256" => EcNamedCurve::P256,
    "P-384" => EcNamedCurve::P384,
    "P-521" => EcNamedCurve::P521,
    _ => return Err(data("Invalid namedCurve")),
  })
}

fn ec_opts(name: &str, curve: EcNamedCurve) -> ImportKeyOptions {
  match name {
    "ECDSA" => ImportKeyOptions::Ecdsa { named_curve: curve },
    _ => ImportKeyOptions::Ecdh { named_curve: curve },
  }
}

fn finish_ec(
  res: ImportKeyResult,
  name: &str,
  curve: &str,
  key_type: &str,
  usages: Vec<String>,
) -> Res<ImportKeyWebResult> {
  let ImportKeyResult::Ec { raw_data } = res else {
    return Err(data("invalid key data"));
  };
  let (data_type, data) = split_raw(raw_data);
  Ok(ImportKeyWebResult {
    key_type: key_type.to_string(),
    usages,
    algorithm: AlgorithmDict {
      name: name.to_string(),
      modulus_length: None,
      public_exponent: None,
      hash: None,
      named_curve: Some(curve.to_string()),
      length: None,
    },
    data_type,
    data,
  })
}

fn import_ec(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  let name = arg.name.as_str();
  let curve = arg
    .named_curve
    .as_deref()
    .ok_or_else(|| data("missing namedCurve"))?;
  let sk = supported_key_usages(name).unwrap();

  match arg.format.as_str() {
    "raw" => {
      // 1.
      if !SUPPORTED_NAMED_CURVES.contains(&curve) {
        return Err(data("Invalid namedCurve"));
      }
      // 2.
      if has_disallowed_usage(&arg.key_usages, sk.public) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = expect_buffer(key_data)?;
      let res = import_key_inner(
        ec_opts(name, ec_named_curve(curve)?),
        KeyData::Raw(buf),
      )?;
      finish_ec(
        res,
        name,
        curve,
        "public",
        usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
      )
    }
    "pkcs8" => {
      if has_disallowed_usage(&arg.key_usages, sk.private) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = expect_buffer(key_data)?;
      let res = import_key_inner(
        ec_opts(name, ec_named_curve(curve)?),
        KeyData::Pkcs8(buf),
      )?;
      finish_ec(
        res,
        name,
        curve,
        "private",
        usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
      )
    }
    "spki" => {
      // 1.
      if name == "ECDSA" {
        if has_disallowed_usage(&arg.key_usages, sk.public) {
          return Err(syntax("Invalid key usage"));
        }
      } else if !arg.key_usages.is_empty() {
        return Err(syntax("Key usage must be empty"));
      }
      let buf = expect_buffer(key_data)?;
      let res = import_key_inner(
        ec_opts(name, ec_named_curve(curve)?),
        KeyData::Spki(buf),
      )?;
      finish_ec(
        res,
        name,
        curve,
        "public",
        usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
      )
    }
    "jwk" => import_ec_jwk(arg, expect_jwk(key_data)?, curve, &sk),
    _ => Err(not_supported("Not implemented")),
  }
}

fn import_ec_jwk(
  arg: &ImportKeyWebArg,
  jwk: Value,
  curve: &str,
  sk: &SupportedKeyUsages,
) -> Res<ImportKeyWebResult> {
  let name = arg.name.as_str();
  let is_private = jwk_has(&jwk, "d");
  let allowed = if is_private { sk.private } else { sk.public };

  // 2.
  if has_disallowed_usage(&arg.key_usages, allowed) {
    return Err(syntax("Invalid key usage"));
  }

  // 3.
  if jwk_str(&jwk, "kty") != Some("EC") {
    return Err(data("'kty' property of JsonWebKey must be 'EC'"));
  }

  // 4.
  if !arg.key_usages.is_empty()
    && let Some(usev) = jwk_str(&jwk, "use")
    && usev != sk.jwk_use
  {
    return Err(data(format!(
      "'use' property of JsonWebKey must be '{}'",
      sk.jwk_use
    )));
  }

  // 5.
  validate_key_ops(&jwk, &arg.key_usages)?;

  // 6.
  validate_ext(&jwk, arg.extractable)?;

  // 9. alg vs curve (ECDSA only).
  if name == "ECDSA"
    && let Some(alg) = jwk_str(&jwk, "alg")
  {
    let alg_curve = match alg {
      "ES256" => "P-256",
      "ES384" => "P-384",
      "ES512" => "P-521",
      _ => return Err(data("Curve algorithm not supported")),
    };
    if alg_curve != curve {
      return Err(data("Mismatched curve algorithm"));
    }
  }

  // x / y required.
  if !jwk_has(&jwk, "x") {
    return Err(data("'x' property of JsonWebKey is required for EC keys"));
  }
  if !jwk_has(&jwk, "y") {
    return Err(data("'y' property of JsonWebKey is required for EC keys"));
  }

  if is_private {
    let res = import_key_inner(
      ec_opts(name, ec_named_curve(curve)?),
      KeyData::JwkPrivateEc {
        x: jwk_string(&jwk, "x")?,
        y: jwk_string(&jwk, "y")?,
        d: jwk_string(&jwk, "d")?,
      },
    )?;
    finish_ec(
      res,
      name,
      curve,
      "private",
      usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    )
  } else {
    let res = import_key_inner(
      ec_opts(name, ec_named_curve(curve)?),
      KeyData::JwkPublicEc {
        x: jwk_string(&jwk, "x")?,
        y: jwk_string(&jwk, "y")?,
      },
    )?;
    finish_ec(
      res,
      name,
      curve,
      "public",
      usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    )
  }
}

// ----- HMAC ----------------------------------------------------------------

fn import_hmac(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  // 2.
  if has_disallowed_usage(&arg.key_usages, &["sign", "verify"]) {
    return Err(syntax("Invalid key usage"));
  }
  let hash = arg.hash.as_deref().ok_or_else(|| data("missing hash"))?;

  let data_bytes: Vec<u8> = match arg.format.as_str() {
    "raw" => expect_buffer(key_data)?.to_vec(),
    "jwk" => {
      let jwk = expect_jwk(key_data)?;
      // 2.
      if jwk_str(&jwk, "kty") != Some("oct") {
        return Err(data("'kty' property of JsonWebKey must be 'oct'"));
      }
      if !jwk_has(&jwk, "k") {
        return Err(data("'k' property of JsonWebKey must be present"));
      }
      let bytes = decode_jwk_k(&jwk)?;
      // 6. alg vs hash.
      let expected_alg = match hash {
        "SHA-1" => "HS1",
        "SHA-256" => "HS256",
        "SHA-384" => "HS384",
        "SHA-512" => "HS512",
        "SHA3-256" => "HS3-256",
        "SHA3-384" => "HS3-384",
        "SHA3-512" => "HS3-512",
        _ => return Err(WebImportExportError::Type("Unreachable".into())),
      };
      if let Some(alg) = jwk_str(&jwk, "alg")
        && alg != expected_alg
      {
        return Err(data(format!(
          "'alg' property of JsonWebKey must be '{expected_alg}'"
        )));
      }
      // 7. use.
      if !arg.key_usages.is_empty()
        && let Some(usev) = jwk_str(&jwk, "use")
        && usev != "sig"
      {
        return Err(data("'use' property of JsonWebKey must be 'sig'"));
      }
      // 8. key_ops.
      validate_key_ops(&jwk, &arg.key_usages)?;
      // 9. ext.
      validate_ext(&jwk, arg.extractable)?;
      bytes
    }
    _ => return Err(not_supported("Not implemented")),
  };

  // 5-7. length handling.
  let mut length = data_bytes.len() * 8;
  if length == 0 {
    return Err(data("Key length is zero"));
  }
  if let Some(req) = arg.length {
    if req > length || req <= length.saturating_sub(8) {
      return Err(data("Key length is invalid"));
    }
    length = req;
  }

  Ok(ImportKeyWebResult {
    key_type: "secret".to_string(),
    usages: usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    algorithm: AlgorithmDict {
      name: "HMAC".to_string(),
      modulus_length: None,
      public_exponent: None,
      hash: Some(hash.to_string()),
      named_curve: None,
      length: Some(length),
    },
    data_type: "secret".to_string(),
    data: data_bytes,
  })
}

/// Decode a JWK `k` (base64url, forgiving) into bytes via the existing
/// `import_key` JWK-secret path so the decoding rules match exactly.
fn decode_jwk_k(jwk: &Value) -> Res<Vec<u8>> {
  let k = jwk_string(jwk, "k")?;
  let res =
    import_key_inner(ImportKeyOptions::Hmac {}, KeyData::JwkSecret { k })?;
  match res {
    ImportKeyResult::Hmac {
      raw_data: RustRawKeyData::Secret(b),
    } => Ok(b.to_vec()),
    _ => Err(data("invalid key data")),
  }
}

// ----- AES -----------------------------------------------------------------

fn import_aes(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
  supported_usages: &[&str],
) -> Res<ImportKeyWebResult> {
  let name = arg.name.as_str();
  // 1.
  if has_disallowed_usage(&arg.key_usages, supported_usages) {
    return Err(syntax("Invalid key usage"));
  }

  let bytes: Vec<u8> = match arg.format.as_str() {
    "raw" => {
      let buf = expect_buffer(key_data)?;
      let bits = buf.len() * 8;
      if !matches!(bits, 128 | 192 | 256) {
        return Err(data("Invalid key length"));
      }
      buf.to_vec()
    }
    "jwk" => {
      let jwk = expect_jwk(key_data)?;
      // 2.
      if jwk_str(&jwk, "kty") != Some("oct") {
        return Err(data("'kty' property of JsonWebKey must be 'oct'"));
      }
      if !jwk_has(&jwk, "k") {
        return Err(data("'k' property of JsonWebKey must be present"));
      }
      // 4.
      let bytes = decode_jwk_k(&jwk)?;
      // 5. length + alg.
      let bits = bytes.len() * 8;
      if !matches!(bits, 128 | 192 | 256) {
        return Err(data("Invalid key length"));
      }
      if let Some(alg) = jwk_str(&jwk, "alg")
        && Some(alg) != aes_jwk_alg(name, bits)
      {
        return Err(data(format!("Invalid algorithm: {alg}")));
      }
      // 6. use.
      if !arg.key_usages.is_empty()
        && let Some(usev) = jwk_str(&jwk, "use")
        && usev != "enc"
      {
        return Err(data("Invalid key usage"));
      }
      // 7. key_ops.
      validate_key_ops(&jwk, &arg.key_usages)?;
      // 8. ext.
      validate_ext(&jwk, arg.extractable)?;
      bytes
    }
    _ => return Err(not_supported("Not implemented")),
  };

  let length = bytes.len() * 8;
  Ok(ImportKeyWebResult {
    key_type: "secret".to_string(),
    usages: usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    algorithm: AlgorithmDict {
      name: name.to_string(),
      modulus_length: None,
      public_exponent: None,
      hash: None,
      named_curve: None,
      length: Some(length),
    },
    data_type: "secret".to_string(),
    data: bytes,
  })
}

// ----- ChaCha20-Poly1305 ---------------------------------------------------

fn import_chacha(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  if has_disallowed_usage(
    &arg.key_usages,
    &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
  ) {
    return Err(syntax("Invalid key usage"));
  }
  let bytes: Vec<u8> = match arg.format.as_str() {
    "raw" => {
      let buf = expect_buffer(key_data)?;
      if buf.len() != 32 {
        return Err(data(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key",
        ));
      }
      buf.to_vec()
    }
    "jwk" => {
      let jwk = expect_jwk(key_data)?;
      if jwk_str(&jwk, "kty") != Some("oct") {
        return Err(data("'kty' property of JsonWebKey must be 'oct'"));
      }
      if !jwk_has(&jwk, "k") {
        return Err(data("'k' property of JsonWebKey must be present"));
      }
      let bytes = decode_jwk_k(&jwk)?;
      if bytes.len() != 32 {
        return Err(data(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key",
        ));
      }
      // The JWK `alg`, if present, must be "C20P".
      if let Some(alg) = jwk_str(&jwk, "alg")
        && alg != "C20P"
      {
        return Err(data("'alg' property of JsonWebKey must be 'C20P'"));
      }
      if !arg.key_usages.is_empty()
        && let Some(usev) = jwk_str(&jwk, "use")
        && usev != "enc"
      {
        return Err(data("'use' property of JsonWebKey must be 'enc'"));
      }
      validate_key_ops(&jwk, &arg.key_usages)?;
      validate_ext(&jwk, arg.extractable)?;
      bytes
    }
    _ => return Err(not_supported("Not implemented")),
  };
  Ok(ImportKeyWebResult {
    key_type: "secret".to_string(),
    usages: usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    algorithm: AlgorithmDict::name_only("ChaCha20-Poly1305"),
    data_type: "secret".to_string(),
    data: bytes,
  })
}

// ----- HKDF / PBKDF2 -------------------------------------------------------

fn import_hkdf(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  import_kdf(arg, key_data, "HKDF")
}

fn import_pbkdf2(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
) -> Res<ImportKeyWebResult> {
  import_kdf(arg, key_data, "PBKDF2")
}

fn import_kdf(
  arg: &ImportKeyWebArg,
  key_data: ImportKeyWebData,
  name: &str,
) -> Res<ImportKeyWebResult> {
  if arg.format != "raw" {
    return Err(not_supported("Format not supported"));
  }
  if has_disallowed_usage(&arg.key_usages, &["deriveKey", "deriveBits"]) {
    return Err(syntax("Invalid key usage"));
  }
  if arg.extractable {
    return Err(syntax("Key must not be extractable"));
  }
  let buf = expect_buffer(key_data)?;
  Ok(ImportKeyWebResult {
    key_type: "secret".to_string(),
    // JS passes `false` (not `extractable`) to constructKey for KDF keys; the
    // stub hard-codes that, so the value here is only used for the raw data.
    usages: usage_intersection(&arg.key_usages, RECOGNISED_USAGES),
    algorithm: AlgorithmDict::name_only(name),
    data_type: "secret".to_string(),
    data: buf.to_vec(),
  })
}

// ---------------------------------------------------------------------------
// exportKey op
// ---------------------------------------------------------------------------

pub struct ExportKeyWebArg {
  /// "raw" | "spki" | "pkcs8" | "jwk".
  pub format: String,
  pub name: String,
  pub named_curve: Option<String>,
  /// `key[_algorithm].hash.name` (RSA/HMAC).
  pub hash: Option<String>,
  /// `key[_algorithm].length` (AES).
  pub length: Option<usize>,
  /// "public" | "private" | "secret".
  pub key_type: String,
  pub extractable: bool,
  /// `key.usages` (for JWK key_ops / ext).
  pub usages: Vec<String>,
}

/// Either raw bytes (raw/spki/pkcs8) or a JWK object.
pub enum ExportKeyWebResult {
  Buffer(Vec<u8>),
  Jwk(Value),
}

/// The "web" (RSA/EC/HMAC/AES/ChaCha) `exportKey` dispatch, callable directly
/// from the cppgc `SubtleCrypto::exportKey` method.
pub fn export_key_web_inner(
  mut arg: ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Result<ExportKeyWebResult, WebImportExportError> {
  arg.format = normalize_web_format(&arg.name, &arg.format)?;
  // The JS `exportKey` checks `key.extractable === false` *after* computing the
  // result (so non-extractable still validates type first). We surface the same
  // ordering: do the export, then enforce extractability.
  let result = match arg.name.as_str() {
    "HMAC" => export_hmac(&arg, key_data)?,
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => export_rsa(&arg, key_data)?,
    "ECDH" | "ECDSA" => export_ec(&arg, key_data)?,
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
      export_aes(&arg, key_data)?
    }
    "ChaCha20-Poly1305" => export_chacha(&arg, key_data)?,
    _ => return Err(not_supported("Not implemented")),
  };
  if !arg.extractable {
    return Err(WebImportExportError::InvalidAccess(
      "Key is not extractable".to_string(),
    ));
  }
  Ok(result)
}

fn export_format(format: &str) -> ExportKeyFormat {
  match format {
    "raw" => ExportKeyFormat::Raw,
    "pkcs8" => ExportKeyFormat::Pkcs8,
    "spki" => ExportKeyFormat::Spki,
    "jwkpublic" => ExportKeyFormat::JwkPublic,
    "jwkprivate" => ExportKeyFormat::JwkPrivate,
    _ => ExportKeyFormat::JwkSecret,
  }
}

fn export_alg(
  name: &str,
  curve: Option<EcNamedCurve>,
) -> Res<ExportKeyAlgorithm> {
  Ok(match name {
    "RSASSA-PKCS1-v1_5" => ExportKeyAlgorithm::RsassaPkcs1v15 {},
    "RSA-PSS" => ExportKeyAlgorithm::RsaPss {},
    "RSA-OAEP" => ExportKeyAlgorithm::RsaOaep {},
    "ECDSA" => ExportKeyAlgorithm::Ecdsa {
      named_curve: curve.ok_or_else(|| data("missing namedCurve"))?,
    },
    "ECDH" => ExportKeyAlgorithm::Ecdh {
      named_curve: curve.ok_or_else(|| data("missing namedCurve"))?,
    },
    "HMAC" => ExportKeyAlgorithm::Hmac {},
    _ => ExportKeyAlgorithm::Aes {},
  })
}

fn export_opts(
  name: &str,
  format: &str,
  curve: Option<EcNamedCurve>,
) -> Res<ExportKeyOptions> {
  Ok(ExportKeyOptions {
    format: export_format(format),
    algorithm: export_alg(name, curve)?,
  })
}

fn raw_export(res: ExportKeyResult) -> Res<ExportKeyWebResult> {
  match res {
    ExportKeyResult::Raw(b)
    | ExportKeyResult::Pkcs8(b)
    | ExportKeyResult::Spki(b) => Ok(ExportKeyWebResult::Buffer(b.to_vec())),
    _ => Err(data("invalid export")),
  }
}

// ----- HMAC ----------------------------------------------------------------

fn export_hmac(
  arg: &ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Res<ExportKeyWebResult> {
  let hash = arg.hash.as_deref().unwrap_or("");
  match arg.format.as_str() {
    "raw" => {
      let bytes = key_data.as_secret_key().map_err(ExportKeyError::from)?;
      Ok(ExportKeyWebResult::Buffer(bytes.to_vec()))
    }
    "jwk" => {
      let res = export_key_inner(
        ExportKeyOptions {
          format: ExportKeyFormat::JwkSecret,
          algorithm: ExportKeyAlgorithm::Hmac {},
        },
        key_data,
      )?;
      let k = match res {
        ExportKeyResult::JwkSecret { k } => k,
        _ => return Err(data("invalid export")),
      };
      let alg = match hash {
        "SHA-1" => "HS1",
        "SHA-256" => "HS256",
        "SHA-384" => "HS384",
        "SHA-512" => "HS512",
        "SHA3-256" => "HS3-256",
        "SHA3-384" => "HS3-384",
        "SHA3-512" => "HS3-512",
        _ => return Err(not_supported("Hash algorithm not supported")),
      };
      Ok(ExportKeyWebResult::Jwk(json!({
        "kty": "oct",
        "k": k,
        "alg": alg,
        "key_ops": arg.usages,
        "ext": arg.extractable,
      })))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

// ----- RSA -----------------------------------------------------------------

fn export_rsa(
  arg: &ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Res<ExportKeyWebResult> {
  let name = arg.name.as_str();
  match arg.format.as_str() {
    "pkcs8" => {
      if arg.key_type != "private" {
        return Err(WebImportExportError::InvalidAccess(
          "Key is not a private key".to_string(),
        ));
      }
      let res = export_key_inner(export_opts(name, "pkcs8", None)?, key_data)?;
      raw_export(res)
    }
    "spki" => {
      if arg.key_type != "public" {
        return Err(WebImportExportError::InvalidAccess(
          "Key is not a public key".to_string(),
        ));
      }
      let res = export_key_inner(export_opts(name, "spki", None)?, key_data)?;
      raw_export(res)
    }
    "jwk" => {
      let hash = arg.hash.as_deref().unwrap_or("");
      let alg = match name {
        "RSASSA-PKCS1-v1_5" => match hash {
          "SHA-1" => "RS1",
          "SHA-256" => "RS256",
          "SHA-384" => "RS384",
          "SHA-512" => "RS512",
          "SHA3-256" => "RS3-256",
          "SHA3-384" => "RS3-384",
          "SHA3-512" => "RS3-512",
          _ => return Err(not_supported("Hash algorithm not supported")),
        },
        "RSA-PSS" => match hash {
          "SHA-1" => "PS1",
          "SHA-256" => "PS256",
          "SHA-384" => "PS384",
          "SHA-512" => "PS512",
          "SHA3-256" => "PS3-256",
          "SHA3-384" => "PS3-384",
          "SHA3-512" => "PS3-512",
          _ => return Err(not_supported("Hash algorithm not supported")),
        },
        _ => match hash {
          "SHA-1" => "RSA-OAEP",
          "SHA-256" => "RSA-OAEP-256",
          "SHA-384" => "RSA-OAEP-384",
          "SHA-512" => "RSA-OAEP-512",
          "SHA3-256" => "RSA-OAEP3-256",
          "SHA3-384" => "RSA-OAEP3-384",
          "SHA3-512" => "RSA-OAEP3-512",
          _ => return Err(not_supported("Hash algorithm not supported")),
        },
      };
      let inner_format = if arg.key_type == "private" {
        "jwkprivate"
      } else {
        "jwkpublic"
      };
      let res =
        export_key_inner(export_opts(name, inner_format, None)?, key_data)?;
      let mut jwk = json!({ "kty": "RSA", "alg": alg });
      merge_rsa_jwk(&mut jwk, res)?;
      jwk["key_ops"] = json!(arg.usages);
      jwk["ext"] = json!(arg.extractable);
      Ok(ExportKeyWebResult::Jwk(jwk))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn merge_rsa_jwk(jwk: &mut Value, res: ExportKeyResult) -> Res<()> {
  let obj = jwk.as_object_mut().unwrap();
  match res {
    ExportKeyResult::JwkPublicRsa { n, e } => {
      obj.insert("n".into(), json!(n));
      obj.insert("e".into(), json!(e));
    }
    ExportKeyResult::JwkPrivateRsa {
      n,
      e,
      d,
      p,
      q,
      dp,
      dq,
      qi,
    } => {
      obj.insert("n".into(), json!(n));
      obj.insert("e".into(), json!(e));
      obj.insert("d".into(), json!(d));
      obj.insert("p".into(), json!(p));
      obj.insert("q".into(), json!(q));
      obj.insert("dp".into(), json!(dp));
      obj.insert("dq".into(), json!(dq));
      obj.insert("qi".into(), json!(qi));
    }
    _ => return Err(data("invalid export")),
  }
  Ok(())
}

// ----- EC ------------------------------------------------------------------

fn export_ec(
  arg: &ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Res<ExportKeyWebResult> {
  let name = arg.name.as_str();
  let curve_str = arg.named_curve.as_deref();
  let curve = match curve_str {
    Some(c) => Some(ec_named_curve(c)?),
    None => None,
  };
  match arg.format.as_str() {
    "raw" => {
      if arg.key_type != "public" {
        return Err(WebImportExportError::InvalidAccess(
          "Key is not a public key".to_string(),
        ));
      }
      let res = export_key_inner(export_opts(name, "raw", curve)?, key_data)?;
      raw_export(res)
    }
    "pkcs8" => {
      if arg.key_type != "private" {
        return Err(WebImportExportError::InvalidAccess(
          "Key is not a private key".to_string(),
        ));
      }
      let res = export_key_inner(export_opts(name, "pkcs8", curve)?, key_data)?;
      raw_export(res)
    }
    "spki" => {
      if arg.key_type != "public" {
        return Err(WebImportExportError::InvalidAccess(
          "Key is not a public key".to_string(),
        ));
      }
      let res = export_key_inner(export_opts(name, "spki", curve)?, key_data)?;
      raw_export(res)
    }
    "jwk" => {
      let curve_str = curve_str.ok_or_else(|| data("missing namedCurve"))?;
      let mut jwk = json!({ "kty": "EC", "crv": curve_str });
      if name == "ECDSA" {
        let alg = match curve_str {
          "P-256" => "ES256",
          "P-384" => "ES384",
          "P-521" => "ES512",
          _ => return Err(data("Curve algorithm not supported")),
        };
        jwk["alg"] = json!(alg);
      } else {
        jwk["alg"] = json!("ECDH");
      }
      let inner_format = if arg.key_type == "private" {
        "jwkprivate"
      } else {
        "jwkpublic"
      };
      let res =
        export_key_inner(export_opts(name, inner_format, curve)?, key_data)?;
      merge_ec_jwk(&mut jwk, res)?;
      jwk["key_ops"] = json!(arg.usages);
      jwk["ext"] = json!(arg.extractable);
      Ok(ExportKeyWebResult::Jwk(jwk))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn merge_ec_jwk(jwk: &mut Value, res: ExportKeyResult) -> Res<()> {
  let obj = jwk.as_object_mut().unwrap();
  match res {
    ExportKeyResult::JwkPublicEc { x, y } => {
      obj.insert("x".into(), json!(x));
      obj.insert("y".into(), json!(y));
    }
    ExportKeyResult::JwkPrivateEc { x, y, d } => {
      obj.insert("x".into(), json!(x));
      obj.insert("y".into(), json!(y));
      obj.insert("d".into(), json!(d));
    }
    _ => return Err(data("invalid export")),
  }
  Ok(())
}

// ----- AES -----------------------------------------------------------------

fn export_aes(
  arg: &ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Res<ExportKeyWebResult> {
  match arg.format.as_str() {
    "raw" => {
      let bytes = key_data.as_secret_key().map_err(ExportKeyError::from)?;
      Ok(ExportKeyWebResult::Buffer(bytes.to_vec()))
    }
    "jwk" => {
      let res = export_key_inner(
        ExportKeyOptions {
          format: ExportKeyFormat::JwkSecret,
          algorithm: ExportKeyAlgorithm::Aes {},
        },
        key_data,
      )?;
      let k = match res {
        ExportKeyResult::JwkSecret { k } => k,
        _ => return Err(data("invalid export")),
      };
      let length = arg.length.unwrap_or(0);
      let alg = aes_jwk_alg(&arg.name, length).ok_or_else(|| {
        not_supported(format!("Invalid key length: {length}"))
      })?;
      Ok(ExportKeyWebResult::Jwk(json!({
        "kty": "oct",
        "k": k,
        "alg": alg,
        "key_ops": arg.usages,
        "ext": arg.extractable,
      })))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn export_chacha(
  arg: &ExportKeyWebArg,
  key_data: V8RawKeyData,
) -> Res<ExportKeyWebResult> {
  match arg.format.as_str() {
    "raw" => {
      let bytes = key_data.as_secret_key().map_err(ExportKeyError::from)?;
      Ok(ExportKeyWebResult::Buffer(bytes.to_vec()))
    }
    "jwk" => {
      let res = export_key_inner(
        ExportKeyOptions {
          format: ExportKeyFormat::JwkSecret,
          algorithm: ExportKeyAlgorithm::Aes {},
        },
        key_data,
      )?;
      let k = match res {
        ExportKeyResult::JwkSecret { k } => k,
        _ => return Err(data("invalid export")),
      };
      Ok(ExportKeyWebResult::Jwk(json!({
        "kty": "oct",
        "k": k,
        "alg": "C20P",
        "key_ops": arg.usages,
        "ext": arg.extractable,
      })))
    }
    _ => Err(not_supported("Not implemented")),
  }
}
