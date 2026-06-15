// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.exportKey()` body in Rust.
//!
//! `WebIdlConverter` for the `KeyFormat` enum (`raw`, `raw-secret`,
//! `raw-public`, `raw-seed`, `spki`, `pkcs8`, `jwk`), the per-algorithm
//! spec validation (`extractable`, key type vs format), and dispatch into
//! the existing per-algorithm export helpers, with JWK assembly done here
//! (`kty`, `alg`, `crv`, `key_ops`, `ext` slots stamped in Rust so there
//! is no JS-side post-processing left).

use std::borrow::Cow;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::ToV8;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::export_pkcs8_ed25519;
use crate::ed25519::export_spki_ed25519;
use crate::ed25519::jwk_x_ed25519;
use crate::mldsa::mldsa_export_pkcs8;
use crate::mldsa::mldsa_export_spki;
use crate::mldsa::mldsa_from_seed;
use crate::mlkem::MlKemVariant;
use crate::mlkem::ml_kem_export_pkcs8;
use crate::mlkem::ml_kem_export_spki;
use crate::shared::ShaHash;
use crate::subtle_key::SubtleKey;
use crate::x448::export_pkcs8_x448;
use crate::x448::export_spki_x448;
use crate::x448::x448_public_key;
use crate::x25519::export_pkcs8_x25519;
use crate::x25519::export_spki_x25519;
use crate::x25519::x25519_public_key;

/// `KeyFormat` enum as used by the spec, with the modern-algos `raw-secret`
/// / `raw-public` / `raw-seed` extensions.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum KeyFormat {
  Raw,
  RawSecret,
  RawPublic,
  RawPrivate,
  RawSeed,
  Spki,
  Pkcs8,
  Jwk,
}

impl<'a> WebIdlConverter<'a> for KeyFormat {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let Some(s) = value.to_string(scope) else {
      return Err(WebIdlError::new(
        prefix,
        context,
        WebIdlErrorKind::ConvertToConverterType("KeyFormat"),
      ));
    };
    let s = s.to_rust_string_lossy(scope);
    Ok(match s.as_str() {
      "raw" => Self::Raw,
      "raw-secret" => Self::RawSecret,
      "raw-public" => Self::RawPublic,
      "raw-private" => Self::RawPrivate,
      "raw-seed" => Self::RawSeed,
      "spki" => Self::Spki,
      "pkcs8" => Self::Pkcs8,
      "jwk" => Self::Jwk,
      _ => {
        return Err(WebIdlError::other(
          prefix,
          context,
          JsErrorBox::type_error(format!(
            "The provided value '{s}' is not a valid enum value of type KeyFormat"
          )),
        ));
      }
    })
  }
}

/// Output of `SubtleCrypto.exportKey()`. The `Bytes` variant becomes a
/// `v8::ArrayBuffer` (spec mandates `ArrayBuffer` for non-jwk formats);
/// `Jwk` becomes a plain `v8::Object` with the spec-mandated slots. The
/// `Jwk` payload is `Box`ed so the discriminant doesn't carry the full
/// `JsonWebKey` size on the `Bytes` path.
pub enum ExportKeyOutput {
  Bytes(Vec<u8>),
  Jwk(Box<JsonWebKey>),
}

#[derive(Default)]
pub struct JsonWebKey {
  pub kty: &'static str,
  pub alg: Option<String>,
  pub crv: Option<String>,
  pub k: Option<String>,
  pub n: Option<String>,
  pub e: Option<String>,
  pub d: Option<String>,
  pub p: Option<String>,
  pub q: Option<String>,
  pub dp: Option<String>,
  pub dq: Option<String>,
  pub qi: Option<String>,
  pub x: Option<String>,
  pub y: Option<String>,
  pub pub_field: Option<String>,
  pub priv_field: Option<String>,
  pub key_ops: Vec<String>,
  pub ext: bool,
}

impl<'a> ToV8<'a> for ExportKeyOutput {
  type Error = JsErrorBox;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    match self {
      ExportKeyOutput::Bytes(bytes) => Ok(bytes_to_array_buffer(scope, bytes)),
      ExportKeyOutput::Jwk(jwk) => Ok(jwk_to_object(scope, *jwk).into()),
    }
  }
}

