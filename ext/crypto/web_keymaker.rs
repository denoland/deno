// Copyright 2018-2026 the Deno authors. MIT license.

//! The `CryptoKey`-producing/consuming `SubtleCrypto` methods, ported from the
//! JS `importKeyXXX` / `exportKeyXXX` / `generateKey` helpers in
//! `ext/crypto/00_crypto.js`.
//!
//! These build (or read) cppgc `CryptoKey` objects. Because a cppgc object can
//! only be constructed when a `v8` scope is available, the work happens in
//! synchronous `#[op2] impl SubtleCrypto` methods (see `web_subtle.rs`), exactly
//! like webgpu's `createBuffer` returns a `#[cppgc] GPUBuffer`. The per-spec
//! Promise wrapping (importKey/generateKey/etc. return Promises) is provided by
//! thin `async`/Promise wrappers in `00_crypto.js`.
//!
//! The algorithm-specific crypto is reused: the "web" algorithms
//! (RSA/EC/HMAC/AES/AES-KW/ChaCha20-Poly1305/HKDF/PBKDF2) go through
//! `web_import_export::{import_key_web_inner, export_key_web_inner}`, and the
//! per-curve / post-quantum algorithms (Ed25519/X25519/X448/ML-KEM/ML-DSA) call
//! the `pub fn` cores exposed by `ed25519.rs` / `x25519.rs` / `x448.rs` /
//! `mlkem.rs` / `mldsa.rs`.

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::FromV8;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::v8;
use deno_error::JsErrorBox;

use crate::mlkem::MlKemVariant;
use crate::web_cryptokey as ck;
use crate::web_cryptokey::CryptoKey;
use crate::web_import_export::AlgorithmDict;
use crate::web_import_export::ExportKeyWebArg;
use crate::web_import_export::ExportKeyWebResult;
use crate::web_import_export::ImportKeyWebArg;
use crate::web_import_export::ImportKeyWebData;
use crate::web_import_export::ImportKeyWebResult;
use crate::web_import_export::WebImportExportError;
use crate::web_keyutil as ku;

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

const ML_KEM_PRIVATE_USAGES: &[&str] = &["decapsulateKey", "decapsulateBits"];
const ML_KEM_PUBLIC_USAGES: &[&str] = &["encapsulateKey", "encapsulateBits"];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors for the keymaker methods. The class strings used here all have a
/// registered `DOMException` builder in `runtime/js/99_main.js`.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum KeyMakerError {
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
  #[class("DOMExceptionOperationError")]
  #[error("{0}")]
  Operation(String),
  #[class(type)]
  #[error("{0}")]
  Type(String),
  #[class(inherit)]
  #[error(transparent)]
  WebImportExport(
    #[from]
    #[inherit]
    WebImportExportError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  MlKem(
    #[from]
    #[inherit]
    crate::mlkem::MlKemError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  MlDsa(
    #[from]
    #[inherit]
    crate::mldsa::MlDsaError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Ed25519(
    #[from]
    #[inherit]
    crate::ed25519::Ed25519Error,
  ),
  #[class(inherit)]
  #[error(transparent)]
  X25519(
    #[from]
    #[inherit]
    crate::x25519::X25519Error,
  ),
  #[class(inherit)]
  #[error(transparent)]
  X448(
    #[from]
    #[inherit]
    crate::x448::X448Error,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Other(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
}

type Res<T> = Result<T, KeyMakerError>;

fn data(msg: impl Into<String>) -> KeyMakerError {
  KeyMakerError::Data(msg.into())
}
fn syntax(msg: impl Into<String>) -> KeyMakerError {
  KeyMakerError::Syntax(msg.into())
}
fn not_supported(msg: impl Into<String>) -> KeyMakerError {
  KeyMakerError::NotSupported(msg.into())
}
fn invalid_access(msg: impl Into<String>) -> KeyMakerError {
  KeyMakerError::InvalidAccess(msg.into())
}

fn usage_intersection(a: &[String], b: &[&str]) -> Vec<String> {
  a.iter()
    .filter(|u| b.contains(&u.as_str()))
    .cloned()
    .collect()
}

fn has_disallowed_usage(usages: &[String], allowed: &[&str]) -> bool {
  usages.iter().any(|u| !allowed.contains(&u.as_str()))
}

fn mldsa_variant_id(name: &str) -> Res<u8> {
  match name {
    "ML-DSA-44" => Ok(0),
    "ML-DSA-65" => Ok(1),
    "ML-DSA-87" => Ok(2),
    _ => Err(KeyMakerError::Type(format!(
      "Unknown ML-DSA variant: {name}"
    ))),
  }
}

fn mldsa_public_key_len(variant: u8) -> usize {
  match variant {
    0 => 1312,
    1 => 1952,
    _ => 2592,
  }
}

// ---------------------------------------------------------------------------
// Key material description + cppgc construction
// ---------------------------------------------------------------------------

/// The algorithm dictionary to attach to a `CryptoKey`. Built in Rust then
/// materialized into a v8 object in [`build_alg`].
pub enum AlgDesc {
  /// `{ name }`.
  Name(String),
  /// `{ name, hash: { name }, length? }` (HMAC).
  Hmac { hash: String, length: usize },
  /// `{ name, length }` (AES).
  AesLength { name: String, length: usize },
  /// `{ name, namedCurve }` (EC).
  Ec { name: String, named_curve: String },
  /// `{ name, modulusLength, publicExponent, hash: { name } }` (RSA).
  Rsa {
    name: String,
    modulus_length: usize,
    public_exponent: Vec<u8>,
    hash: String,
  },
}

/// The raw `key_data` value to attach to a `CryptoKey`.
pub enum KeyDataDesc {
  /// `{ type, data }` (AES/HMAC/RSA/EC/ChaCha/HKDF/PBKDF2).
  TypeData { data_type: String, data: Vec<u8> },
  /// Raw bytes (Ed25519/X25519/X448/ML-KEM public+private).
  Raw(Vec<u8>),
  /// `{ seed, privateKey }` (ML-DSA private key).
  MlDsaPrivate {
    seed: Option<Vec<u8>>,
    private_key: Vec<u8>,
  },
}

/// A fully-described `CryptoKey` ready to be materialized into a cppgc object.
pub struct KeyDesc {
  pub key_type: String,
  pub extractable: bool,
  pub usages: Vec<String>,
  pub algorithm: AlgDesc,
  pub key_data: KeyDataDesc,
  /// For ML-DSA private keys: the public key counterpart, materialized as a
  /// public CryptoKey and stored in the `MLDSA_PUBLIC_FROM_PRIVATE` map by JS.
  pub mldsa_public: Option<Box<KeyDesc>>,
}

impl KeyDesc {
  fn new(
    key_type: &str,
    extractable: bool,
    usages: Vec<String>,
    algorithm: AlgDesc,
    key_data: KeyDataDesc,
  ) -> Self {
    KeyDesc {
      key_type: key_type.to_string(),
      extractable,
      usages,
      algorithm,
      key_data,
      mldsa_public: None,
    }
  }
}

/// The result of `importKey`: a single key, or (for generateKey) a key pair.
pub enum KeyOrPair {
  Single(KeyDesc),
  Pair {
    public_key: KeyDesc,
    private_key: KeyDesc,
  },
}

fn build_alg<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  desc: &AlgDesc,
) -> v8::Local<'a, v8::Object> {
  match desc {
    AlgDesc::Name(name) => ck::alg_object(scope, name),
    AlgDesc::Hmac { hash, length } => {
      ck::alg_object_hash(scope, "HMAC", hash, Some(*length))
    }
    AlgDesc::AesLength { name, length } => {
      let obj = ck::alg_object(scope, name);
      ck::set_num(scope, obj, "length", *length as f64);
      obj
    }
    AlgDesc::Ec { name, named_curve } => {
      let obj = ck::alg_object(scope, name);
      ck::set_str(scope, obj, "namedCurve", named_curve);
      obj
    }
    AlgDesc::Rsa {
      name,
      modulus_length,
      public_exponent,
      hash,
    } => {
      let obj = ck::alg_object(scope, name);
      ck::set_num(scope, obj, "modulusLength", *modulus_length as f64);
      let pe = ck::u8_array(scope, public_exponent);
      ck::set_val(scope, obj, "publicExponent", pe);
      let hash_obj = v8::Object::new(scope);
      ck::set_str(scope, hash_obj, "name", hash);
      ck::set_val(scope, obj, "hash", hash_obj.into());
      obj
    }
  }
}

fn build_key_data<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  desc: &KeyDataDesc,
) -> v8::Local<'a, v8::Value> {
  match desc {
    KeyDataDesc::TypeData { data_type, data } => {
      ck::type_data_value(scope, data_type, data)
    }
    KeyDataDesc::Raw(bytes) => ck::u8_array(scope, bytes),
    KeyDataDesc::MlDsaPrivate { seed, private_key } => {
      let obj = v8::Object::new(scope);
      match seed {
        Some(s) => {
          let arr = ck::u8_array(scope, s);
          ck::set_val(scope, obj, "seed", arr);
        }
        None => {
          let null = v8::null(scope);
          ck::set_val(scope, obj, "seed", null.into());
        }
      }
      let pk = ck::u8_array(scope, private_key);
      ck::set_val(scope, obj, "privateKey", pk);
      obj.into()
    }
  }
}

