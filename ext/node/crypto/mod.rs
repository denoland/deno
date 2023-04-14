// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use hkdf::Hkdf;
use num_bigint::BigInt;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::Rng;
use std::future::Future;
use std::rc::Rc;

use rsa::padding::PaddingScheme;
use rsa::pkcs8::DecodePrivateKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::PublicKey;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;

mod cipher;
mod digest;
mod primes;
pub mod x509;

#[op]
pub fn op_node_check_prime(num: serde_v8::BigInt, checks: usize) -> bool {
  primes::is_probably_prime(&num, checks)
}

#[op]
pub fn op_node_check_prime_bytes(
  bytes: &[u8],
  checks: usize,
) -> Result<bool, AnyError> {
  let candidate = BigInt::from_bytes_be(num_bigint::Sign::Plus, bytes);
  Ok(primes::is_probably_prime(&candidate, checks))
}

#[op]
pub async fn op_node_check_prime_async(
  num: serde_v8::BigInt,
  checks: usize,
) -> Result<bool, AnyError> {
  // TODO(@littledivy): use rayon for CPU-bound tasks
  Ok(
    tokio::task::spawn_blocking(move || {
      primes::is_probably_prime(&num, checks)
    })
    .await?,
  )
}

#[op]
pub fn op_node_check_prime_bytes_async(
  bytes: &[u8],
  checks: usize,
) -> Result<impl Future<Output = Result<bool, AnyError>> + 'static, AnyError> {
  let candidate = BigInt::from_bytes_be(num_bigint::Sign::Plus, bytes);
  // TODO(@littledivy): use rayon for CPU-bound tasks
  Ok(async move {
    Ok(
      tokio::task::spawn_blocking(move || {
        primes::is_probably_prime(&candidate, checks)
      })
      .await?,
    )
  })
}

#[op(fast)]
pub fn op_node_create_hash(state: &mut OpState, algorithm: &str) -> u32 {
  state
    .resource_table
    .add(match digest::Context::new(algorithm) {
      Ok(context) => context,
      Err(_) => return 0,
    })
}

#[op(fast)]
pub fn op_node_hash_update(state: &mut OpState, rid: u32, data: &[u8]) -> bool {
  let context = match state.resource_table.get::<digest::Context>(rid) {
    Ok(context) => context,
    _ => return false,
  };
  context.update(data);
  true
}

#[op(fast)]
pub fn op_node_hash_update_str(
  state: &mut OpState,
  rid: u32,
  data: &str,
) -> bool {
  let context = match state.resource_table.get::<digest::Context>(rid) {
    Ok(context) => context,
    _ => return false,
  };
  context.update(data.as_bytes());
  true
}