fn bytes_to_array_buffer<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: Vec<u8>,
) -> v8::Local<'s, v8::Value> {
  if bytes.is_empty() {
    return v8::ArrayBuffer::new(scope, 0).into();
  }
  let backing =
    v8::ArrayBuffer::new_backing_store_from_bytes(bytes.into_boxed_slice());
  let shared = backing.make_shared();
  v8::ArrayBuffer::with_backing_store(scope, &shared).into()
}

fn jwk_to_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  jwk: JsonWebKey,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  set_str(scope, obj, b"kty", jwk.kty);
  if let Some(v) = jwk.alg.as_deref() {
    set_str(scope, obj, b"alg", v);
  }
  if let Some(v) = jwk.crv.as_deref() {
    set_str(scope, obj, b"crv", v);
  }
  if let Some(v) = jwk.k.as_deref() {
    set_str(scope, obj, b"k", v);
  }
  if let Some(v) = jwk.n.as_deref() {
    set_str(scope, obj, b"n", v);
  }
  if let Some(v) = jwk.e.as_deref() {
    set_str(scope, obj, b"e", v);
  }
  if let Some(v) = jwk.d.as_deref() {
    set_str(scope, obj, b"d", v);
  }
  if let Some(v) = jwk.p.as_deref() {
    set_str(scope, obj, b"p", v);
  }
  if let Some(v) = jwk.q.as_deref() {
    set_str(scope, obj, b"q", v);
  }
  if let Some(v) = jwk.dp.as_deref() {
    set_str(scope, obj, b"dp", v);
  }
  if let Some(v) = jwk.dq.as_deref() {
    set_str(scope, obj, b"dq", v);
  }
  if let Some(v) = jwk.qi.as_deref() {
    set_str(scope, obj, b"qi", v);
  }
  if let Some(v) = jwk.x.as_deref() {
    set_str(scope, obj, b"x", v);
  }
  if let Some(v) = jwk.y.as_deref() {
    set_str(scope, obj, b"y", v);
  }
  if let Some(v) = jwk.pub_field.as_deref() {
    set_str(scope, obj, b"pub", v);
  }
  if let Some(v) = jwk.priv_field.as_deref() {
    set_str(scope, obj, b"priv", v);
  }
  let arr = v8::Array::new(scope, jwk.key_ops.len() as i32);
  for (i, op) in jwk.key_ops.iter().enumerate() {
    let s = v8::String::new(scope, op).unwrap();
    arr.set_index(scope, i as u32, s.into());
  }
  let key_ops_key = key_intern(scope, b"key_ops");
  obj.set(scope, key_ops_key.into(), arr.into());
  let ext_key = key_intern(scope, b"ext");
  let ext_val = v8::Boolean::new(scope, jwk.ext);
  obj.set(scope, ext_key.into(), ext_val.into());
  obj
}

fn set_str<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
  value: &str,
) {
  let key = key_intern(scope, field);
  let v = v8::String::new(scope, value).unwrap();
  obj.set(scope, key.into(), v.into());
}

fn key_intern<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: &[u8],
) -> v8::Local<'s, v8::String> {
  v8::String::new_from_one_byte(scope, bytes, v8::NewStringType::Internalized)
    .unwrap()
}

fn b64_url(bytes: &[u8]) -> String {
  BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

fn invalid_access(msg: &'static str) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionInvalidAccessError", msg))
}

fn not_supported(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionNotSupportedError", msg))
}

fn op_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionOperationError", msg))
}

