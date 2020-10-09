// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use digest::{Digest, DynDigest};
use hmac::{Hmac, Mac, NewMac};
use sha2::{Sha256, Sha512};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct DenoHash {
  inner: Box<dyn DynDigest>,
}

#[wasm_bindgen]
pub fn create_hash(algorithm: &str) -> Result<DenoHash, JsValue> {
  let hash: Option<Box<dyn DynDigest>> = match algorithm {
    "md2" => Some(Box::new(md2::Md2::new())),
    "md4" => Some(Box::new(md4::Md4::new())),
    "md5" => Some(Box::new(md5::Md5::new())),
    "ripemd160" => Some(Box::new(ripemd160::Ripemd160::new())),
    "ripemd320" => Some(Box::new(ripemd320::Ripemd320::new())),
    "sha1" => Some(Box::new(sha1::Sha1::new())),
    "sha224" => Some(Box::new(sha2::Sha224::new())),
    "sha256" => Some(Box::new(sha2::Sha256::new())),
    "sha384" => Some(Box::new(sha2::Sha384::new())),
    "sha512" => Some(Box::new(sha2::Sha512::new())),
    "sha3-224" => Some(Box::new(sha3::Sha3_224::new())),
    "sha3-256" => Some(Box::new(sha3::Sha3_256::new())),
    "sha3-384" => Some(Box::new(sha3::Sha3_384::new())),
    "sha3-512" => Some(Box::new(sha3::Sha3_512::new())),
    "keccak224" => Some(Box::new(sha3::Keccak224::new())),
    "keccak256" => Some(Box::new(sha3::Keccak256::new())),
    "keccak384" => Some(Box::new(sha3::Keccak384::new())),
    "keccak512" => Some(Box::new(sha3::Keccak512::new())),
    _ => None,
  };

  if let Some(h) = hash {
    Ok(DenoHash { inner: h })
  } else {
    let err_msg = format!("unsupported hash algorithm: {}", algorithm);
    Err(JsValue::from_str(&err_msg))
  }
}

#[wasm_bindgen]
pub fn update_hash(hash: &mut DenoHash, data: &[u8]) {
  hash.inner.update(data)
}

#[wasm_bindgen]
pub fn digest_hash(hash: &mut DenoHash) -> Box<[u8]> {
  hash.inner.finalize_reset()
}

#[wasm_bindgen]
pub struct HmacSha256Hash {
  inner: Hmac<Sha256>,
}

#[wasm_bindgen]
impl HmacSha256Hash {
  #[wasm_bindgen(constructor)]
  pub fn new(secret: &str) -> Result<HmacSha256Hash, JsValue> {
    let hash = Hmac::<Sha256>::new_varkey(&secret.to_string().into_bytes());
    if let Ok(h) = hash {
      Ok(HmacSha256Hash { inner: h })
    } else {
      Err(JsValue::from_str("Invalid key length"))
    }
  }

  #[wasm_bindgen]
  pub fn update(&mut self, key: &str) {
    self.inner.update(&key.to_string().into_bytes())
  }

  #[wasm_bindgen]
  pub fn digest(&mut self) -> Box<[u8]> {
    self
      .inner
      .finalize_reset()
      .into_bytes()
      .as_slice()
      .to_vec()
      .into_boxed_slice()
  }
}

#[wasm_bindgen]
pub struct HmacSha512Hash {
  inner: Hmac<Sha512>,
}

#[wasm_bindgen]
impl HmacSha512Hash {
  #[wasm_bindgen(constructor)]
  pub fn new(secret: &str) -> Result<HmacSha512Hash, JsValue> {
    let hash = Hmac::<Sha512>::new_varkey(&secret.to_string().into_bytes());
    if let Ok(h) = hash {
      Ok(HmacSha512Hash { inner: h })
    } else {
      Err(JsValue::from_str("Invalid key length"))
    }
  }

  #[wasm_bindgen]
  pub fn update(&mut self, key: &str) {
    self.inner.update(&key.to_string().into_bytes())
  }

  #[wasm_bindgen]
  pub fn digest(&mut self) -> Box<[u8]> {
    self
      .inner
      .finalize_reset()
      .into_bytes()
      .as_slice()
      .to_vec()
      .into_boxed_slice()
  }
}
