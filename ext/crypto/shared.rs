use std::borrow::Cow;

use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::ZeroCopyBuf;
use elliptic_curve::sec1::ToEncodedPoint;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::EncodeRsaPublicKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::RsaPrivateKey;
use serde::Deserialize;
use serde::Serialize;

pub const RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
pub const SHA1_RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.5");
pub const SHA256_RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11");
pub const SHA384_RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.12");
pub const SHA512_RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.13");
pub const RSASSA_PSS_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.10");
pub const ID_SHA1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.14.3.2.26");
pub const ID_SHA256_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");
pub const ID_SHA384_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2");
pub const ID_SHA512_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3");
pub const ID_MFG1: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.8");
pub const RSAES_OAEP_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.7");
pub const ID_P_SPECIFIED: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.9");

pub const ID_SECP256R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
pub const ID_SECP384R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.34");
pub const ID_SECP521R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.35");

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum ShaHash {
  #[serde(rename = "SHA-1")]
  Sha1,
  #[serde(rename = "SHA-256")]
  Sha256,
  #[serde(rename = "SHA-384")]
  Sha384,
  #[serde(rename = "SHA-512")]
  Sha512,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum EcNamedCurve {
  #[serde(rename = "P-256")]
  P256,
  #[serde(rename = "P-384")]
  P384,
  #[serde(rename = "P-521")]
  P521,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "data")]
pub enum RawKeyData {
  Secret(ZeroCopyBuf),
  Private(ZeroCopyBuf),
  Public(ZeroCopyBuf),
}

impl RawKeyData {
  pub fn as_rsa_public_key(&self) -> Result<Cow<'_, [u8]>, AnyError> {
    match self {
      RawKeyData::Public(data) => Ok(Cow::Borrowed(data)),
      RawKeyData::Private(data) => {
        let private_key = RsaPrivateKey::from_pkcs1_der(data)
          .map_err(|_| type_error("expected valid private key"))?;

        let public_key_doc = private_key
          .to_public_key()
          .to_pkcs1_der()
          .map_err(|_| type_error("expected valid public key"))?;

        Ok(Cow::Owned(public_key_doc.as_bytes().into()))
      }
      _ => Err(type_error("expected public key")),
    }
  }

  pub fn as_rsa_private_key(&self) -> Result<&[u8], AnyError> {
    match self {
      RawKeyData::Private(data) => Ok(data),
      _ => Err(type_error("expected private key")),
    }
  }

  pub fn as_secret_key(&self) -> Result<&[u8], AnyError> {
    match self {
      RawKeyData::Secret(data) => Ok(data),
      _ => Err(type_error("expected secret key")),
    }
  }

  pub fn as_ec_public_key_p256(&self) -> Result<p256::EncodedPoint, AnyError> {
    match self {
      RawKeyData::Public(data) => {
        // public_key is a serialized EncodedPoint
        p256::EncodedPoint::from_bytes(&data)
          .map_err(|_| type_error("expected valid public EC key"))
      }
      RawKeyData::Private(data) => {
        let signing_key = p256::SecretKey::from_pkcs8_der(data)
          .map_err(|_| type_error("expected valid private EC key"))?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      // Should never reach here.
      RawKeyData::Secret(_) => unreachable!(),
    }
  }

  pub fn as_ec_public_key_p384(&self) -> Result<p384::EncodedPoint, AnyError> {
    match self {
      RawKeyData::Public(data) => {
        // public_key is a serialized EncodedPoint
        p384::EncodedPoint::from_bytes(&data)
          .map_err(|_| type_error("expected valid public EC key"))
      }
      RawKeyData::Private(data) => {
        let signing_key = p384::SecretKey::from_pkcs8_der(data)
          .map_err(|_| type_error("expected valid private EC key"))?;
        Ok(signing_key.public_key().to_encoded_point(false))
      }
      // Should never reach here.
      RawKeyData::Secret(_) => unreachable!(),
    }
  }

  pub fn as_ec_private_key(&self) -> Result<&[u8], AnyError> {
    match self {
      RawKeyData::Private(data) => Ok(data),
      _ => Err(type_error("expected private key")),
    }
  }
}

pub fn data_error(msg: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("DOMExceptionDataError", msg)
}

pub fn not_supported_error(msg: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("DOMExceptionNotSupportedError", msg)
}

pub fn operation_error(msg: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("DOMExceptionOperationError", msg)
}

pub fn unsupported_format() -> AnyError {
  not_supported_error("unsupported format")
}