/// Main dispatch — called from the cppgc impl block on `SubtleCrypto` after
/// the `WebIdlConverter`s have produced the `KeyFormat` + `SubtleKey`.
pub fn run(
  format: KeyFormat,
  key: SubtleKey,
) -> Result<ExportKeyOutput, CryptoError> {
  // Spec: `exportKey` MUST throw `InvalidAccessError` before any
  // per-algorithm export work runs when the key is non-extractable.
  // Hoisting this guard ahead of the dispatch prevents a non-extractable
  // key whose export would also fail (unsupported algorithm or format)
  // from masking the `InvalidAccessError` with `NotSupportedError` /
  // `OperationError`.
  if !key.extractable {
    return Err(invalid_access("Key is not extractable"));
  }
  match key.algorithm_name.as_str() {
    "HMAC" => export_symmetric(format, &key, SymKind::Hmac),
    "KMAC128" | "KMAC256" => export_symmetric(format, &key, SymKind::Kmac),
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
      export_symmetric(format, &key, SymKind::Aes)
    }
    "ChaCha20-Poly1305" => export_symmetric(format, &key, SymKind::ChaCha),
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => export_rsa(format, &key),
    "ECDSA" | "ECDH" => export_ec(format, &key),
    "Ed25519" => export_okp(format, &key, OkpKind::Ed25519),
    "X25519" => export_okp(format, &key, OkpKind::X25519),
    "X448" => export_okp(format, &key, OkpKind::X448),
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      export_akp_mlkem(format, &key)
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => export_akp_mldsa(format, &key),
    other => Err(not_supported(format!(
      "Unrecognized algorithm name: {other}"
    ))),
  }
}

#[derive(Copy, Clone)]
enum SymKind {
  Aes,
  Hmac,
  Kmac,
  ChaCha,
}

fn export_symmetric(
  format: KeyFormat,
  key: &SubtleKey,
  kind: SymKind,
) -> Result<ExportKeyOutput, CryptoError> {
  let raw_alias_allowed = !matches!(kind, SymKind::ChaCha);
  match format {
    KeyFormat::RawSecret => {
      Ok(ExportKeyOutput::Bytes(key.raw.bytes().to_vec()))
    }
    KeyFormat::Raw if raw_alias_allowed => {
      Ok(ExportKeyOutput::Bytes(key.raw.bytes().to_vec()))
    }
    KeyFormat::Jwk => {
      let mut jwk = JsonWebKey {
        kty: "oct",
        ext: key.extractable,
        key_ops: key.usages.clone(),
        ..Default::default()
      };
      jwk.k = Some(b64_url(key.raw.bytes()));
      jwk.alg = Some(jwk_alg_for_symmetric(kind, key)?);
      Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
    }
    _ => Err(not_supported("Not implemented".to_string())),
  }
}

fn jwk_alg_for_symmetric(
  kind: SymKind,
  key: &SubtleKey,
) -> Result<String, CryptoError> {
  match kind {
    SymKind::Aes => {
      let len = key.algorithm_length.unwrap_or(0);
      if !matches!(len, 128 | 192 | 256) {
        return Err(not_supported(format!("Invalid key length: {len}")));
      }
      let suffix = match key.algorithm_name.as_str() {
        "AES-CTR" => "CTR",
        "AES-CBC" => "CBC",
        "AES-GCM" => "GCM",
        "AES-KW" => "KW",
        "AES-OCB" => "OCB",
        other => {
          return Err(not_supported(format!(
            "Unsupported AES variant: {other}"
          )));
        }
      };
      Ok(format!("A{len}{suffix}"))
    }
    SymKind::ChaCha => Ok("C20P".to_string()),
    SymKind::Hmac => {
      let hash = key.algorithm_hash.ok_or_else(|| {
        not_supported("Hash algorithm not supported".to_string())
      })?;
      Ok(
        match hash {
          ShaHash::Sha1 => "HS1",
          ShaHash::Sha256 => "HS256",
          ShaHash::Sha384 => "HS384",
          ShaHash::Sha512 => "HS512",
          ShaHash::Sha3_256 => "HS3-256",
          ShaHash::Sha3_384 => "HS3-384",
          ShaHash::Sha3_512 => "HS3-512",
        }
        .to_string(),
      )
    }
    SymKind::Kmac => match key.algorithm_name.as_str() {
      "KMAC128" => Ok("K128".to_string()),
      "KMAC256" => Ok("K256".to_string()),
      other => Err(not_supported(format!("Unsupported KMAC variant: {other}"))),
    },
  }
}