/// Materialize a [`KeyDesc`] into a cppgc `CryptoKey` v8 object.
pub fn build_one<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  desc: KeyDesc,
) -> v8::Local<'a, v8::Object> {
  // ML-DSA: build the associated public key first so it can be stored on the
  // private key (for `getPublicKey()`).
  let mldsa_public =
    desc.mldsa_public.map(|pubdesc| build_one(scope, *pubdesc));
  let alg = build_alg(scope, &desc.algorithm);
  let key_data = build_key_data(scope, &desc.key_data);
  ck::build_crypto_key(
    scope,
    &desc.key_type,
    desc.extractable,
    desc.usages,
    alg,
    key_data,
    mldsa_public,
  )
}

/// Materialize a [`KeyOrPair`] into the value `generateKey`/`importKey` return.
/// A single key returns the CryptoKey directly; a pair returns
/// `{ publicKey, privateKey }`.
pub fn build_result<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  result: KeyOrPair,
) -> v8::Local<'a, v8::Value> {
  match result {
    KeyOrPair::Single(desc) => build_one(scope, desc).into(),
    KeyOrPair::Pair {
      public_key,
      private_key,
    } => {
      let pubk = build_one(scope, public_key);
      let privk = build_one(scope, private_key);
      let obj = v8::Object::new(scope);
      ck::set_val(scope, obj, "publicKey", pubk.into());
      ck::set_val(scope, obj, "privateKey", privk.into());
      obj.into()
    }
  }
}

// ---------------------------------------------------------------------------
// importKey
// ---------------------------------------------------------------------------

/// The normalized algorithm + format/usages/data for `importKey`, extracted in
/// the sync op2 prelude (FromV8). Mirrors the JS `importKey` after webidl
/// conversion + `normalizeAlgorithm(algorithm, "importKey")`.
pub struct ImportKeyParams {
  pub format: String,
  pub name: String,
  pub hash: Option<String>,
  pub named_curve: Option<String>,
  pub length: Option<usize>,
  pub extractable: bool,
  pub key_usages: Vec<String>,
  /// Buffer (raw/spki/pkcs8) key material.
  pub buffer: Option<deno_core::JsBuffer>,
  /// JWK key material.
  pub jwk: Option<Value>,
}

impl ImportKeyParams {
  fn web_arg(&self) -> ImportKeyWebArg {
    ImportKeyWebArg {
      format: self.format.clone(),
      name: self.name.clone(),
      hash: self.hash.clone(),
      named_curve: self.named_curve.clone(),
      length: self.length,
      extractable: self.extractable,
      key_usages: self.key_usages.clone(),
    }
  }
}

/// Perform an `importKey`, returning a [`KeyDesc`] (no scope needed). The cppgc
/// method then materializes the key.
pub fn import_key_compute(p: &ImportKeyParams) -> Res<KeyDesc> {
  match p.name.as_str() {
    "HMAC" | "ECDH" | "ECDSA" | "RSASSA-PKCS1-v1_5" | "RSA-PSS"
    | "RSA-OAEP" | "HKDF" | "PBKDF2" | "AES-CTR" | "AES-CBC" | "AES-GCM"
    | "AES-OCB" | "AES-KW" | "ChaCha20-Poly1305" => import_web(p),
    "X448" => import_x448(p),
    "X25519" => import_x25519(p),
    "Ed25519" => import_ed25519(p),
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => import_mlkem(p),
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => import_mldsa(p),
    _ => Err(not_supported("Not implemented")),
  }
}

fn import_web(p: &ImportKeyParams) -> Res<KeyDesc> {
  let key_data = match (&p.buffer, &p.jwk) {
    (Some(b), _) => ImportKeyWebData::Buffer(b.clone()),
    (None, Some(v)) => ImportKeyWebData::Jwk(v.clone()),
    (None, None) => return Err(KeyMakerError::Type("Missing key data".into())),
  };
  let ImportKeyWebResult {
    key_type,
    usages,
    algorithm,
    data_type,
    data,
  } = crate::web_import_export::import_key_web_inner(p.web_arg(), key_data)?;
  let is_kdf = algorithm.name == "HKDF" || algorithm.name == "PBKDF2";
  Ok(KeyDesc::new(
    &key_type,
    if is_kdf { false } else { p.extractable },
    usages,
    alg_from_dict(algorithm),
    KeyDataDesc::TypeData { data_type, data },
  ))
}

fn alg_from_dict(d: AlgorithmDict) -> AlgDesc {
  match (d.modulus_length, d.public_exponent, d.named_curve, d.hash) {
    (Some(ml), Some(pe), _, Some(hash)) => AlgDesc::Rsa {
      name: d.name,
      modulus_length: ml,
      public_exponent: pe,
      hash,
    },
    (_, _, Some(curve), _) => AlgDesc::Ec {
      name: d.name,
      named_curve: curve,
    },
    (_, _, _, Some(hash)) if d.name == "HMAC" => AlgDesc::Hmac {
      hash,
      length: d.length.unwrap_or(0),
    },
    _ => match d.length {
      Some(l) if d.name.starts_with("AES") => AlgDesc::AesLength {
        name: d.name,
        length: l,
      },
      _ => AlgDesc::Name(d.name),
    },
  }
}

fn buffer(p: &ImportKeyParams) -> Res<&[u8]> {
  p.buffer
    .as_deref()
    .ok_or_else(|| KeyMakerError::Type("Expected a BufferSource".into()))
}

