// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use digest::{Digest, DynDigest};
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
  hash.inner.input(data)
}

#[wasm_bindgen]
pub fn digest_hash(hash: &mut DenoHash) -> Box<[u8]> {
  hash.inner.result_reset()
}
