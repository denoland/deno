// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::JsBuffer;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use rsa::RsaPrivateKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::EncodeRsaPublicKey;
use serde::Deserialize;
use serde::Serialize;

pub const RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");

pub const ID_SECP256R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
pub const ID_SECP384R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.34");
pub const ID_SECP521R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.35");

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum ShaHash {
  #[serde(rename = "SHA-1")]
  Sha1,
  #[serde(rename = "SHA-256")]
  Sha256,
  #[serde(rename = "SHA-384")]
  Sha384,
  #[serde(rename = "SHA-512")]
  Sha512,
  #[serde(rename = "SHA3-256")]
  Sha3_256,
  #[serde(rename = "SHA3-384")]
  Sha3_384,
  #[serde(rename = "SHA3-512")]
  Sha3_512,
}

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum EcNamedCurve {
  #[serde(rename = "P-256")]
  P256,
  #[serde(rename = "P-384")]
  P384,
  #[serde(rename = "P-521")]
  P521,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "data")]
pub enum V8RawKeyData {
  Secret(JsBuffer),
  Private(JsBuffer),
  Public(JsBuffer),
}

#[derive(ToV8)]
#[to_v8(tag = "type", content = "data")]
pub enum RustRawKeyData {
  Secret(Uint8Array),
  Private(Uint8Array),
  Public(Uint8Array),
}

/// Owned WebCrypto key material, wrapped by a V8 garbage-collected handle
/// ([`crate::key_store::CryptoKeyHandle`]).
///
/// This mirrors [`V8RawKeyData`], but owns its bytes (rather than borrowing a
/// `JsBuffer`) so the key material can live in Rust - held alive by the cppgc
/// handle that JavaScript stores on the `CryptoKey` - instead of being
/// serialized back to JavaScript and passed to every operation.
///
/// The `Raw` variant is key material that is stored verbatim (e.g.
/// Ed25519/X25519/X448/ML-KEM public key bytes) and carries no `secret`/
/// `private`/`public` tag. `SeededPrivate` holds the composite
/// `{ seed, private_key }` material used by the FIPS 203/204 algorithms
/// (ML-KEM decapsulation keys and ML-DSA signing keys), where `private_key`
/// is the expanded key bytes and `seed` is the short seed used to derive them.
#[derive(Debug, Clone)]
pub enum RawKeyData {
  Secret(Box<[u8]>),
  Private(Box<[u8]>),
  Public(Box<[u8]>),
  Raw(Box<[u8]>),
  /// `seed` is `None` for keys imported from expanded raw private bytes (which
  /// carry no seed); exporting the `raw-seed`/`jwk`/`pkcs8` formats then
  /// correctly rejects.
  SeededPrivate {
    seed: Option<Box<[u8]>>,
    private_key: Box<[u8]>,
  },
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
  Secret,
  Private,
  Public,
  Raw,
  Seeded,
}

/// Wire representation used by the key store insert op. JavaScript passes a
/// `{ kind, data }` object (or `{ kind: "seeded", seed, privateKey }` for the
/// composite ML-KEM/ML-DSA private key material).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertKeyData {
  kind: KeyKind,
  data: Option<JsBuffer>,
  seed: Option<JsBuffer>,
  private_key: Option<JsBuffer>,
}

/// Wire representation used by the key store get op, returned to JavaScript for
/// export, structured clone and node:crypto interop.
#[derive(ToV8)]
pub struct StoredKeyData {
  kind: &'static str,
  data: Option<Uint8Array>,
  seed: Option<Uint8Array>,
  private_key: Option<Uint8Array>,
}

fn into_boxed(buf: Option<JsBuffer>) -> Box<[u8]> {
  buf.map(|b| b.as_ref().into()).unwrap_or_default()
}

impl From<InsertKeyData> for RawKeyData {
  fn from(data: InsertKeyData) -> Self {
    match data.kind {
      KeyKind::Secret => RawKeyData::Secret(into_boxed(data.data)),
      KeyKind::Private => RawKeyData::Private(into_boxed(data.data)),
      KeyKind::Public => RawKeyData::Public(into_boxed(data.data)),
      KeyKind::Raw => RawKeyData::Raw(into_boxed(data.data)),
      KeyKind::Seeded => RawKeyData::SeededPrivate {
        seed: data.seed.map(|b| b.as_ref().into()),
        private_key: into_boxed(data.private_key),
      },
    }
  }
}