fn export_rsa(
  format: KeyFormat,
  key: &SubtleKey,
) -> Result<ExportKeyOutput, CryptoError> {
  use crate::export_key::ExportKeyFormat as F;
  use crate::export_key::ExportKeyResult;
  let raw = &key.raw;
  match format {
    KeyFormat::Pkcs8 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key is not a private key"));
      }
      let res = crate::export_key::export_key_rsa_for_subtle(F::Pkcs8, raw)
        .map_err(|e| op_error(e.to_string()))?;
      match res {
        ExportKeyResult::Pkcs8(b) => Ok(ExportKeyOutput::Bytes(b.0.into())),
        _ => Err(op_error("unexpected".into())),
      }
    }
    KeyFormat::Spki => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key is not a public key"));
      }
      let res = crate::export_key::export_key_rsa_for_subtle(F::Spki, raw)
        .map_err(|e| op_error(e.to_string()))?;
      match res {
        ExportKeyResult::Spki(b) => Ok(ExportKeyOutput::Bytes(b.0.into())),
        _ => Err(op_error("unexpected".into())),
      }
    }
    KeyFormat::Jwk => {
      let inner_format = match key.key_type {
        CryptoKeyType::Public => F::JwkPublic,
        CryptoKeyType::Private => F::JwkPrivate,
        _ => return Err(not_supported("Not implemented".to_string())),
      };
      let res = crate::export_key::export_key_rsa_for_subtle(inner_format, raw)
        .map_err(|e| op_error(e.to_string()))?;
      let alg = rsa_jwk_alg(key)?;
      let mut jwk = JsonWebKey {
        kty: "RSA",
        ext: key.extractable,
        key_ops: key.usages.clone(),
        alg: Some(alg),
        ..Default::default()
      };
      match res {
        ExportKeyResult::JwkPublicRsa { n, e } => {
          jwk.n = Some(n);
          jwk.e = Some(e);
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
          jwk.n = Some(n);
          jwk.e = Some(e);
          jwk.d = Some(d);
          jwk.p = Some(p);
          jwk.q = Some(q);
          jwk.dp = Some(dp);
          jwk.dq = Some(dq);
          jwk.qi = Some(qi);
        }
        _ => return Err(op_error("unexpected".into())),
      }
      Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
    }
    _ => Err(not_supported("Not implemented".to_string())),
  }
}

fn rsa_jwk_alg(key: &SubtleKey) -> Result<String, CryptoError> {
  let hash = key
    .algorithm_hash
    .ok_or_else(|| not_supported("Hash algorithm not supported".to_string()))?;
  Ok(
    match key.algorithm_name.as_str() {
      "RSASSA-PKCS1-v1_5" => match hash {
        ShaHash::Sha1 => "RS1",
        ShaHash::Sha256 => "RS256",
        ShaHash::Sha384 => "RS384",
        ShaHash::Sha512 => "RS512",
        ShaHash::Sha3_256 => "RS3-256",
        ShaHash::Sha3_384 => "RS3-384",
        ShaHash::Sha3_512 => "RS3-512",
      },
      "RSA-PSS" => match hash {
        ShaHash::Sha1 => "PS1",
        ShaHash::Sha256 => "PS256",
        ShaHash::Sha384 => "PS384",
        ShaHash::Sha512 => "PS512",
        ShaHash::Sha3_256 => "PS3-256",
        ShaHash::Sha3_384 => "PS3-384",
        ShaHash::Sha3_512 => "PS3-512",
      },
      "RSA-OAEP" => match hash {
        ShaHash::Sha1 => "RSA-OAEP",
        ShaHash::Sha256 => "RSA-OAEP-256",
        ShaHash::Sha384 => "RSA-OAEP-384",
        ShaHash::Sha512 => "RSA-OAEP-512",
        ShaHash::Sha3_256 => "RSA-OAEP3-256",
        ShaHash::Sha3_384 => "RSA-OAEP3-384",
        ShaHash::Sha3_512 => "RSA-OAEP3-512",
      },
      other => {
        return Err(not_supported(format!("Unsupported RSA variant: {other}")));
      }
    }
    .to_string(),
  )
}

fn ec_named_curve(name: &str) -> Option<crate::shared::EcNamedCurve> {
  match name {
    "P-256" => Some(crate::shared::EcNamedCurve::P256),
    "P-384" => Some(crate::shared::EcNamedCurve::P384),
    "P-521" => Some(crate::shared::EcNamedCurve::P521),
    _ => None,
  }
}