fn jwk(p: &ImportKeyParams) -> Res<&Value> {
  p.jwk
    .as_ref()
    .ok_or_else(|| KeyMakerError::Type("Expected a JsonWebKey".into()))
}

fn jwk_str<'a>(jwk: &'a Value, key: &str) -> Option<&'a str> {
  jwk.get(key).and_then(|v| v.as_str())
}
fn jwk_has(jwk: &Value, key: &str) -> bool {
  jwk.get(key).is_some()
}

fn validate_key_ops(jwk: &Value, key_usages: &[String]) -> Res<()> {
  if let Some(key_ops) = jwk.get("key_ops") {
    let ops: Vec<String> = match key_ops.as_array() {
      Some(arr) => arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect(),
      None => {
        return Err(data("'key_ops' property of JsonWebKey is invalid"));
      }
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
    return Err(data("Invalid key extractability"));
  }
  Ok(())
}

fn b64url_decode(s: &str) -> Res<Vec<u8>> {
  BASE64_URL_SAFE_NO_PAD
    .decode(s)
    .map_err(|_| data("Invalid key data"))
}

// ----- Ed25519 / X25519 / X448 ---------------------------------------------

/// Shared OKP (Ed25519/X25519/X448) JWK validation. `crv` is the expected
/// curve, `sign_use` the JWK `use` value ("sig"/"enc").
struct OkpConfig {
  name: &'static str,
  crv: &'static str,
  len: usize,
  /// Allowed usages for a private key.
  private_usages: &'static [&'static str],
  /// Allowed usages for a public key (raw/spki/jwk-public).
  public_usages: &'static [&'static str],
  jwk_use: &'static str,
}

fn import_ed25519(p: &ImportKeyParams) -> Res<KeyDesc> {
  import_okp(
    p,
    OkpConfig {
      name: "Ed25519",
      crv: "Ed25519",
      len: 32,
      private_usages: &["sign"],
      public_usages: &["verify"],
      jwk_use: "sig",
    },
    crate::ed25519::import_spki_ed25519,
    crate::ed25519::import_pkcs8_ed25519,
  )
}

fn import_x25519(p: &ImportKeyParams) -> Res<KeyDesc> {
  import_okp(
    p,
    OkpConfig {
      name: "X25519",
      crv: "X25519",
      len: 32,
      private_usages: &["deriveKey", "deriveBits"],
      public_usages: &[],
      jwk_use: "enc",
    },
    crate::x25519::import_spki_x25519,
    crate::x25519::import_pkcs8_x25519,
  )
}

fn import_x448(p: &ImportKeyParams) -> Res<KeyDesc> {
  import_okp(
    p,
    OkpConfig {
      name: "X448",
      crv: "X448",
      len: 56,
      private_usages: &["deriveKey", "deriveBits"],
      public_usages: &[],
      jwk_use: "enc",
    },
    crate::x448::import_spki_x448,
    crate::x448::import_pkcs8_x448,
  )
}