#[op]
pub fn op_node_hash_digest(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ZeroCopyBuf, AnyError> {
  let context = state.resource_table.take::<digest::Context>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Hash context is already in use"))?;
  Ok(context.digest()?.into())
}

#[op]
pub fn op_node_hash_digest_hex(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<String, AnyError> {
  let context = state.resource_table.take::<digest::Context>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Hash context is already in use"))?;
  let digest = context.digest()?;
  Ok(hex::encode(digest))
}

#[op]
pub fn op_node_hash_clone(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  let context = state.resource_table.get::<digest::Context>(rid)?;
  Ok(state.resource_table.add(context.as_ref().clone()))
}

#[op]
pub fn op_node_private_encrypt(
  key: StringOrBuffer,
  msg: StringOrBuffer,
  padding: u32,
) -> Result<ZeroCopyBuf, AnyError> {
  let key = RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)?;

  let mut rng = rand::thread_rng();
  match padding {
    1 => Ok(
      key
        .encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(), &msg)?
        .into(),
    ),
    4 => Ok(
      key
        .encrypt(&mut rng, PaddingScheme::new_oaep::<sha1::Sha1>(), &msg)?
        .into(),
    ),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op]
pub fn op_node_private_decrypt(
  key: StringOrBuffer,
  msg: StringOrBuffer,
  padding: u32,
) -> Result<ZeroCopyBuf, AnyError> {
  let key = RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)?;

  match padding {
    1 => Ok(
      key
        .decrypt(PaddingScheme::new_pkcs1v15_encrypt(), &msg)?
        .into(),
    ),
    4 => Ok(
      key
        .decrypt(PaddingScheme::new_oaep::<sha1::Sha1>(), &msg)?
        .into(),
    ),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op]
pub fn op_node_public_encrypt(
  key: StringOrBuffer,
  msg: StringOrBuffer,
  padding: u32,
) -> Result<ZeroCopyBuf, AnyError> {
  let key = RsaPublicKey::from_public_key_pem((&key).try_into()?)?;

  let mut rng = rand::thread_rng();
  match padding {
    1 => Ok(
      key
        .encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(), &msg)?
        .into(),
    ),
    4 => Ok(
      key
        .encrypt(&mut rng, PaddingScheme::new_oaep::<sha1::Sha1>(), &msg)?
        .into(),
    ),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op(fast)]
pub fn op_node_create_cipheriv(
  state: &mut OpState,
  algorithm: &str,
  key: &[u8],
  iv: &[u8],
) -> u32 {
  state.resource_table.add(
    match cipher::CipherContext::new(algorithm, key, iv) {
      Ok(context) => context,
      Err(_) => return 0,
    },
  )
}

#[op(fast)]
pub fn op_node_cipheriv_encrypt(
  state: &mut OpState,
  rid: u32,
  input: &[u8],
  output: &mut [u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::CipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.encrypt(input, output);
  true
}

#[op]
pub fn op_node_cipheriv_final(
  state: &mut OpState,
  rid: u32,
  input: &[u8],
  output: &mut [u8],
) -> Result<(), AnyError> {
  let context = state.resource_table.take::<cipher::CipherContext>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  context.r#final(input, output)
}

#[op(fast)]
pub fn op_node_create_decipheriv(
  state: &mut OpState,
  algorithm: &str,
  key: &[u8],
  iv: &[u8],
) -> u32 {
  state.resource_table.add(
    match cipher::DecipherContext::new(algorithm, key, iv) {
      Ok(context) => context,
      Err(_) => return 0,
    },
  )
}

#[op(fast)]
pub fn op_node_decipheriv_decrypt(
  state: &mut OpState,
  rid: u32,
  input: &[u8],
  output: &mut [u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::DecipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.decrypt(input, output);
  true
}

#[op]
pub fn op_node_decipheriv_final(
  state: &mut OpState,
  rid: u32,
  input: &[u8],
  output: &mut [u8],
) -> Result<(), AnyError> {
  let context = state.resource_table.take::<cipher::DecipherContext>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  context.r#final(input, output)
}

#[op]
pub fn op_node_sign(
  digest: &[u8],
  digest_type: &str,
  key: StringOrBuffer,
  key_type: &str,
  key_format: &str,
) -> Result<ZeroCopyBuf, AnyError> {
  match key_type {
    "rsa" => {
      use rsa::pkcs1v15::SigningKey;
      use signature::hazmat::PrehashSigner;
      let key = match key_format {
        "pem" => RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)
          .map_err(|_| type_error("Invalid RSA key"))?,
        // TODO(kt3k): Support der and jwk formats
        _ => {
          return Err(type_error(format!(
            "Unsupported key format: {}",
            key_format
          )))
        }
      };
      Ok(
        match digest_type {
          "sha224" => {
            let signing_key = SigningKey::<sha2::Sha224>::new_with_prefix(key);
            signing_key.sign_prehash(digest)?.to_vec()
          }
          "sha256" => {
            let signing_key = SigningKey::<sha2::Sha256>::new_with_prefix(key);
            signing_key.sign_prehash(digest)?.to_vec()
          }
          "sha384" => {
            let signing_key = SigningKey::<sha2::Sha384>::new_with_prefix(key);
            signing_key.sign_prehash(digest)?.to_vec()
          }
          "sha512" => {
            let signing_key = SigningKey::<sha2::Sha512>::new_with_prefix(key);
            signing_key.sign_prehash(digest)?.to_vec()
          }
          _ => {
            return Err(type_error(format!(
              "Unknown digest algorithm: {}",
              digest_type
            )))
          }
        }
        .into(),
      )
    }
    _ => Err(type_error(format!(
      "Signing with {} keys is not supported yet",
      key_type
    ))),
  }
}

fn pbkdf2_sync(
  password: &[u8],
  salt: &[u8],
  iterations: u32,
  digest: &str,
  derived_key: &mut [u8],
) -> Result<(), AnyError> {
  macro_rules! pbkdf2_hmac {
    ($digest:ty) => {{
      pbkdf2::pbkdf2_hmac::<$digest>(password, salt, iterations, derived_key)
    }};
  }

  match digest {
    "md4" => pbkdf2_hmac!(md4::Md4),
    "md5" => pbkdf2_hmac!(md5::Md5),
    "ripemd160" => pbkdf2_hmac!(ripemd::Ripemd160),
    "sha1" => pbkdf2_hmac!(sha1::Sha1),
    "sha224" => pbkdf2_hmac!(sha2::Sha224),
    "sha256" => pbkdf2_hmac!(sha2::Sha256),
    "sha384" => pbkdf2_hmac!(sha2::Sha384),
    "sha512" => pbkdf2_hmac!(sha2::Sha512),
    _ => return Err(type_error("Unknown digest")),
  }

  Ok(())
}

#[op]
pub fn op_node_pbkdf2(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  iterations: u32,
  digest: &str,
  derived_key: &mut [u8],
) -> bool {
  pbkdf2_sync(&password, &salt, iterations, digest, derived_key).is_ok()
}

#[op]
pub async fn op_node_pbkdf2_async(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  iterations: u32,
  digest: String,
  keylen: usize,
) -> Result<ZeroCopyBuf, AnyError> {
  tokio::task::spawn_blocking(move || {
    let mut derived_key = vec![0; keylen];
    pbkdf2_sync(&password, &salt, iterations, &digest, &mut derived_key)
      .map(|_| derived_key.into())
  })
  .await?
}

#[op]
pub fn op_node_generate_secret(buf: &mut [u8]) {
  rand::thread_rng().fill(buf);
}

#[op]
pub async fn op_node_generate_secret_async(len: i32) -> ZeroCopyBuf {
  tokio::task::spawn_blocking(move || {
    let mut buf = vec![0u8; len as usize];
    rand::thread_rng().fill(&mut buf[..]);
    buf.into()
  })
  .await
  .unwrap()
}

fn hkdf_sync(
  hash: &str,
  ikm: &[u8],
  salt: &[u8],
  info: &[u8],
  okm: &mut [u8],
) -> Result<(), AnyError> {
  macro_rules! hkdf {
    ($hash:ty) => {{
      let hk = Hkdf::<$hash>::new(Some(salt), ikm);
      hk.expand(info, okm)
        .map_err(|_| type_error("HKDF-Expand failed"))?;
    }};
  }

  match hash {
    "md4" => hkdf!(md4::Md4),
    "md5" => hkdf!(md5::Md5),
    "ripemd160" => hkdf!(ripemd::Ripemd160),
    "sha1" => hkdf!(sha1::Sha1),
    "sha224" => hkdf!(sha2::Sha224),
    "sha256" => hkdf!(sha2::Sha256),
    "sha384" => hkdf!(sha2::Sha384),
    "sha512" => hkdf!(sha2::Sha512),
    _ => return Err(type_error("Unknown digest")),
  }

  Ok(())
}

#[op]
pub fn op_node_hkdf(
  hash: &str,
  ikm: &[u8],
  salt: &[u8],
  info: &[u8],
  okm: &mut [u8],
) -> Result<(), AnyError> {
  hkdf_sync(hash, ikm, salt, info, okm)
}

#[op]
pub async fn op_node_hkdf_async(
  hash: String,
  ikm: ZeroCopyBuf,
  salt: ZeroCopyBuf,
  info: ZeroCopyBuf,
  okm_len: usize,
) -> Result<ZeroCopyBuf, AnyError> {
  tokio::task::spawn_blocking(move || {
    let mut okm = vec![0u8; okm_len];
    hkdf_sync(&hash, &ikm, &salt, &info, &mut okm)?;
    Ok(okm.into())
  })
  .await?
}

#[op]
pub fn op_node_random_int(min: i32, max: i32) -> Result<i32, AnyError> {
  let mut rng = rand::thread_rng();
  // Uniform distribution is required to avoid Modulo Bias
  // https://en.wikipedia.org/wiki/Fisher–Yates_shuffle#Modulo_bias
  let dist = Uniform::from(min..max);

  Ok(dist.sample(&mut rng))
}