fn export_ec(
  format: KeyFormat,
  key: &SubtleKey,
) -> Result<ExportKeyOutput, CryptoError> {
  use crate::export_key::ExportKeyAlgorithm;
  use crate::export_key::ExportKeyFormat as F;
  use crate::export_key::ExportKeyResult;
  let curve_name = key
    .algorithm_named_curve
    .as_deref()
    .ok_or_else(|| not_supported("Missing namedCurve".to_string()))?;
  let curve = ec_named_curve(curve_name)
    .ok_or_else(|| not_supported(format!("Unsupported curve: {curve_name}")))?;
  let algorithm = match key.algorithm_name.as_str() {
    "ECDSA" => ExportKeyAlgorithm::Ecdsa { named_curve: curve },
    "ECDH" => ExportKeyAlgorithm::Ecdh { named_curve: curve },
    other => return Err(not_supported(format!("Unsupported EC: {other}"))),
  };

  let (inner_format, want_jwk) = match (format, key.key_type) {
    (KeyFormat::Raw | KeyFormat::RawPublic, CryptoKeyType::Public) => {
      (F::Raw, false)
    }
    (KeyFormat::Raw | KeyFormat::RawPublic, _) => {
      return Err(invalid_access("Key is not a public key"));
    }
    (KeyFormat::Pkcs8, CryptoKeyType::Private) => (F::Pkcs8, false),
    (KeyFormat::Pkcs8, _) => {
      return Err(invalid_access("Key is not a private key"));
    }
    (KeyFormat::Spki, CryptoKeyType::Public) => (F::Spki, false),
    (KeyFormat::Spki, _) => {
      return Err(invalid_access("Key is not a public key"));
    }
    (KeyFormat::Jwk, CryptoKeyType::Private) => (F::JwkPrivate, true),
    (KeyFormat::Jwk, CryptoKeyType::Public) => (F::JwkPublic, true),
    (KeyFormat::Jwk, _) => {
      return Err(not_supported("Not implemented".to_string()));
    }
    _ => return Err(not_supported("Not implemented".to_string())),
  };
  let res = crate::export_key::export_key_ec_for_subtle(
    inner_format,
    &key.raw,
    algorithm,
    curve,
  )
  .map_err(|e| op_error(e.to_string()))?;
  if !want_jwk {
    return match res {
      ExportKeyResult::Raw(b)
      | ExportKeyResult::Spki(b)
      | ExportKeyResult::Pkcs8(b) => Ok(ExportKeyOutput::Bytes(b.0.into())),
      _ => Err(op_error("unexpected".into())),
    };
  }
  let mut jwk = JsonWebKey {
    kty: "EC",
    ext: key.extractable,
    key_ops: key.usages.clone(),
    crv: Some(curve_name.to_string()),
    ..Default::default()
  };
  jwk.alg = Some(if key.algorithm_name == "ECDSA" {
    match curve {
      crate::shared::EcNamedCurve::P256 => "ES256",
      crate::shared::EcNamedCurve::P384 => "ES384",
      crate::shared::EcNamedCurve::P521 => "ES512",
    }
    .to_string()
  } else {
    "ECDH".to_string()
  });
  match res {
    ExportKeyResult::JwkPublicEc { x, y } => {
      jwk.x = Some(x);
      jwk.y = Some(y);
    }
    ExportKeyResult::JwkPrivateEc { x, y, d } => {
      jwk.x = Some(x);
      jwk.y = Some(y);
      jwk.d = Some(d);
    }
    _ => return Err(op_error("unexpected".into())),
  }
  Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
}

#[derive(Copy, Clone)]
enum OkpKind {
  Ed25519,
  X25519,
  X448,
}

fn okp_crv(kind: OkpKind) -> &'static str {
  match kind {
    OkpKind::Ed25519 => "Ed25519",
    OkpKind::X25519 => "X25519",
    OkpKind::X448 => "X448",
  }
}

