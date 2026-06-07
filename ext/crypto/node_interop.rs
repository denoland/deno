// Copyright 2018-2026 the Deno authors. MIT license.

//! Node.js interop helpers exposed as static methods on the `CryptoKey`
//! cppgc class.
//!
//! Replaces the legacy `cryptoKeyExportNodeKeyMaterial` /
//! `importCryptoKeySync` JS bridges. Both shapes are needed by
//! `ext/node/polyfills/internal/crypto/keys.ts`; declaring them on the
//! cppgc class keeps the JS bootstrap a no-op (no extra ops).

use deno_core::v8;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKey;
use crate::crypto_key::CryptoKeyType;
use crate::export_key::ExportKeyAlgorithm;
use crate::export_key::ExportKeyFormat;
use crate::export_key::ExportKeyOptions;
use crate::export_key::export_key_with_raw;
use crate::make_key::AlgorithmDict;
use crate::make_key::make_crypto_key;
use crate::shared::EcNamedCurve;
use crate::shared::RawKeyData;
use crate::subtle_export_key::KeyFormat;
use crate::subtle_import_key::ImportAlgorithm;
use crate::subtle_import_key::ImportKeyData;
use crate::subtle_import_key::run as run_import_key;

/// Output of `CryptoKey.exportNodeKeyMaterial(key)` — `{ type, data }`.
pub struct NodeKeyMaterial {
  pub key_type: &'static str,
  pub data: Vec<u8>,
}

/// Body of `cryptoKeyExportNodeKeyMaterial`. The returned `data` is the
/// raw bytes (for secret keys), SPKI DER (public keys), or PKCS#8 DER
/// (private keys), matching the legacy JS behavior including the
/// special PKCS#8 wrapper for the OKP curves.
pub fn export_node_key_material<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Value>,
) -> Result<v8::Local<'s, v8::Value>, CryptoError> {
  let ptr = deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, key)
    .ok_or_else(|| {
      CryptoError::Other(JsErrorBox::type_error("Argument is not a CryptoKey"))
    })?;
  let key_obj = &ptr;
  let key_type = key_obj.key_type();
  let alg_name = key_obj.algorithm_name(scope).ok_or_else(|| {
    CryptoError::Other(JsErrorBox::type_error("Missing algorithm.name"))
  })?;
  let handle_ptr = key_obj.key_handle(scope).ok_or_else(|| {
    CryptoError::Other(JsErrorBox::type_error("CryptoKey handle missing"))
  })?;
  let raw = handle_ptr.data();
  let alg_named_curve = read_named_curve(scope, key_obj);

  let mat = match key_type {
    CryptoKeyType::Secret => NodeKeyMaterial {
      key_type: "secret",
      data: raw.bytes().to_vec(),
    },
    CryptoKeyType::Public => {
      let data = export_asym_spki(&alg_name, alg_named_curve.as_deref(), raw)?;
      NodeKeyMaterial {
        key_type: "public",
        data,
      }
    }
    CryptoKeyType::Private => {
      let data = export_asym_pkcs8(&alg_name, alg_named_curve.as_deref(), raw)?;
      NodeKeyMaterial {
        key_type: "private",
        data,
      }
    }
  };

  // Build {type, data: Uint8Array} v8 object.
  let obj = v8::Object::new(scope);
  let tk = v8::String::new(scope, "type").unwrap();
  let tv = v8::String::new(scope, mat.key_type).unwrap();
  obj.set(scope, tk.into(), tv.into());
  let dk = v8::String::new(scope, "data").unwrap();
  let backing = if mat.data.is_empty() {
    v8::ArrayBuffer::new(scope, 0)
  } else {
    let bs = v8::ArrayBuffer::new_backing_store_from_bytes(
      mat.data.clone().into_boxed_slice(),
    )
    .make_shared();
    v8::ArrayBuffer::with_backing_store(scope, &bs)
  };
  let u8 = v8::Uint8Array::new(scope, backing, 0, mat.data.len()).unwrap();
  obj.set(scope, dk.into(), u8.into());
  Ok(obj.into())
}

