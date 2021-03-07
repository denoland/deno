// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::WebCryptoError;
use ring::agreement::Algorithm as RingAlgorithm;
use ring::hmac::Algorithm as HmacAlgorithm;
use ring::signature::EcdsaSigningAlgorithm;
use serde::Deserialize;
use serde::Serialize;
use std::convert::TryInto;

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum KeyType {
  Public,
  Private,
  Secret,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum WebCryptoHash {
  #[serde(rename = "SHA-1")]
  Sha1,
  #[serde(rename = "SHA-256")]
  Sha256,
  #[serde(rename = "SHA-384")]
  Sha384,
  #[serde(rename = "SHA-512")]
  Sha512,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum WebCryptoNamedCurve {
  #[serde(rename = "P-256")]
  P256,
  #[serde(rename = "P-384")]
  P384,
  #[serde(rename = "P-512")]
  P521,
}

impl TryInto<&RingAlgorithm> for WebCryptoNamedCurve {
  type Error = WebCryptoError;

  fn try_into(self) -> Result<&'static RingAlgorithm, Self::Error> {
    match self {
      WebCryptoNamedCurve::P256 => Ok(&ring::agreement::ECDH_P256),
      WebCryptoNamedCurve::P384 => Ok(&ring::agreement::ECDH_P384),
      WebCryptoNamedCurve::P521 => Err(WebCryptoError::Unsupported),
    }
  }
}

impl TryInto<&EcdsaSigningAlgorithm> for WebCryptoNamedCurve {
  type Error = WebCryptoError;

  fn try_into(self) -> Result<&'static EcdsaSigningAlgorithm, Self::Error> {
    match self {
      WebCryptoNamedCurve::P256 => {
        Ok(&ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING)
      }
      WebCryptoNamedCurve::P384 => {
        Ok(&ring::signature::ECDSA_P384_SHA384_FIXED_SIGNING)
      }
      WebCryptoNamedCurve::P521 => Err(WebCryptoError::Unsupported),
    }
  }
}

impl Into<HmacAlgorithm> for WebCryptoHash {
  fn into(self) -> HmacAlgorithm {
    match self {
      WebCryptoHash::Sha1 => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
      WebCryptoHash::Sha256 => ring::hmac::HMAC_SHA256,
      WebCryptoHash::Sha384 => ring::hmac::HMAC_SHA384,
      WebCryptoHash::Sha512 => ring::hmac::HMAC_SHA512,
    }
  }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum KeyUsage {
  Encrypt,
  Decrypt,
  Sign,
  Verify,
  DeriveKey,
  DeriveBits,
  WrapKey,
  UnwrapKey,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum Algorithm {
  #[serde(rename = "RSASSA-PKCS1-v1_5")]
  RsassaPkcs1v15,
  #[serde(rename = "RSA-PSS")]
  RsaPss,
  #[serde(rename = "RSA-OAEP")]
  RsaOaep,
  #[serde(rename = "ECDSA")]
  Ecdsa,
  #[serde(rename = "ECDH")]
  Ecdh,
  #[serde(rename = "AES-CTR")]
  AesCtr,
  #[serde(rename = "AES-CBC")]
  AesCbc,
  #[serde(rename = "AES-GCM")]
  AesGcm,
  #[serde(rename = "RSA-PSS")]
  AesKw,
  #[serde(rename = "HMAC")]
  Hmac,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WebCryptoKey {
  pub key_type: KeyType,
  pub extractable: bool,
  pub algorithm: Algorithm,
  pub usages: Vec<KeyUsage>,
}

impl WebCryptoKey {
  pub fn new_private(
    algorithm: Algorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
  ) -> Self {
    Self {
      key_type: KeyType::Private,
      extractable,
      algorithm,
      usages,
    }
  }

  pub fn new_public(
    algorithm: Algorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
  ) -> Self {
    Self {
      key_type: KeyType::Public,
      extractable,
      algorithm,
      usages,
    }
  }

  pub fn new_secret(
    algorithm: Algorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
  ) -> Self {
    Self {
      key_type: KeyType::Secret,
      extractable,
      algorithm,
      usages,
    }
  }
}

impl WebCryptoKeyPair {
  pub fn new(public_key: WebCryptoKey, private_key: WebCryptoKey) -> Self {
    Self {
      public_key,
      private_key,
    }
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WebCryptoKeyPair {
  pub public_key: WebCryptoKey,
  pub private_key: WebCryptoKey,
}