fn import_okp(
  p: &ImportKeyParams,
  cfg: OkpConfig,
  import_spki: fn(&[u8]) -> Option<Vec<u8>>,
  import_pkcs8: fn(&[u8]) -> Option<Vec<u8>>,
) -> Res<KeyDesc> {
  let alg = || AlgDesc::Name(cfg.name.to_string());
  match p.format.as_str() {
    // `raw-public` is an alias of `raw` for the existing asymmetric algorithms.
    "raw" | "raw-public" => {
      // Ed25519 raw is a public key with usages restricted to "verify";
      // X25519/X448 raw requires empty usages.
      if cfg.name == "Ed25519" {
        if has_disallowed_usage(&p.key_usages, &["verify"]) {
          return Err(syntax("Invalid key usage"));
        }
      } else if !p.key_usages.is_empty() {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      if buf.len() != cfg.len {
        return Err(data("Invalid key data"));
      }
      let usages = if cfg.name == "Ed25519" {
        usage_intersection(&p.key_usages, RECOGNISED_USAGES)
      } else {
        vec![]
      };
      Ok(KeyDesc::new(
        "public",
        p.extractable,
        usages,
        alg(),
        KeyDataDesc::Raw(buf.to_vec()),
      ))
    }
    "spki" => {
      if cfg.name == "Ed25519" {
        if has_disallowed_usage(&p.key_usages, &["verify"]) {
          return Err(syntax("Invalid key usage"));
        }
      } else if !p.key_usages.is_empty() {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let pub_bytes =
        import_spki(buf).ok_or_else(|| data("Invalid key data"))?;
      if pub_bytes.len() != cfg.len {
        return Err(data("Invalid key data"));
      }
      let usages = if cfg.name == "Ed25519" {
        usage_intersection(&p.key_usages, RECOGNISED_USAGES)
      } else {
        vec![]
      };
      Ok(KeyDesc::new(
        "public",
        p.extractable,
        usages,
        alg(),
        KeyDataDesc::Raw(pub_bytes),
      ))
    }
    "pkcs8" => {
      if has_disallowed_usage(&p.key_usages, cfg.private_usages) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let priv_bytes =
        import_pkcs8(buf).ok_or_else(|| data("Invalid key data"))?;
      if priv_bytes.len() != cfg.len {
        return Err(data("Invalid key data"));
      }
      Ok(KeyDesc::new(
        "private",
        p.extractable,
        usage_intersection(&p.key_usages, RECOGNISED_USAGES),
        alg(),
        KeyDataDesc::Raw(priv_bytes),
      ))
    }
    "jwk" => import_okp_jwk(p, &cfg),
    _ => Err(not_supported("Not implemented")),
  }
}

fn import_okp_jwk(p: &ImportKeyParams, cfg: &OkpConfig) -> Res<KeyDesc> {
  let jwk = jwk(p)?;
  let is_private = jwk_has(jwk, "d");

  // 2.
  if is_private {
    if has_disallowed_usage(&p.key_usages, cfg.private_usages) {
      return Err(syntax("Invalid key usage"));
    }
  } else if cfg.name == "Ed25519" {
    if has_disallowed_usage(&p.key_usages, cfg.public_usages) {
      return Err(syntax("Invalid key usage"));
    }
  } else if !p.key_usages.is_empty() {
    return Err(syntax("Invalid key usage"));
  }

  // kty.
  if jwk_str(jwk, "kty") != Some("OKP") {
    return Err(data("Invalid key type"));
  }
  // crv.
  if jwk_str(jwk, "crv") != Some(cfg.crv) {
    return Err(data("Invalid curve"));
  }
  // use.
  if !p.key_usages.is_empty()
    && let Some(usev) = jwk_str(jwk, "use")
    && usev != cfg.jwk_use
  {
    return Err(data("Invalid key use"));
  }
  // key_ops.
  validate_key_ops(jwk, &p.key_usages)?;
  // ext.
  validate_ext(jwk, p.extractable)?;

  if is_private {
    let d =
      jwk_str(jwk, "d").ok_or_else(|| data("Invalid private key data"))?;
    let bytes =
      b64url_decode(d).map_err(|_| data("Invalid private key data"))?;
    if bytes.len() != cfg.len {
      return Err(data("Invalid private key data"));
    }
    Ok(KeyDesc::new(
      "private",
      p.extractable,
      usage_intersection(&p.key_usages, cfg.private_usages),
      AlgDesc::Name(cfg.name.to_string()),
      KeyDataDesc::Raw(bytes),
    ))
  } else {
    let x = jwk_str(jwk, "x").ok_or_else(|| data("Invalid public key data"))?;
    let bytes =
      b64url_decode(x).map_err(|_| data("Invalid public key data"))?;
    if bytes.len() != cfg.len {
      return Err(data("Invalid public key data"));
    }
    let usages = if cfg.name == "Ed25519" {
      usage_intersection(&p.key_usages, RECOGNISED_USAGES)
    } else {
      vec![]
    };
    Ok(KeyDesc::new(
      "public",
      p.extractable,
      usages,
      AlgDesc::Name(cfg.name.to_string()),
      KeyDataDesc::Raw(bytes),
    ))
  }
}

// ----- ML-KEM --------------------------------------------------------------

/// Build an ML-KEM private-key [`KeyDesc`] carrying the FIPS 203 seed (when
/// available) and the expanded decapsulation key, mirroring the ML-DSA
/// `{ seed, privateKey }` model. `seed` is `None` for keys imported from the
/// expanded `raw-private` form (which carry no seed).
fn mlkem_private_desc(
  name: &str,
  extractable: bool,
  usages: &[String],
  seed: Option<Vec<u8>>,
  private_key: Vec<u8>,
) -> KeyDesc {
  KeyDesc::new(
    "private",
    extractable,
    usage_intersection(usages, ML_KEM_PRIVATE_USAGES),
    AlgDesc::Name(name.to_string()),
    KeyDataDesc::MlDsaPrivate { seed, private_key },
  )
}

fn import_mlkem(p: &ImportKeyParams) -> Res<KeyDesc> {
  let name = p.name.clone();
  let variant = MlKemVariant::from_name(&name)
    .ok_or_else(|| not_supported("Unsupported"))?;
  let alg = || AlgDesc::Name(name.clone());
  match p.format.as_str() {
    "raw-public" => {
      let buf = buffer(p)?;
      if buf.len() != variant.public_key_size() {
        return Err(data("Invalid key data"));
      }
      if has_disallowed_usage(&p.key_usages, ML_KEM_PUBLIC_USAGES) {
        return Err(syntax("Invalid key usage"));
      }
      if !crate::mlkem::ml_kem_validate_public_key(variant, buf) {
        return Err(data("Invalid key data"));
      }
      Ok(KeyDesc::new(
        "public",
        p.extractable,
        usage_intersection(&p.key_usages, ML_KEM_PUBLIC_USAGES),
        alg(),
        KeyDataDesc::Raw(buf.to_vec()),
      ))
    }
    "raw-private" => {
      // The expanded decapsulation key; carries no seed.
      let buf = buffer(p)?;
      if buf.len() != variant.private_key_size() {
        return Err(data("Invalid key data"));
      }
      if has_disallowed_usage(&p.key_usages, ML_KEM_PRIVATE_USAGES) {
        return Err(syntax("Invalid key usage"));
      }
      if !crate::mlkem::ml_kem_validate_private_key(variant, buf) {
        return Err(data("Invalid key data"));
      }
      Ok(mlkem_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        None,
        buf.to_vec(),
      ))
    }
    "raw-seed" => {
      let buf = buffer(p)?;
      if buf.len() != 64 {
        return Err(data("Invalid key data"));
      }
      if has_disallowed_usage(&p.key_usages, ML_KEM_PRIVATE_USAGES) {
        return Err(syntax("Invalid key usage"));
      }
      let (private_key, _public_key) =
        crate::mlkem::ml_kem_from_seed(variant, buf)
          .map_err(|_| data("Invalid key data"))?;
      Ok(mlkem_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        Some(buf.to_vec()),
        private_key,
      ))
    }
    "jwk" => import_mlkem_jwk(p, variant, &name),
    "spki" => {
      if has_disallowed_usage(&p.key_usages, ML_KEM_PUBLIC_USAGES) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let (v, public_key) = crate::mlkem::ml_kem_import_spki(buf)?;
      if v != variant {
        return Err(data("Imported key algorithm does not match"));
      }
      Ok(KeyDesc::new(
        "public",
        p.extractable,
        usage_intersection(&p.key_usages, ML_KEM_PUBLIC_USAGES),
        alg(),
        KeyDataDesc::Raw(public_key),
      ))
    }
    "pkcs8" => {
      if has_disallowed_usage(&p.key_usages, ML_KEM_PRIVATE_USAGES) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      // Only the seed form is supported; the expanded-key form is rejected
      // with NotSupportedError (mapped from `UnsupportedPkcs8Format`).
      let (v, seed) = crate::mlkem::ml_kem_import_pkcs8_seed(buf)?;
      if v != variant {
        return Err(data("Imported key algorithm does not match"));
      }
      let (private_key, _public_key) =
        crate::mlkem::ml_kem_from_seed(variant, &seed)
          .map_err(|_| data("Invalid key data"))?;
      Ok(mlkem_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        Some(seed),
        private_key,
      ))
    }
    _ => Err(not_supported("Unsupported key format for ML-KEM")),
  }
}

/// Import an ML-KEM JWK (`kty: "AKP"`). The private JWK carries the seed in
/// `priv` and the encapsulation key in `pub`; the public JWK carries only
/// `pub`. The seed-derived public key must match the embedded `pub` member.
fn import_mlkem_jwk(
  p: &ImportKeyParams,
  variant: MlKemVariant,
  name: &str,
) -> Res<KeyDesc> {
  let jwk = jwk(p)?;
  // 1. kty must be "AKP".
  if jwk_str(jwk, "kty") != Some("AKP") {
    return Err(data("'kty' property of JsonWebKey must be 'AKP'"));
  }
  // 2. alg, if present, must match the algorithm name.
  if let Some(a) = jwk_str(jwk, "alg")
    && a != name
  {
    return Err(data(
      "'alg' property of JsonWebKey does not match algorithm",
    ));
  }
  // 3. key_ops / ext validation.
  validate_key_ops(jwk, &p.key_usages)?;
  validate_ext(jwk, p.extractable)?;

  let has_priv = jwk_has(jwk, "priv");
  let pub_b64 = jwk_str(jwk, "pub");

  if has_priv {
    // Private (seed-bearing) key: only decapsulate usages are allowed.
    if has_disallowed_usage(&p.key_usages, ML_KEM_PRIVATE_USAGES) {
      return Err(syntax("Invalid key usage"));
    }
    let seed = b64url_decode(jwk_str(jwk, "priv").unwrap())?;
    if seed.len() != 64 {
      return Err(data("Invalid 'priv' length"));
    }
    let (private_key, public_key) =
      crate::mlkem::ml_kem_from_seed(variant, &seed)
        .map_err(|_| data("Invalid key data"))?;
    // The embedded public key (if present) must match the seed-derived one.
    if let Some(pb) = pub_b64 {
      let embedded = b64url_decode(pb)?;
      if embedded != public_key {
        return Err(data("'pub' does not match the private key"));
      }
    }
    Ok(mlkem_private_desc(
      name,
      p.extractable,
      &p.key_usages,
      Some(seed),
      private_key,
    ))
  } else {
    // Public key: only encapsulate usages are allowed.
    if has_disallowed_usage(&p.key_usages, ML_KEM_PUBLIC_USAGES) {
      return Err(syntax("Invalid key usage"));
    }
    let pb = pub_b64
      .ok_or_else(|| data("'pub' property of JsonWebKey is required"))?;
    let public_key = b64url_decode(pb)?;
    if public_key.len() != variant.public_key_size() {
      return Err(data("Invalid 'pub' length"));
    }
    if !crate::mlkem::ml_kem_validate_public_key(variant, &public_key) {
      return Err(data("Invalid key data"));
    }
    Ok(KeyDesc::new(
      "public",
      p.extractable,
      usage_intersection(&p.key_usages, ML_KEM_PUBLIC_USAGES),
      AlgDesc::Name(name.to_string()),
      KeyDataDesc::Raw(public_key),
    ))
  }
}