fn read_named_curve<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &CryptoKey,
) -> Option<String> {
  let alg = key.algorithm_local(scope)?;
  let k = v8::String::new_from_one_byte(
    scope,
    b"namedCurve",
    v8::NewStringType::Internalized,
  )?;
  let v = alg.get(scope, k.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  Some(v.to_rust_string_lossy(scope))
}

fn export_asym_spki(
  name: &str,
  named_curve: Option<&str>,
  raw: &RawKeyData,
) -> Result<Vec<u8>, CryptoError> {
  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => {
      let alg = match name {
        "RSASSA-PKCS1-v1_5" => ExportKeyAlgorithm::RsassaPkcs1v15 {},
        "RSA-PSS" => ExportKeyAlgorithm::RsaPss {},
        "RSA-OAEP" => ExportKeyAlgorithm::RsaOaep {},
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Spki, alg);
      result_bytes(export_key_with_raw(opts, raw))
    }
    "ECDH" | "ECDSA" => {
      let curve = ec_curve(named_curve)?;
      let alg = match name {
        "ECDH" => ExportKeyAlgorithm::Ecdh { named_curve: curve },
        "ECDSA" => ExportKeyAlgorithm::Ecdsa { named_curve: curve },
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Spki, alg);
      result_bytes(export_key_with_raw(opts, raw))
    }
    "Ed25519" => export_okp_spki(raw, &crate::ed25519::ED25519_OID),
    "X25519" => export_okp_spki(raw, &crate::x25519::X25519_OID),
    "X448" => export_okp_spki(raw, &crate::x448::X448_OID),
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      let variant = mldsa_variant(name);
      crate::mldsa::mldsa_export_spki(variant, raw.bytes())
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))
    }
    other => Err(type_error(format!("Unsupported algorithm: {other}"))),
  }
}

fn export_asym_pkcs8(
  name: &str,
  named_curve: Option<&str>,
  raw: &RawKeyData,
) -> Result<Vec<u8>, CryptoError> {
  match name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => {
      let alg = match name {
        "RSASSA-PKCS1-v1_5" => ExportKeyAlgorithm::RsassaPkcs1v15 {},
        "RSA-PSS" => ExportKeyAlgorithm::RsaPss {},
        "RSA-OAEP" => ExportKeyAlgorithm::RsaOaep {},
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Pkcs8, alg);
      result_bytes(export_key_with_raw(opts, raw))
    }
    "ECDH" | "ECDSA" => {
      let curve = ec_curve(named_curve)?;
      let alg = match name {
        "ECDH" => ExportKeyAlgorithm::Ecdh { named_curve: curve },
        "ECDSA" => ExportKeyAlgorithm::Ecdsa { named_curve: curve },
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Pkcs8, alg);
      result_bytes(export_key_with_raw(opts, raw))
    }
    "Ed25519" => export_okp_pkcs8(raw, &crate::ed25519::ED25519_OID, 0x20),
    "X25519" => export_okp_pkcs8(raw, &crate::x25519::X25519_OID, 0x20),
    "X448" => export_okp_pkcs8(raw, &crate::x448::X448_OID, 0x38),
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      let variant = mldsa_variant(name);
      let seed = raw.seed().ok_or_else(|| {
        type_error(format!("Cannot export {name} private key without a seed"))
      })?;
      crate::mldsa::mldsa_export_pkcs8(variant, seed)
        .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))
    }
    other => Err(type_error(format!("Unsupported algorithm: {other}"))),
  }
}

fn export_okp_spki(
  raw: &RawKeyData,
  oid: &const_oid::ObjectIdentifier,
) -> Result<Vec<u8>, CryptoError> {
  use spki::der::Encode;
  let bit_string = spki::der::asn1::BitString::from_bytes(raw.bytes())
    .map_err(|e| {
      CryptoError::Other(JsErrorBox::type_error(format!(
        "OKP SPKI encode failed: {e:?}"
      )))
    })?;
  let info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      oid: *oid,
      parameters: None,
    },
    subject_public_key: bit_string,
  };
  info.to_der().map_err(|e| {
    CryptoError::Other(JsErrorBox::type_error(format!(
      "OKP SPKI encode failed: {e:?}"
    )))
  })
}

fn export_okp_pkcs8(
  raw: &RawKeyData,
  oid: &const_oid::ObjectIdentifier,
  inner_len_byte: u8,
) -> Result<Vec<u8>, CryptoError> {
  use rsa::pkcs8 as pk8;
  use rsa::pkcs8::der::Encode;
  // CurvePrivateKey ::= OCTET STRING. The PKCS#8 wrapper provides:
  //   [0x04, inner_len, ...raw bytes].
  let inner_len = raw.bytes().len() as u8;
  let mut wrapped = Vec::with_capacity(2 + raw.bytes().len());
  wrapped.push(0x04);
  wrapped.push(inner_len);
  wrapped.extend_from_slice(raw.bytes());
  let pk = pk8::PrivateKeyInfo {
    algorithm: pk8::AlgorithmIdentifierRef {
      oid: *oid,
      parameters: None,
    },
    private_key: &wrapped,
    public_key: None,
  };
  let mut out = pk.to_der().map_err(|e| {
    CryptoError::Other(JsErrorBox::type_error(format!(
      "OKP PKCS8 encode failed: {e:?}"
    )))
  })?;
  // The legacy JS wrote `data[15] = inner_len_byte` to set the inner
  // OCTET STRING length byte. Match the legacy behavior for byte-perfect
  // parity with consumers that grew tolerant of the original encoder.
  if out.len() > 15 {
    out[15] = inner_len_byte;
  }
  Ok(out)
}