fn export_okp(
  format: KeyFormat,
  key: &SubtleKey,
  kind: OkpKind,
) -> Result<ExportKeyOutput, CryptoError> {
  let raw = key.raw.bytes();
  match format {
    KeyFormat::Raw | KeyFormat::RawPublic => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key is not a public key"));
      }
      Ok(ExportKeyOutput::Bytes(raw.to_vec()))
    }
    KeyFormat::Spki => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key is not a public key"));
      }
      let bytes: Vec<u8> = match kind {
        OkpKind::Ed25519 => {
          export_spki_ed25519(raw).map_err(|e| op_error(e.to_string()))?
        }
        OkpKind::X25519 => {
          export_spki_x25519(raw).map_err(|e| op_error(e.to_string()))?
        }
        OkpKind::X448 => {
          export_spki_x448(raw).map_err(|e| op_error(e.to_string()))?
        }
      };
      Ok(ExportKeyOutput::Bytes(bytes))
    }
    KeyFormat::Pkcs8 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key is not a private key"));
      }
      let (prefix, len_byte): (&[u8], u8) = match kind {
        OkpKind::Ed25519 => (&[0x04, 0x22], 0x20),
        OkpKind::X25519 => (&[0x04, 0x22], 0x20),
        OkpKind::X448 => (&[0x04, 0x3a], 0x38),
      };
      let mut combined = Vec::with_capacity(prefix.len() + raw.len());
      combined.extend_from_slice(prefix);
      combined.extend_from_slice(raw);
      let mut pkcs8: Vec<u8> = match kind {
        OkpKind::Ed25519 => export_pkcs8_ed25519(&combined)
          .map_err(|e| op_error(e.to_string()))?,
        OkpKind::X25519 => {
          export_pkcs8_x25519(&combined).map_err(|e| op_error(e.to_string()))?
        }
        OkpKind::X448 => {
          export_pkcs8_x448(&combined).map_err(|e| op_error(e.to_string()))?
        }
      };
      if pkcs8.len() > 15 {
        pkcs8[15] = len_byte;
      }
      Ok(ExportKeyOutput::Bytes(pkcs8))
    }
    KeyFormat::Jwk => {
      let mut jwk = JsonWebKey {
        kty: "OKP",
        ext: key.extractable,
        key_ops: key.usages.clone(),
        crv: Some(okp_crv(kind).to_string()),
        ..Default::default()
      };
      if key.key_type == CryptoKeyType::Private {
        let x = match kind {
          OkpKind::Ed25519 => {
            jwk_x_ed25519(raw).map_err(|e| op_error(e.to_string()))?
          }
          OkpKind::X25519 => x25519_public_key(raw),
          OkpKind::X448 => {
            x448_public_key(raw).map_err(|e| op_error(e.to_string()))?
          }
        };
        jwk.x = Some(x);
        jwk.d = Some(b64_url(raw));
      } else {
        jwk.x = Some(b64_url(raw));
      }
      Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
    }
    _ => Err(not_supported("Not implemented".to_string())),
  }
}

fn mlkem_variant(name: &str) -> Option<MlKemVariant> {
  match name {
    "ML-KEM-512" => Some(MlKemVariant::MlKem512),
    "ML-KEM-768" => Some(MlKemVariant::MlKem768),
    "ML-KEM-1024" => Some(MlKemVariant::MlKem1024),
    _ => None,
  }
}

fn mldsa_variant_id(name: &str) -> Option<u8> {
  match name {
    "ML-DSA-44" => Some(0),
    "ML-DSA-65" => Some(1),
    "ML-DSA-87" => Some(2),
    _ => None,
  }
}