impl RawKeyData {
  pub fn to_stored_key_data(&self) -> StoredKeyData {
    let tagged = |kind, b: &[u8]| StoredKeyData {
      kind,
      data: Some(b.to_vec().into()),
      seed: None,
      private_key: None,
    };
    match self {
      RawKeyData::Secret(b) => tagged("secret", b),
      RawKeyData::Private(b) => tagged("private", b),
      RawKeyData::Public(b) => tagged("public", b),
      RawKeyData::Raw(b) => tagged("raw", b),
      RawKeyData::SeededPrivate { seed, private_key } => StoredKeyData {
        kind: "seeded",
        data: None,
        seed: seed.as_ref().map(|s| s.as_ref().to_vec().into()),
        private_key: Some(private_key.as_ref().to_vec().into()),
      },
    }
  }

  /// The raw key bytes, regardless of the secret/private/public/raw tag.
  ///
  /// Not valid for `SeededPrivate`; use [`Self::expanded_private_key`] /
  /// [`Self::seed`] for those.
  pub fn bytes(&self) -> &[u8] {
    match self {
      RawKeyData::Secret(b)
      | RawKeyData::Private(b)
      | RawKeyData::Public(b)
      | RawKeyData::Raw(b) => b,
      RawKeyData::SeededPrivate { .. } => unreachable!(),
    }
  }

  /// The expanded private key bytes of a `SeededPrivate` (ML-KEM/ML-DSA).
  pub fn expanded_private_key(&self) -> &[u8] {
    match self {
      RawKeyData::SeededPrivate { private_key, .. } => private_key,
      _ => unreachable!(),
    }
  }

  /// The seed of a `SeededPrivate`, or `None` for keys imported from expanded
  /// raw private bytes.
  pub fn seed(&self) -> Option<&[u8]> {
    match self {
      RawKeyData::SeededPrivate { seed, .. } => seed.as_deref(),
      _ => None,
    }
  }

