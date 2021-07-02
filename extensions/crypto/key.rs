// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
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
pub enum CryptoHash {
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
pub enum CryptoNamedCurve {
  #[serde(rename = "P-256")]
  P256,
  #[serde(rename = "P-384")]
  P384,
}

impl TryInto<&RingAlgorithm> for CryptoNamedCurve {
  type Error = AnyError;

  fn try_into(self) -> Result<&'static RingAlgorithm, Self::Error> {
    match self {
      CryptoNamedCurve::P256 => Ok(&ring::agreement::ECDH_P256),
      CryptoNamedCurve::P384 => Ok(&ring::agreement::ECDH_P384),
    }
  }
}

impl TryInto<&EcdsaSigningAlgorithm> for CryptoNamedCurve {
  type Error = AnyError;

  fn try_into(self) -> Result<&'static EcdsaSigningAlgorithm, Self::Error> {
    match self {
      CryptoNamedCurve::P256 => {
        Ok(&ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING)
      }
      CryptoNamedCurve::P384 => {
        Ok(&ring::signature::ECDSA_P384_SHA384_FIXED_SIGNING)
      }
    }
  }
}

impl From<CryptoHash> for HmacAlgorithm {
  fn from(hash: CryptoHash) -> HmacAlgorithm {
    match hash {
      CryptoHash::Sha1 => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
      CryptoHash::Sha256 => ring::hmac::HMAC_SHA256,
      CryptoHash::Sha384 => ring::hmac::HMAC_SHA384,
      CryptoHash::Sha512 => ring::hmac::HMAC_SHA512,
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
  #[serde(rename = "AES-KW")]
  AesKw,
  #[serde(rename = "HMAC")]
  Hmac,
}