fn ec_curve(named: Option<&str>) -> Result<EcNamedCurve, CryptoError> {
  match named {
    Some("P-256") => Ok(EcNamedCurve::P256),
    Some("P-384") => Ok(EcNamedCurve::P384),
    Some("P-521") => Ok(EcNamedCurve::P521),
    _ => Err(type_error("Unsupported namedCurve".to_string())),
  }
}

fn mldsa_variant(name: &str) -> u8 {
  match name {
    "ML-DSA-44" => 0,
    "ML-DSA-65" => 1,
    "ML-DSA-87" => 2,
    _ => unreachable!(),
  }
}

fn result_bytes(
  res: Result<
    crate::export_key::ExportKeyResult,
    crate::export_key::ExportKeyError,
  >,
) -> Result<Vec<u8>, CryptoError> {
  let r = res.map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
  match r {
    crate::export_key::ExportKeyResult::Spki(b)
    | crate::export_key::ExportKeyResult::Pkcs8(b)
    | crate::export_key::ExportKeyResult::Raw(b) => Ok(b.as_ref().to_vec()),
    _ => Err(type_error("Unexpected export result".to_string())),
  }
}

fn type_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::type_error(msg))
}

/// Body of `CryptoKey.fromCloneData(data)` — invoked by the JS
/// `registerCloneableResource("CryptoKey", ...)` callback to resurrect a
/// `CryptoKey` from the snapshot produced by the host-object brand
/// callback in [`crate::make_key`]. The snapshot has shape
/// `{ type: "CryptoKey", keyType, extractable, usages, algorithm, keyData }`.
pub fn from_clone_data<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  data: v8::Local<'s, v8::Value>,
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let obj = v8::Local::<v8::Object>::try_from(data).map_err(|_| {
    CryptoError::Other(JsErrorBox::type_error("Clone data must be an object"))
  })?;
  let key_type_str = read_string_member(scope, obj, b"keyType")
    .ok_or_else(|| type_error("Missing keyType".to_string()))?;
  let key_type = match key_type_str.as_str() {
    "public" => crate::crypto_key::CryptoKeyType::Public,
    "private" => crate::crypto_key::CryptoKeyType::Private,
    "secret" => crate::crypto_key::CryptoKeyType::Secret,
    _ => return Err(type_error("Invalid keyType".to_string())),
  };
  let extractable =
    read_bool_member(scope, obj, b"extractable").unwrap_or(true);
  let usages_strs =
    read_string_array(scope, obj, b"usages").unwrap_or_default();
  let alg_name = read_algorithm_name(scope, obj)
    .ok_or_else(|| type_error("Missing algorithm.name".to_string()))?;
  let alg = build_algorithm_dict_from_v8(scope, obj, &alg_name);
  let raw = read_key_data_from_v8(scope, obj)?;
  let usages: Vec<&str> = usages_strs.iter().map(String::as_str).collect();
  Ok(make_crypto_key(
    scope,
    key_type,
    extractable,
    &usages,
    alg,
    raw,
  ))
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
  let v = obj.get(scope, key.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  Some(v.boolean_value(scope))
}

fn read_string_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<String>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, key.into())?;
  let arr = v8::Local::<v8::Array>::try_from(v).ok()?;
  let len = arr.length();
  let mut out = Vec::with_capacity(len as usize);
  for i in 0..len {
    let item = arr.get_index(scope, i)?;
    let s = item.to_string(scope)?;
    out.push(s.to_rust_string_lossy(scope));
  }
  Some(out)
}

fn read_algorithm_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<String> {
  let k = v8::String::new_from_one_byte(
    scope,
    b"algorithm",
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, k.into())?;
  let alg = v8::Local::<v8::Object>::try_from(v).ok()?;
  read_string_member(scope, alg, b"name")
}