// ----- ML-DSA --------------------------------------------------------------

fn mldsa_public_desc(
  name: &str,
  extractable: bool,
  usages: &[String],
  public_bytes: Vec<u8>,
) -> KeyDesc {
  KeyDesc::new(
    "public",
    extractable,
    usage_intersection(usages, &["verify"]),
    AlgDesc::Name(name.to_string()),
    KeyDataDesc::Raw(public_bytes),
  )
}

fn mldsa_private_desc(
  name: &str,
  extractable: bool,
  usages: &[String],
  seed: Option<Vec<u8>>,
  private_bytes: Vec<u8>,
  public_bytes: Vec<u8>,
) -> KeyDesc {
  let mut desc = KeyDesc::new(
    "private",
    extractable,
    usage_intersection(usages, &["sign"]),
    AlgDesc::Name(name.to_string()),
    KeyDataDesc::MlDsaPrivate {
      seed,
      private_key: private_bytes,
    },
  );
  desc.mldsa_public = Some(Box::new(mldsa_public_desc(
    name,
    extractable,
    usages,
    public_bytes,
  )));
  desc
}

fn import_mldsa(p: &ImportKeyParams) -> Res<KeyDesc> {
  let name = p.name.clone();
  let variant = mldsa_variant_id(&name)?;
  match p.format.as_str() {
    "raw-seed" => {
      if has_disallowed_usage(&p.key_usages, &["sign"]) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      if buf.len() != 32 {
        return Err(data("Invalid key data"));
      }
      let (private_key, public_key) =
        crate::mldsa::mldsa_from_seed(variant, buf)?;
      Ok(mldsa_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        Some(buf.to_vec()),
        private_key,
        public_key,
      ))
    }
    "raw-private" => {
      if has_disallowed_usage(&p.key_usages, &["sign"]) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let (private_key, public_key) =
        crate::mldsa::mldsa_from_raw_private(variant, buf)?;
      Ok(mldsa_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        None,
        private_key,
        public_key,
      ))
    }
    "raw-public" => {
      if has_disallowed_usage(&p.key_usages, &["verify"]) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      if buf.len() != mldsa_public_key_len(variant) {
        return Err(data("Invalid key data"));
      }
      Ok(mldsa_public_desc(
        &name,
        p.extractable,
        &p.key_usages,
        buf.to_vec(),
      ))
    }
    "pkcs8" => {
      if has_disallowed_usage(&p.key_usages, &["sign"]) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let (private_key, public_key, seed) =
        crate::mldsa::mldsa_from_pkcs8(variant, buf)?;
      Ok(mldsa_private_desc(
        &name,
        p.extractable,
        &p.key_usages,
        seed,
        private_key,
        public_key,
      ))
    }
    "spki" => {
      if has_disallowed_usage(&p.key_usages, &["verify"]) {
        return Err(syntax("Invalid key usage"));
      }
      let buf = buffer(p)?;
      let public_key = crate::mldsa::mldsa_from_spki(variant, buf)?;
      Ok(mldsa_public_desc(
        &name,
        p.extractable,
        &p.key_usages,
        public_key,
      ))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

// ---------------------------------------------------------------------------
// generateKey
// ---------------------------------------------------------------------------

/// Parameters for `constructGeneratedKey`: the normalized algorithm fields, the
/// usages/extractable, and the freshly-generated raw key material (the result
/// of the async `op_crypto_generate_key_web`).
pub struct GenerateBuildParams {
  pub name: String,
  pub extractable: bool,
  pub usages: Vec<String>,
  pub hash: Option<String>,
  pub modulus_length: Option<usize>,
  pub public_exponent: Option<Vec<u8>>,
  pub named_curve: Option<String>,
  pub length: Option<usize>,
  /// Single secret/private blob (AES/HMAC/ChaCha/RSA).
  pub data: Option<Vec<u8>>,
  /// Key-pair material (curves / PQ).
  pub public_key: Option<Vec<u8>>,
  pub private_key: Option<Vec<u8>>,
  /// ML-DSA seed.
  pub seed: Option<Vec<u8>>,
}

impl<'a> FromV8<'a> for GenerateBuildParams {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let obj = value
      .try_cast::<v8::Object>()
      .map_err(|_| JsErrorBox::type_error("Expected object"))?;
    Ok(GenerateBuildParams {
      name: ku::get_string(scope, obj, "name").unwrap_or_default(),
      extractable: ku::get_value(scope, obj, "extractable")
        .map(|v| v.boolean_value(scope))
        .unwrap_or(false),
      usages: get_string_array(scope, obj, "usages"),
      hash: ku::get_string(scope, obj, "hash"),
      modulus_length: ku::get_usize(scope, obj, "modulusLength"),
      public_exponent: ku::get_buffer(scope, obj, "publicExponent"),
      named_curve: ku::get_string(scope, obj, "namedCurve"),
      length: ku::get_usize(scope, obj, "length"),
      data: ku::get_buffer(scope, obj, "data"),
      public_key: ku::get_buffer(scope, obj, "publicKey"),
      private_key: ku::get_buffer(scope, obj, "privateKey"),
      seed: ku::get_buffer(scope, obj, "seed"),
    })
  }
}

