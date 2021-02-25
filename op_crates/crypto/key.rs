pub enum KeyType {
  Public,
  Private,
  Secret,
}

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

#[derive(Clone)]
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