fn build_algorithm_dict_from_v8<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  name: &str,
) -> AlgorithmDict {
  let mut dict = AlgorithmDict::new(name);
  let alg_key = v8::String::new(scope, "algorithm").unwrap();
  if let Some(alg_val) = obj.get(scope, alg_key.into())
    && let Ok(alg_obj) = v8::Local::<v8::Object>::try_from(alg_val)
  {
    let length_key = v8::String::new(scope, "length").unwrap();
    if let Some(l) = alg_obj.get(scope, length_key.into()).and_then(|v| {
      if v.is_undefined() {
        None
      } else {
        v.uint32_value(scope)
      }
    }) {
      dict.length = Some(l);
    }
    let curve_key = v8::String::new(scope, "namedCurve").unwrap();
    if let Some(s) = alg_obj.get(scope, curve_key.into()).and_then(|v| {
      if v.is_undefined() {
        None
      } else {
        Some(v.to_rust_string_lossy(scope))
      }
    }) {
      dict.named_curve = Some(s);
    }
    let hash_key = v8::String::new(scope, "hash").unwrap();
    if let Some(h) = alg_obj.get(scope, hash_key.into())
      && !h.is_undefined()
      && !h.is_null()
    {
      let h_name = if h.is_string() {
        Some(h.to_rust_string_lossy(scope))
      } else {
        v8::Local::<v8::Object>::try_from(h).ok().and_then(|ho| {
          let nk = v8::String::new(scope, "name").unwrap();
          ho.get(scope, nk.into())
            .map(|nv| nv.to_rust_string_lossy(scope))
        })
      };
      dict.hash_name = h_name;
    }
    let ml_key = v8::String::new(scope, "modulusLength").unwrap();
    if let Some(ml) = alg_obj.get(scope, ml_key.into()).and_then(|v| {
      if v.is_undefined() {
        None
      } else {
        v.uint32_value(scope)
      }
    }) {
      dict.modulus_length = Some(ml);
    }
    let pe_key = v8::String::new(scope, "publicExponent").unwrap();
    if let Some(pe) = alg_obj.get(scope, pe_key.into())
      && let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(pe)
    {
      let mut out = vec![0u8; view.byte_length()];
      let n = view.copy_contents(&mut out);
      out.truncate(n);
      dict.public_exponent = Some(out);
    }
  }
  dict
}

fn read_key_data_from_v8<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Result<RawKeyData, CryptoError> {
  let k = v8::String::new(scope, "keyData").unwrap();
  let v = obj
    .get(scope, k.into())
    .ok_or_else(|| type_error("Missing keyData".to_string()))?;
  // Match the shape produced by `key_data_to_jsval` in `make_key.rs`:
  // either `{ type, data }`, `{ seed, privateKey }`, or a bare
  // `Uint8Array` for `Raw`.
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(v) {
    let mut out = vec![0u8; view.byte_length()];
    let n = view.copy_contents(&mut out);
    out.truncate(n);
    return Ok(RawKeyData::Raw(out.into_boxed_slice()));
  }
  let data_obj = v8::Local::<v8::Object>::try_from(v)
    .map_err(|_| type_error("keyData must be object/Uint8Array".to_string()))?;
  if let Some(type_str) = read_string_member(scope, data_obj, b"type") {
    let data_bytes = read_uint8array_member(scope, data_obj, b"data")
      .ok_or_else(|| type_error("Missing keyData.data".to_string()))?;
    let boxed = data_bytes.into_boxed_slice();
    return Ok(match type_str.as_str() {
      "secret" => RawKeyData::Secret(boxed),
      "private" => RawKeyData::Private(boxed),
      "public" => RawKeyData::Public(boxed),
      _ => RawKeyData::Raw(boxed),
    });
  }
  // SeededPrivate form.
  let pk = read_uint8array_member(scope, data_obj, b"privateKey")
    .ok_or_else(|| type_error("Missing keyData.privateKey".to_string()))?;
  let seed = read_uint8array_member(scope, data_obj, b"seed");
  Ok(RawKeyData::SeededPrivate {
    seed: seed.map(|s| s.into_boxed_slice()),
    private_key: pk.into_boxed_slice(),
  })
}

fn read_uint8array_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let k = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let v = obj.get(scope, k.into())?;
  if v.is_undefined() || v.is_null() {
    return None;
  }
  let view = v8::Local::<v8::ArrayBufferView>::try_from(v).ok()?;
  let mut out = vec![0u8; view.byte_length()];
  let n = view.copy_contents(&mut out);
  out.truncate(n);
  Some(out)
}

/// Body of `importCryptoKeySync` — synchronous import for the node:crypto
/// interop path. The format/algorithm/keyData triple is coerced via the
/// same converters used by `SubtleCrypto.importKey`.
pub fn import_sync<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  format: KeyFormat,
  key_data: v8::Local<'s, v8::Value>,
  algorithm: ImportAlgorithm,
  extractable: bool,
  usages: Vec<String>,
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  // ChaCha20-Poly1305 only knows `raw-secret`; legacy node interop
  // passes `raw` so map it on the way in.
  let format =
    if format == KeyFormat::Raw && algorithm.name == "ChaCha20-Poly1305" {
      KeyFormat::RawSecret
    } else {
      format
    };
  let data = ImportKeyData::from_v8(scope, key_data, format)?;
  run_import_key(scope, format, &algorithm, data, extractable, &usages)
}