  pub fn as_rsa_public_key(&self) -> Result<Cow<'_, [u8]>, SharedError> {
    match self {
      RawKeyData::Public(data) => Ok(Cow::Borrowed(data)),
      RawKeyData::Private(data) => {
        let private_key = RsaPrivateKey::from_pkcs1_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateKey)?;

        let public_key_doc = private_key
          .to_public_key()
          .to_pkcs1_der()
          .map_err(|_| SharedError::ExpectedValidPublicKey)?;

        Ok(Cow::Owned(public_key_doc.as_bytes().into()))
      }
      _ => Err(SharedError::ExpectedPublicKey),
    }
  }

  pub fn as_rsa_private_key(&self) -> Result<&[u8], SharedError> {
    match self {
      RawKeyData::Private(data) => Ok(data),
      _ => Err(SharedError::ExpectedPrivateKey),
    }
  }

  pub fn as_secret_key(&self) -> Result<&[u8], SharedError> {
    match self {
      RawKeyData::Secret(data) => Ok(data),
      _ => Err(SharedError::ExpectedSecretKey),
    }
  }

  pub fn as_ec_public_key_p256(
    &self,
  ) -> Result<p256::EncodedPoint, SharedError> {
    match self {
      RawKeyData::Public(data) => p256::PublicKey::from_sec1_bytes(data)
        .map(|p| p.to_encoded_point(false))
        .map_err(|_| SharedError::ExpectedValidPublicECKey),
      RawKeyData::Private(data) => {
        let signing_key = p256::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      _ => unreachable!(),
    }
  }

  pub fn as_ec_public_key_p384(
    &self,
  ) -> Result<p384::EncodedPoint, SharedError> {
    match self {
      RawKeyData::Public(data) => p384::PublicKey::from_sec1_bytes(data)
        .map(|p| p.to_encoded_point(false))
        .map_err(|_| SharedError::ExpectedValidPublicECKey),
      RawKeyData::Private(data) => {
        let signing_key = p384::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      _ => unreachable!(),
    }
  }

  pub fn as_ec_public_key_p521(
    &self,
  ) -> Result<p521::EncodedPoint, SharedError> {
    match self {
      RawKeyData::Public(data) => p521::PublicKey::from_sec1_bytes(data)
        .map(|p| p.to_encoded_point(false))
        .map_err(|_| SharedError::ExpectedValidPublicECKey),
      RawKeyData::Private(data) => {
        let signing_key = p521::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      _ => unreachable!(),
    }
  }

  pub fn as_ec_private_key(&self) -> Result<&[u8], SharedError> {
    match self {
      RawKeyData::Private(data) => Ok(data),
      _ => Err(SharedError::ExpectedPrivateKey),
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SharedError {
  #[class(type)]
  #[error("expected valid private key")]
  ExpectedValidPrivateKey,
  #[class(type)]
  #[error("expected valid public key")]
  ExpectedValidPublicKey,
  #[class(type)]
  #[error("expected valid private EC key")]
  ExpectedValidPrivateECKey,
  #[class(type)]
  #[error("expected valid public EC key")]
  ExpectedValidPublicECKey,
  #[class(type)]
  #[error("expected private key")]
  ExpectedPrivateKey,
  #[class(type)]
  #[error("expected public key")]
  ExpectedPublicKey,
  #[class(type)]
  #[error("expected secret key")]
  ExpectedSecretKey,
  #[class("DOMExceptionOperationError")]
  #[error("failed to decode private key")]
  FailedDecodePrivateKey,
  #[class("DOMExceptionOperationError")]
  #[error("failed to decode public key")]
  FailedDecodePublicKey,
  #[class("DOMExceptionNotSupportedError")]
  #[error("unsupported format")]
  UnsupportedFormat,
  #[class(type)]
  #[error("invalid crypto key handle")]
  InvalidKeyHandle,
  #[class(type)]
  #[error("Illegal constructor")]
  #[property("code" = "ERR_ILLEGAL_CONSTRUCTOR")]
  IllegalConstructor,
  #[class(type)]
  #[error("invalid CryptoKey type")]
  InvalidKeyType,
}

#[allow(
  dead_code,
  reason = "V8RawKeyData accessors fronted the legacy op_crypto_sign/verify path; \
            kept for completeness while the per-op key-handling helpers are \
            still being merged into the cppgc impl"
)]
impl V8RawKeyData {
  pub fn as_rsa_public_key(&self) -> Result<Cow<'_, [u8]>, SharedError> {
    match self {
      V8RawKeyData::Public(data) => Ok(Cow::Borrowed(data)),
      V8RawKeyData::Private(data) => {
        let private_key = RsaPrivateKey::from_pkcs1_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateKey)?;

        let public_key_doc = private_key
          .to_public_key()
          .to_pkcs1_der()
          .map_err(|_| SharedError::ExpectedValidPublicKey)?;

        Ok(Cow::Owned(public_key_doc.as_bytes().into()))
      }
      _ => Err(SharedError::ExpectedPublicKey),
    }
  }

  pub fn as_rsa_private_key(&self) -> Result<&[u8], SharedError> {
    match self {
      V8RawKeyData::Private(data) => Ok(data),
      _ => Err(SharedError::ExpectedPrivateKey),
    }
  }

  pub fn as_secret_key(&self) -> Result<&[u8], SharedError> {
    match self {
      V8RawKeyData::Secret(data) => Ok(data),
      _ => Err(SharedError::ExpectedSecretKey),
    }
  }

  pub fn as_ec_public_key_p256(
    &self,
  ) -> Result<p256::EncodedPoint, SharedError> {
    match self {
      V8RawKeyData::Public(data) => p256::PublicKey::from_sec1_bytes(data)
        .map(|p| p.to_encoded_point(false))
        .map_err(|_| SharedError::ExpectedValidPublicECKey),
      V8RawKeyData::Private(data) => {
        let signing_key = p256::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      // Should never reach here.
      V8RawKeyData::Secret(_) => unreachable!(),
    }
  }

  pub fn as_ec_public_key_p384(
    &self,
  ) -> Result<p384::EncodedPoint, SharedError> {
    match self {
      V8RawKeyData::Public(data) => p384::PublicKey::from_sec1_bytes(data)
        .map(|p| p.to_encoded_point(false))
        .map_err(|_| SharedError::ExpectedValidPublicECKey),
      V8RawKeyData::Private(data) => {
        let signing_key = p384::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      // Should never reach here.
      V8RawKeyData::Secret(_) => unreachable!(),
    }
  }

  pub fn as_ec_public_key_p521(
    &self,
  ) -> Result<p521::EncodedPoint, SharedError> {
    match self {
      V8RawKeyData::Public(data) => {
        // public_key is a serialized EncodedPoint
        p521::PublicKey::from_sec1_bytes(data)
          .map(|p| p.to_encoded_point(false))
          .map_err(|_| SharedError::ExpectedValidPublicECKey)
      }
      V8RawKeyData::Private(data) => {
        let signing_key = p521::SecretKey::from_pkcs8_der(data)
          .map_err(|_| SharedError::ExpectedValidPrivateECKey)?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      // Should never reach here.
      V8RawKeyData::Secret(_) => unreachable!(),
    }
  }

  pub fn as_ec_private_key(&self) -> Result<&[u8], SharedError> {
    match self {
      V8RawKeyData::Private(data) => Ok(data),
      _ => Err(SharedError::ExpectedPrivateKey),
    }
  }
}
