use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub enum KeyType {
  Public,
  Private,
  Secret,
}

#[derive(Serialize, Deserialize)]
pub enum WebCryptoHash {
  Sha1,
  Sha256,
  Sha384,
  Sha512,
}

#[derive(Serialize, Deserialize)]
pub enum WebCryptoNamedCurve {
  P256,
  P384,
  P521,
}

#[derive(Serialize, Deserialize)]
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
  RsassaPkcs1v15,
  RsaPss,
  RsaOaep,
  Ecdsa,
  Ecdh,
  AesCtr,
  AesCbc,
  AesGcm,
  AesKw,
  Hmac,
}

#[derive(Serialize, Deserialize)]
pub struct WebCryptoKey {
  pub key_type: KeyType,
  pub extractable: bool,
  pub algorithm: Algorithm,
  pub usages: Vec<KeyUsage>,
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