fn export_akp_mlkem(
  format: KeyFormat,
  key: &SubtleKey,
) -> Result<ExportKeyOutput, CryptoError> {
  let variant = mlkem_variant(&key.algorithm_name)
    .ok_or_else(|| op_error("Unknown ML-KEM variant".into()))?;
  match format {
    KeyFormat::RawPublic => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access(
          "'raw-public' is only valid for public keys",
        ));
      }
      Ok(ExportKeyOutput::Bytes(key.raw.bytes().to_vec()))
    }
    KeyFormat::RawSeed => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access(
          "'raw-seed' is only valid for private keys",
        ));
      }
      let seed = key
        .raw
        .seed()
        .ok_or_else(|| op_error("Seed is not available for this key".into()))?;
      Ok(ExportKeyOutput::Bytes(seed.to_vec()))
    }
    KeyFormat::Spki => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("'spki' is only valid for public keys"));
      }
      let bytes: Vec<u8> = ml_kem_export_spki(variant, key.raw.bytes())
        .map_err(|e| op_error(e.to_string()))?;
      Ok(ExportKeyOutput::Bytes(bytes))
    }
    KeyFormat::Pkcs8 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("'pkcs8' is only valid for private keys"));
      }
      let seed = key.raw.seed().ok_or_else(|| {
        op_error(
          "PKCS#8 export requires the original seed; this key was imported without one"
            .into(),
        )
      })?;
      let bytes: Vec<u8> = ml_kem_export_pkcs8(variant, seed)
        .map_err(|e| op_error(e.to_string()))?;
      Ok(ExportKeyOutput::Bytes(bytes))
    }
    KeyFormat::Jwk => {
      let mut jwk = JsonWebKey {
        kty: "AKP",
        alg: Some(key.algorithm_name.clone()),
        ext: key.extractable,
        key_ops: key.usages.clone(),
        ..Default::default()
      };
      if key.key_type == CryptoKeyType::Private {
        let seed = key.raw.seed().ok_or_else(|| {
          op_error(
            "JWK export requires the original seed; this key was imported without one"
              .into(),
          )
        })?;
        let pub_bytes = variant
          .public_from_expanded_for_subtle(key.raw.expanded_private_key())
          .map_err(|e| op_error(e.to_string()))?;
        jwk.pub_field = Some(b64_url(&pub_bytes));
        jwk.priv_field = Some(b64_url(seed));
      } else {
        jwk.pub_field = Some(b64_url(key.raw.bytes()));
      }
      Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
    }
    _ => Err(not_supported(format!(
      "Unsupported key format for ML-KEM: {format:?}"
    ))),
  }
}

fn export_akp_mldsa(
  format: KeyFormat,
  key: &SubtleKey,
) -> Result<ExportKeyOutput, CryptoError> {
  let variant = mldsa_variant_id(&key.algorithm_name)
    .ok_or_else(|| op_error("Unknown ML-DSA variant".into()))?;
  match format {
    KeyFormat::RawPublic => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access(
          "'raw-public' is only valid for public keys",
        ));
      }
      Ok(ExportKeyOutput::Bytes(key.raw.bytes().to_vec()))
    }
    KeyFormat::RawSeed => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access(
          "'raw-seed' is only valid for private keys",
        ));
      }
      let seed = key
        .raw
        .seed()
        .ok_or_else(|| op_error("Seed is not available for this key".into()))?;
      Ok(ExportKeyOutput::Bytes(seed.to_vec()))
    }
    KeyFormat::Spki => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("'spki' is only valid for public keys"));
      }
      let bytes: Vec<u8> = mldsa_export_spki(variant, key.raw.bytes())
        .map_err(|e| op_error(e.to_string()))?;
      Ok(ExportKeyOutput::Bytes(bytes))
    }
    KeyFormat::Pkcs8 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("'pkcs8' is only valid for private keys"));
      }
      let seed = key.raw.seed().ok_or_else(|| {
        op_error(
          "PKCS#8 export requires the original seed; this key was imported without one"
            .into(),
        )
      })?;
      let bytes: Vec<u8> = mldsa_export_pkcs8(variant, seed)
        .map_err(|e| op_error(e.to_string()))?;
      Ok(ExportKeyOutput::Bytes(bytes))
    }
    KeyFormat::Jwk => {
      let mut jwk = JsonWebKey {
        kty: "AKP",
        alg: Some(key.algorithm_name.clone()),
        ext: key.extractable,
        key_ops: key.usages.clone(),
        ..Default::default()
      };
      if key.key_type == CryptoKeyType::Private {
        let seed = key.raw.seed().ok_or_else(|| {
          op_error(
            "JWK export requires the original seed; this key was imported without one"
              .into(),
          )
        })?;
        // Re-derive the public key from the seed via the existing op
        // (returns both private and public expanded).
        let (_, pub_bytes) = mldsa_from_seed(variant, seed)
          .map_err(|e| op_error(e.to_string()))?;
        jwk.pub_field = Some(b64_url(&pub_bytes));
        jwk.priv_field = Some(b64_url(seed));
      } else {
        jwk.pub_field = Some(b64_url(key.raw.bytes()));
      }
      Ok(ExportKeyOutput::Jwk(Box::new(jwk)))
    }
    _ => Err(not_supported(format!(
      "Unsupported key format for ML-DSA: {format:?}"
    ))),
  }
}