fn get_string_array(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Vec<String> {
  let mut out = Vec::new();
  if let Some(v) = ku::get_value(scope, obj, key)
    && let Ok(arr) = v8::Local::<v8::Array>::try_from(v)
  {
    for i in 0..arr.length() {
      if let Some(el) = arr.get_index(scope, i) {
        out.push(el.to_rust_string_lossy(scope));
      }
    }
  }
  out
}

/// Build the `CryptoKey`(s) for a generated key from the raw material.
pub fn generate_build(p: GenerateBuildParams) -> Res<KeyOrPair> {
  let name = p.name.as_str();
  let usages = &p.usages;
  let secret = || -> Res<Vec<u8>> {
    p.data
      .clone()
      .ok_or_else(|| KeyMakerError::Operation("missing data".into()))
  };
  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" => {
      let alg = AlgDesc::Rsa {
        name: name.to_string(),
        modulus_length: p.modulus_length.unwrap_or(0),
        public_exponent: p.public_exponent.clone().unwrap_or_default(),
        hash: p.hash.clone().unwrap_or_default(),
      };
      rsa_pair(&p, alg, &["verify"], &["sign"])
    }
    "RSA-OAEP" => {
      let alg = AlgDesc::Rsa {
        name: name.to_string(),
        modulus_length: p.modulus_length.unwrap_or(0),
        public_exponent: p.public_exponent.clone().unwrap_or_default(),
        hash: p.hash.clone().unwrap_or_default(),
      };
      rsa_pair(&p, alg, &["encrypt", "wrapKey"], &["decrypt", "unwrapKey"])
    }
    "ECDSA" => ec_pair(&p, &["verify"], &["sign"]),
    "ECDH" => ec_pair(&p, &[], &["deriveKey", "deriveBits"]),
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW" => {
      Ok(KeyOrPair::Single(KeyDesc::new(
        "secret",
        p.extractable,
        usages.clone(),
        AlgDesc::AesLength {
          name: name.to_string(),
          length: p.length.unwrap_or(0),
        },
        KeyDataDesc::TypeData {
          data_type: "secret".to_string(),
          data: secret()?,
        },
      )))
    }
    "X448" | "X25519" => okp_pair(&p, &["deriveKey", "deriveBits"], &[]),
    "Ed25519" => okp_pair(&p, &["sign"], &["verify"]),
    "ChaCha20-Poly1305" => Ok(KeyOrPair::Single(KeyDesc::new(
      "secret",
      p.extractable,
      usages.clone(),
      AlgDesc::Name(name.to_string()),
      KeyDataDesc::TypeData {
        data_type: "secret".to_string(),
        data: secret()?,
      },
    ))),
    "HMAC" => {
      let data = secret()?;
      let length = data.len() * 8;
      Ok(KeyOrPair::Single(KeyDesc::new(
        "secret",
        p.extractable,
        usages.clone(),
        AlgDesc::Hmac {
          hash: p.hash.clone().unwrap_or_default(),
          length,
        },
        KeyDataDesc::TypeData {
          data_type: "secret".to_string(),
          data,
        },
      )))
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      let private_key = p
        .private_key
        .clone()
        .ok_or_else(|| KeyMakerError::Operation("missing privateKey".into()))?;
      let public_key = p
        .public_key
        .clone()
        .ok_or_else(|| KeyMakerError::Operation("missing publicKey".into()))?;
      let private_desc = mldsa_private_desc(
        name,
        p.extractable,
        usages,
        p.seed.clone(),
        private_key,
        public_key.clone(),
      );
      let public_desc = mldsa_public_desc(name, true, usages, public_key);
      Ok(KeyOrPair::Pair {
        public_key: public_desc,
        private_key: private_desc,
      })
    }
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      let private_key = p
        .private_key
        .clone()
        .ok_or_else(|| KeyMakerError::Operation("missing privateKey".into()))?;
      let public_key = p
        .public_key
        .clone()
        .ok_or_else(|| KeyMakerError::Operation("missing publicKey".into()))?;
      Ok(KeyOrPair::Pair {
        public_key: KeyDesc::new(
          "public",
          true,
          usage_intersection(&p.usages, ML_KEM_PUBLIC_USAGES),
          AlgDesc::Name(name.to_string()),
          KeyDataDesc::Raw(public_key),
        ),
        private_key: KeyDesc::new(
          "private",
          p.extractable,
          usage_intersection(&p.usages, ML_KEM_PRIVATE_USAGES),
          AlgDesc::Name(name.to_string()),
          KeyDataDesc::MlDsaPrivate {
            seed: p.seed.clone(),
            private_key,
          },
        ),
      })
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn rsa_pair(
  p: &GenerateBuildParams,
  alg: AlgDesc,
  public_usages: &[&str],
  private_usages: &[&str],
) -> Res<KeyOrPair> {
  // RSA: both keys share the same private-key blob handle.
  let data = p
    .data
    .clone()
    .ok_or_else(|| KeyMakerError::Operation("missing data".into()))?;
  let pubdata = KeyDataDesc::TypeData {
    data_type: "private".to_string(),
    data: data.clone(),
  };
  let privdata = KeyDataDesc::TypeData {
    data_type: "private".to_string(),
    data,
  };
  let alg2 = clone_alg(&alg);
  Ok(KeyOrPair::Pair {
    public_key: KeyDesc::new(
      "public",
      true,
      usage_intersection(&p.usages, public_usages),
      alg,
      pubdata,
    ),
    private_key: KeyDesc::new(
      "private",
      p.extractable,
      usage_intersection(&p.usages, private_usages),
      alg2,
      privdata,
    ),
  })
}

fn clone_alg(alg: &AlgDesc) -> AlgDesc {
  match alg {
    AlgDesc::Name(n) => AlgDesc::Name(n.clone()),
    AlgDesc::Hmac { hash, length } => AlgDesc::Hmac {
      hash: hash.clone(),
      length: *length,
    },
    AlgDesc::AesLength { name, length } => AlgDesc::AesLength {
      name: name.clone(),
      length: *length,
    },
    AlgDesc::Ec { name, named_curve } => AlgDesc::Ec {
      name: name.clone(),
      named_curve: named_curve.clone(),
    },
    AlgDesc::Rsa {
      name,
      modulus_length,
      public_exponent,
      hash,
    } => AlgDesc::Rsa {
      name: name.clone(),
      modulus_length: *modulus_length,
      public_exponent: public_exponent.clone(),
      hash: hash.clone(),
    },
  }
}

fn ec_pair(
  p: &GenerateBuildParams,
  public_usages: &[&str],
  private_usages: &[&str],
) -> Res<KeyOrPair> {
  let data = p
    .data
    .clone()
    .ok_or_else(|| KeyMakerError::Operation("missing data".into()))?;
  let curve = p.named_curve.clone().unwrap_or_default();
  let alg = AlgDesc::Ec {
    name: p.name.clone(),
    named_curve: curve.clone(),
  };
  let alg2 = AlgDesc::Ec {
    name: p.name.clone(),
    named_curve: curve,
  };
  Ok(KeyOrPair::Pair {
    public_key: KeyDesc::new(
      "public",
      true,
      usage_intersection(&p.usages, public_usages),
      alg,
      KeyDataDesc::TypeData {
        data_type: "private".to_string(),
        data: data.clone(),
      },
    ),
    private_key: KeyDesc::new(
      "private",
      p.extractable,
      usage_intersection(&p.usages, private_usages),
      alg2,
      KeyDataDesc::TypeData {
        data_type: "private".to_string(),
        data,
      },
    ),
  })
}

fn okp_pair(
  p: &GenerateBuildParams,
  private_usages: &[&str],
  public_usages: &[&str],
) -> Res<KeyOrPair> {
  let public_key = p
    .public_key
    .clone()
    .ok_or_else(|| KeyMakerError::Operation("missing publicKey".into()))?;
  let private_key = p
    .private_key
    .clone()
    .ok_or_else(|| KeyMakerError::Operation("missing privateKey".into()))?;
  Ok(KeyOrPair::Pair {
    public_key: KeyDesc::new(
      "public",
      true,
      usage_intersection(&p.usages, public_usages),
      AlgDesc::Name(p.name.clone()),
      KeyDataDesc::Raw(public_key),
    ),
    private_key: KeyDesc::new(
      "private",
      p.extractable,
      usage_intersection(&p.usages, private_usages),
      AlgDesc::Name(p.name.clone()),
      KeyDataDesc::Raw(private_key),
    ),
  })
}

// ---------------------------------------------------------------------------
// exportKey
// ---------------------------------------------------------------------------

