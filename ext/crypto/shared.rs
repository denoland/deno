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

/// Owned key material held by the Rust-side key store
/// ([`crate::key_store::KeyStore`]).
///
/// This mirrors [`V8RawKeyData`], but owns its bytes (rather than borrowing a
/// `JsBuffer`) so the key material can live in Rust across op calls instead of
/// being serialized back to JavaScript and passed to every operation.
///
/// The `Raw` variant is used for key material that is stored verbatim (e.g.
/// Ed25519/X25519/X448/ML-DSA/ML-KEM raw key bytes) and carries no `secret`/
/// `private`/`public` tag.
#[derive(Debug)]
pub enum RawKeyData {
  Secret(Box<[u8]>),
  Private(Box<[u8]>),
  Public(Box<[u8]>),
  Raw(Box<[u8]>),
}

/// Wire representation used by the key store insert op. Deserialized from the
/// `{ type, data }` shape that JavaScript already uses for key material, plus a
/// `raw` tag for untyped key bytes.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "data")]
pub enum InsertKeyData {
  Secret(JsBuffer),
  Private(JsBuffer),
  Public(JsBuffer),
  Raw(JsBuffer),
}

/// Wire representation used by the key store get op, returned to JavaScript for
/// export, structured clone and node:crypto interop.
#[derive(ToV8)]
#[to_v8(tag = "type", content = "data")]
pub enum StoredKeyData {
  Secret(Uint8Array),
  Private(Uint8Array),
  Public(Uint8Array),
  Raw(Uint8Array),
}

impl From<InsertKeyData> for RawKeyData {
  fn from(data: InsertKeyData) -> Self {
    match data {
      InsertKeyData::Secret(b) => RawKeyData::Secret(b.as_ref().into()),
      InsertKeyData::Private(b) => RawKeyData::Private(b.as_ref().into()),
      InsertKeyData::Public(b) => RawKeyData::Public(b.as_ref().into()),
      InsertKeyData::Raw(b) => RawKeyData::Raw(b.as_ref().into()),
    }
  }
}

impl RawKeyData {
  pub fn to_stored_key_data(&self) -> StoredKeyData {
    match self {
      RawKeyData::Secret(b) => {
        StoredKeyData::Secret(b.as_ref().to_vec().into())
      }
      RawKeyData::Private(b) => {
        StoredKeyData::Private(b.as_ref().to_vec().into())
      }
      RawKeyData::Public(b) => {
        StoredKeyData::Public(b.as_ref().to_vec().into())
      }
      RawKeyData::Raw(b) => StoredKeyData::Raw(b.as_ref().to_vec().into()),
    }
  }

  /// The raw key bytes, regardless of the secret/private/public/raw tag.
  pub fn bytes(&self) -> &[u8] {
    match self {
      RawKeyData::Secret(b)
      | RawKeyData::Private(b)
      | RawKeyData::Public(b)
      | RawKeyData::Raw(b) => b,
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
}

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
