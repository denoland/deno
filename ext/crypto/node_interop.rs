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
  let alg_name = key_obj
    .algorithm_name(scope)
    .ok_or_else(|| CryptoError::Other(JsErrorBox::type_error("Missing algorithm.name")))?;
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
        type_error(format!(
          "Cannot export {name} private key without a seed"
        ))
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
  res: Result<crate::export_key::ExportKeyResult, crate::export_key::ExportKeyError>,
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
  let format = if format == KeyFormat::Raw && algorithm.name == "ChaCha20-Poly1305"
  {
    KeyFormat::RawSecret
  } else {
    format
  };
  let data = ImportKeyData::from_v8(scope, key_data, format)?;
  run_import_key(scope, format, &algorithm, data, extractable, &usages)
}