/// Snapshot of a CryptoKey for export, extracted in the sync op2 prelude.
pub struct ExportKeySnapshot {
  pub algorithm_name: String,
  pub named_curve: Option<String>,
  pub hash: Option<String>,
  pub length: Option<usize>,
  pub key_type: String,
  pub extractable: bool,
  pub usages: Vec<String>,
  /// `{ type, data }` (web algorithms), decoded to `V8RawKeyData`.
  pub raw_key_data: Option<crate::shared::V8RawKeyData>,
  /// Raw key bytes (Ed25519/X25519/X448/ML-KEM/ML-DSA public).
  pub raw: Option<Vec<u8>>,
  /// ML-DSA private: `(seed, privateKey)`.
  pub mldsa_private: Option<(Option<Vec<u8>>, Vec<u8>)>,
  /// ML-KEM private: `(seed, expandedPrivateKey)`. `seed` is `None` for keys
  /// imported from the expanded `raw-private` form.
  pub mlkem_private: Option<(Option<Vec<u8>>, Vec<u8>)>,
}

impl<'a> FromV8<'a> for ExportKeySnapshot {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let key =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
        .ok_or_else(|| {
          JsErrorBox::type_error("Argument is not a CryptoKey".to_string())
        })?;
    Ok(snapshot_for_export(scope, &key))
  }
}

fn snapshot_for_export(
  scope: &mut v8::PinScope<'_, '_>,
  key: &CryptoKey,
) -> ExportKeySnapshot {
  let alg = v8::Local::new(scope, &key.algorithm);
  let name = ku::get_string(scope, alg, "name").unwrap_or_default();
  let named_curve = ku::get_string(scope, alg, "namedCurve");
  let hash = ku::get_hash_name(scope, alg);
  let length = ku::get_usize(scope, alg, "length");
  let key_data_v = v8::Local::new(scope, &key.key_data);

  let mut raw_key_data = None;
  let mut raw = None;
  let mut mldsa_private = None;
  let mut mlkem_private = None;

  match name.as_str() {
    "Ed25519" | "X25519" | "X448" => {
      raw = ku::buffer_bytes(key_data_v);
    }
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      if key.key_type == "private" {
        if let Ok(obj) = key_data_v.try_cast::<v8::Object>() {
          let seed = ku::get_buffer(scope, obj, "seed");
          let private_key =
            ku::get_buffer(scope, obj, "privateKey").unwrap_or_default();
          mlkem_private = Some((seed, private_key));
        }
      } else {
        raw = ku::buffer_bytes(key_data_v);
      }
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      if key.key_type == "private" {
        if let Ok(obj) = key_data_v.try_cast::<v8::Object>() {
          let seed = ku::get_buffer(scope, obj, "seed");
          let private_key =
            ku::get_buffer(scope, obj, "privateKey").unwrap_or_default();
          mldsa_private = Some((seed, private_key));
        }
      } else {
        raw = ku::buffer_bytes(key_data_v);
      }
    }
    _ => {
      raw_key_data = ku::raw_key_data(scope, key_data_v);
    }
  }

  ExportKeySnapshot {
    algorithm_name: name,
    named_curve,
    hash,
    length,
    key_type: key.key_type.clone(),
    extractable: *key.extractable.borrow(),
    usages: key.usages.borrow().clone(),
    raw_key_data,
    raw,
    mldsa_private,
    mlkem_private,
  }
}

/// Either raw bytes or a JWK object.
pub enum ExportResult {
  Buffer(Vec<u8>),
  Jwk(Value),
}

pub fn export_key_compute(
  format: &str,
  mut key: ExportKeySnapshot,
) -> Res<ExportResult> {
  let extractable = key.extractable;
  let result = match key.algorithm_name.as_str() {
    "HMAC" | "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" | "ECDH"
    | "ECDSA" | "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" | "AES-KW"
    | "ChaCha20-Poly1305" => export_web(format, &mut key)?,
    "Ed25519" => export_ed25519(format, &key)?,
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => export_mldsa(format, &key)?,
    "X448" => export_x448(format, &key)?,
    "X25519" => export_x25519(format, &key)?,
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => export_mlkem(format, &key)?,
    _ => return Err(not_supported("Not implemented")),
  };
  // Extractability is enforced after computing the result (matching the JS
  // ordering), but the web algorithms already enforce it in
  // `export_key_web_inner`; the others enforce it here.
  if !extractable {
    return Err(invalid_access("Key is not extractable"));
  }
  Ok(result)
}

fn export_web(format: &str, key: &mut ExportKeySnapshot) -> Res<ExportResult> {
  let v8_key_data = key
    .raw_key_data
    .take()
    .ok_or_else(|| KeyMakerError::Operation("Key is not available".into()))?;
  let arg = ExportKeyWebArg {
    format: format.to_string(),
    name: key.algorithm_name.clone(),
    named_curve: key.named_curve.clone(),
    hash: key.hash.clone(),
    length: key.length,
    key_type: key.key_type.clone(),
    extractable: key.extractable,
    usages: key.usages.clone(),
  };
  let res = crate::web_import_export::export_key_web_inner(arg, v8_key_data)?;
  Ok(match res {
    ExportKeyWebResult::Buffer(b) => ExportResult::Buffer(b),
    ExportKeyWebResult::Jwk(v) => ExportResult::Jwk(v),
  })
}

fn require_public(key: &ExportKeySnapshot) -> Res<()> {
  if key.key_type != "public" {
    return Err(invalid_access("Key is not a public key"));
  }
  Ok(())
}
fn require_private(key: &ExportKeySnapshot) -> Res<()> {
  if key.key_type != "private" {
    return Err(invalid_access("Key is not a private key"));
  }
  Ok(())
}

fn raw_bytes(key: &ExportKeySnapshot) -> Res<&Vec<u8>> {
  key
    .raw
    .as_ref()
    .ok_or_else(|| KeyMakerError::Operation("Key is not available".into()))
}

fn export_ed25519(format: &str, key: &ExportKeySnapshot) -> Res<ExportResult> {
  match format {
    // `raw-public` is an alias of `raw` for the existing asymmetric algorithms.
    "raw" | "raw-public" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(raw_bytes(key)?.clone()))
    }
    "spki" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(crate::ed25519::export_spki_ed25519(
        raw_bytes(key)?,
      )?))
    }
    "pkcs8" => {
      require_private(key)?;
      let inner = raw_bytes(key)?;
      let mut der = vec![0x04, 0x22];
      der.extend_from_slice(inner);
      let mut out = crate::ed25519::export_pkcs8_ed25519(&der)?;
      out[15] = 0x20;
      Ok(ExportResult::Buffer(out))
    }
    "jwk" => {
      let inner = raw_bytes(key)?;
      let x = if key.key_type == "private" {
        crate::ed25519::jwk_x_ed25519(inner)?
      } else {
        BASE64_URL_SAFE_NO_PAD.encode(inner)
      };
      let mut jwk = json!({
        "kty": "OKP",
        "crv": "Ed25519",
        "x": x,
        "key_ops": key.usages,
        "ext": key.extractable,
      });
      if key.key_type == "private" {
        jwk["d"] = json!(BASE64_URL_SAFE_NO_PAD.encode(inner));
      }
      Ok(ExportResult::Jwk(jwk))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn export_x25519(format: &str, key: &ExportKeySnapshot) -> Res<ExportResult> {
  match format {
    "raw" | "raw-public" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(raw_bytes(key)?.clone()))
    }
    "spki" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(crate::x25519::export_spki_x25519(
        raw_bytes(key)?,
      )?))
    }
    "pkcs8" => {
      require_private(key)?;
      let inner = raw_bytes(key)?;
      let mut der = vec![0x04, 0x22];
      der.extend_from_slice(inner);
      let mut out = crate::x25519::export_pkcs8_x25519(&der)?;
      out[15] = 0x20;
      Ok(ExportResult::Buffer(out))
    }
    "jwk" => {
      let inner = raw_bytes(key)?;
      let mut jwk = json!({
        "kty": "OKP",
        "crv": "X25519",
        "key_ops": key.usages,
        "ext": key.extractable,
      });
      if key.key_type == "private" {
        jwk["x"] = json!(crate::x25519::x25519_public_key(inner));
        jwk["d"] = json!(BASE64_URL_SAFE_NO_PAD.encode(inner));
      } else {
        jwk["x"] = json!(BASE64_URL_SAFE_NO_PAD.encode(inner));
      }
      Ok(ExportResult::Jwk(jwk))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

fn export_x448(format: &str, key: &ExportKeySnapshot) -> Res<ExportResult> {
  match format {
    "raw" | "raw-public" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(raw_bytes(key)?.clone()))
    }
    "spki" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(crate::x448::export_spki_x448(
        raw_bytes(key)?,
      )?))
    }
    "pkcs8" => {
      require_private(key)?;
      let inner = raw_bytes(key)?;
      let mut der = vec![0x04, 0x3a];
      der.extend_from_slice(inner);
      let mut out = crate::x448::export_pkcs8_x448(&der)?;
      out[15] = 0x38;
      Ok(ExportResult::Buffer(out))
    }
    "jwk" => {
      if key.key_type == "private" {
        return Err(not_supported("Not implemented"));
      }
      let inner = raw_bytes(key)?;
      let jwk = json!({
        "kty": "OKP",
        "crv": "X448",
        "x": BASE64_URL_SAFE_NO_PAD.encode(inner),
        "key_ops": key.usages,
        "ext": key.extractable,
      });
      Ok(ExportResult::Jwk(jwk))
    }
    _ => Err(not_supported("Not implemented")),
  }
}

