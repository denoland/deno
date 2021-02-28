use ring::agreement::Algorithm as RingAlgorithm;
use ring::signature::EcdsaSigningAlgorithm;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyType {
  Public,
  Private,
  Secret,
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub enum WebCryptoNamedCurve {
  #[serde(rename = "P-256")]
  P256,
  #[serde(rename = "P-384")]
  P384,
  #[serde(rename = "P-512")]
  P521,
}

impl Into<&RingAlgorithm> for WebCryptoNamedCurve {
  fn into(self) -> &'static RingAlgorithm {
    match self {
      WebCryptoNamedCurve::P256 => &ring::agreement::ECDH_P256,
      WebCryptoNamedCurve::P384 => &ring::agreement::ECDH_P384,
      // XXX: Not implemented.
      WebCryptoNamedCurve::P521 => panic!(),
    }
  }
}

impl Into<&EcdsaSigningAlgorithm> for WebCryptoNamedCurve {
  fn into(self) -> &'static EcdsaSigningAlgorithm {
    match self {
      WebCryptoNamedCurve::P256 => {
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING
      }
      WebCryptoNamedCurve::P384 => {
        &ring::signature::ECDSA_P384_SHA384_FIXED_SIGNING
      }
      // TODO: Not implemented but don't panic.
      WebCryptoNamedCurve::P521 => panic!(),
    }
  }
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
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
    extractable: bool,
    algorithm: Algorithm,
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

pub struct CryptoKeyPair<A, B> {
  pub public_key: A,
  pub private_key: B,
}

pub type WebCryptoKeyPair = CryptoKeyPair<WebCryptoKey, WebCryptoKey>;