/// `(seed, expandedPrivateKey)` for an ML-KEM private key snapshot.
fn mlkem_private_parts(
  key: &ExportKeySnapshot,
) -> Res<&(Option<Vec<u8>>, Vec<u8>)> {
  key
    .mlkem_private
    .as_ref()
    .ok_or_else(|| KeyMakerError::Operation("Key is not available".into()))
}

/// The seed of an ML-KEM private key; errors (NotSupportedError, matching the
/// WICG spec) if the key was imported from the expanded form (no seed).
fn mlkem_seed(key: &ExportKeySnapshot) -> Res<&Vec<u8>> {
  mlkem_private_parts(key)?.0.as_ref().ok_or_else(|| {
    not_supported(
      "This ML-KEM key was imported without a seed; seed-bearing formats \
       (raw-seed/jwk/pkcs8) cannot be exported",
    )
  })
}

fn export_mlkem(format: &str, key: &ExportKeySnapshot) -> Res<ExportResult> {
  let variant = MlKemVariant::from_name(&key.algorithm_name)
    .ok_or_else(|| not_supported("Unsupported key format for ML-KEM"))?;
  match format {
    "raw-public" => {
      if key.key_type != "public" {
        return Err(invalid_access(
          "'raw-public' is only valid for public keys",
        ));
      }
      Ok(ExportResult::Buffer(raw_bytes(key)?.clone()))
    }
    "raw-private" => {
      if key.key_type != "private" {
        return Err(invalid_access(
          "'raw-private' is only valid for private keys",
        ));
      }
      Ok(ExportResult::Buffer(mlkem_private_parts(key)?.1.clone()))
    }
    "raw-seed" => {
      require_private(key)?;
      Ok(ExportResult::Buffer(mlkem_seed(key)?.clone()))
    }
    "spki" => {
      if key.key_type != "public" {
        return Err(invalid_access("'spki' is only valid for public keys"));
      }
      Ok(ExportResult::Buffer(crate::mlkem::ml_kem_export_spki(
        variant,
        raw_bytes(key)?,
      )?))
    }
    "pkcs8" => {
      if key.key_type != "private" {
        return Err(invalid_access("'pkcs8' is only valid for private keys"));
      }
      let seed = mlkem_seed(key)?;
      Ok(ExportResult::Buffer(
        crate::mlkem::ml_kem_export_pkcs8_seed(variant, seed)?,
      ))
    }
    "jwk" => export_mlkem_jwk(variant, key),
    _ => Err(not_supported("Unsupported key format for ML-KEM")),
  }
}

/// Export an ML-KEM key as an `AKP` JWK. The public JWK carries `pub`; the
/// private JWK additionally carries `priv` (the base64url seed) and the
/// embedded `pub` (the encapsulation key derived from the seed).
fn export_mlkem_jwk(
  variant: MlKemVariant,
  key: &ExportKeySnapshot,
) -> Res<ExportResult> {
  if key.key_type == "private" {
    let seed = mlkem_seed(key)?;
    let (_private_key, public_key) =
      crate::mlkem::ml_kem_from_seed(variant, seed)
        .map_err(|_| KeyMakerError::Operation("Invalid key data".into()))?;
    let jwk = json!({
      "kty": "AKP",
      "alg": key.algorithm_name,
      "pub": BASE64_URL_SAFE_NO_PAD.encode(&public_key),
      "priv": BASE64_URL_SAFE_NO_PAD.encode(seed),
      "key_ops": key.usages,
      "ext": key.extractable,
    });
    Ok(ExportResult::Jwk(jwk))
  } else {
    let public_key = raw_bytes(key)?;
    let jwk = json!({
      "kty": "AKP",
      "alg": key.algorithm_name,
      "pub": BASE64_URL_SAFE_NO_PAD.encode(public_key),
      "key_ops": key.usages,
      "ext": key.extractable,
    });
    Ok(ExportResult::Jwk(jwk))
  }
}

fn export_mldsa(format: &str, key: &ExportKeySnapshot) -> Res<ExportResult> {
  let variant = mldsa_variant_id(&key.algorithm_name)?;
  match format {
    "raw-seed" => {
      require_private(key)?;
      let (seed, _) = key.mldsa_private.as_ref().ok_or_else(|| {
        KeyMakerError::Operation("Key is not available".into())
      })?;
      let seed = seed.as_ref().ok_or_else(|| {
        KeyMakerError::Operation("Seed is not available for this key".into())
      })?;
      Ok(ExportResult::Buffer(seed.clone()))
    }
    "raw-private" => {
      require_private(key)?;
      let (_, private_key) = key.mldsa_private.as_ref().ok_or_else(|| {
        KeyMakerError::Operation("Key is not available".into())
      })?;
      Ok(ExportResult::Buffer(private_key.clone()))
    }
    "raw-public" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(raw_bytes(key)?.clone()))
    }
    "pkcs8" => {
      require_private(key)?;
      let (seed, _) = key.mldsa_private.as_ref().ok_or_else(|| {
        KeyMakerError::Operation("Key is not available".into())
      })?;
      let seed = seed.as_ref().ok_or_else(|| {
        KeyMakerError::Operation(
          "PKCS#8 export requires the original ML-DSA seed; this key was \
           imported without one"
            .into(),
        )
      })?;
      Ok(ExportResult::Buffer(crate::mldsa::mldsa_export_pkcs8(
        variant, seed,
      )?))
    }
    "spki" => {
      require_public(key)?;
      Ok(ExportResult::Buffer(crate::mldsa::mldsa_export_spki(
        variant,
        raw_bytes(key)?,
      )?))
    }
    _ => Err(not_supported("Not implemented")),
  }
}
