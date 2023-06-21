// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::task::spawn_blocking;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::RustToV8Buf;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use hkdf::Hkdf;
use num_bigint::BigInt;
use num_bigint_dig::BigUint;
use num_traits::FromPrimitive;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::thread_rng;
use rand::Rng;
use std::future::Future;
use std::rc::Rc;

use p224::NistP224;
use p256::NistP256;
use p384::NistP384;
use rsa::padding::PaddingScheme;
use rsa::pkcs8::DecodePrivateKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::PublicKey;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use secp256k1::ecdh::SharedSecret;
use secp256k1::Secp256k1;
use secp256k1::SecretKey;

mod cipher;
mod dh;
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
  Ok(spawn_blocking(move || primes::is_probably_prime(&num, checks)).await?)
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
      spawn_blocking(move || primes::is_probably_prime(&candidate, checks))
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
) -> Result<RustToV8Buf, AnyError> {
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
) -> Result<RustToV8Buf, AnyError> {
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
) -> Result<RustToV8Buf, AnyError> {
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
) -> Result<RustToV8Buf, AnyError> {
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
) -> Result<RustToV8Buf, AnyError> {
  match key_type {
    "rsa" => {
      use rsa::pkcs1v15::SigningKey;
      use signature::hazmat::PrehashSigner;
      let key = match key_format {
        "pem" => RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)
          .map_err(|_| type_error("Invalid RSA private key"))?,
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

#[op]
fn op_node_verify(
  digest: &[u8],
  digest_type: &str,
  key: StringOrBuffer,
  key_type: &str,
  key_format: &str,
  signature: &[u8],
) -> Result<bool, AnyError> {
  match key_type {
    "rsa" => {
      use rsa::pkcs1v15::VerifyingKey;
      use signature::hazmat::PrehashVerifier;
      let key = match key_format {
        "pem" => RsaPublicKey::from_public_key_pem((&key).try_into()?)
          .map_err(|_| type_error("Invalid RSA public key"))?,
        // TODO(kt3k): Support der and jwk formats
        _ => {
          return Err(type_error(format!(
            "Unsupported key format: {}",
            key_format
          )))
        }
      };
      Ok(match digest_type {
        "sha224" => VerifyingKey::<sha2::Sha224>::new_with_prefix(key)
          .verify_prehash(digest, &signature.to_vec().try_into()?)
          .is_ok(),
        "sha256" => VerifyingKey::<sha2::Sha256>::new_with_prefix(key)
          .verify_prehash(digest, &signature.to_vec().try_into()?)
          .is_ok(),
        "sha384" => VerifyingKey::<sha2::Sha384>::new_with_prefix(key)
          .verify_prehash(digest, &signature.to_vec().try_into()?)
          .is_ok(),
        "sha512" => VerifyingKey::<sha2::Sha512>::new_with_prefix(key)
          .verify_prehash(digest, &signature.to_vec().try_into()?)
          .is_ok(),
        _ => {
          return Err(type_error(format!(
            "Unknown digest algorithm: {}",
            digest_type
          )))
        }
      })
    }
    _ => Err(type_error(format!(
      "Verifying with {} keys is not supported yet",
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
) -> Result<RustToV8Buf, AnyError> {
  spawn_blocking(move || {
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
pub async fn op_node_generate_secret_async(len: i32) -> RustToV8Buf {
  spawn_blocking(move || {
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
) -> Result<RustToV8Buf, AnyError> {
  spawn_blocking(move || {
    let mut okm = vec![0u8; okm_len];
    hkdf_sync(&hash, &ikm, &salt, &info, &mut okm)?;
    Ok(okm.into())
  })
  .await?
}

use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs1::EncodeRsaPublicKey;

use self::primes::Prime;

fn generate_rsa(
  modulus_length: usize,
  public_exponent: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  let mut rng = rand::thread_rng();
  let private_key = RsaPrivateKey::new_with_exp(
    &mut rng,
    modulus_length,
    &rsa::BigUint::from_usize(public_exponent).unwrap(),
  )?;
  let public_key = private_key.to_public_key();
  let private_key_der = private_key.to_pkcs1_der()?.as_bytes().to_vec();
  let public_key_der = public_key.to_pkcs1_der()?.to_vec();

  Ok((private_key_der.into(), public_key_der.into()))
}

#[op]
pub fn op_node_generate_rsa(
  modulus_length: usize,
  public_exponent: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  generate_rsa(modulus_length, public_exponent)
}

#[op]
pub async fn op_node_generate_rsa_async(
  modulus_length: usize,
  public_exponent: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(move || generate_rsa(modulus_length, public_exponent)).await?
}

fn dsa_generate(
  modulus_length: usize,
  divisor_length: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  let mut rng = rand::thread_rng();
  use dsa::pkcs8::EncodePrivateKey;
  use dsa::pkcs8::EncodePublicKey;
  use dsa::Components;
  use dsa::KeySize;
  use dsa::SigningKey;

  let key_size = match (modulus_length, divisor_length) {
    #[allow(deprecated)]
    (1024, 160) => KeySize::DSA_1024_160,
    (2048, 224) => KeySize::DSA_2048_224,
    (2048, 256) => KeySize::DSA_2048_256,
    (3072, 256) => KeySize::DSA_3072_256,
    _ => return Err(type_error("Invalid modulus_length or divisor_length")),
  };
  let components = Components::generate(&mut rng, key_size);
  let signing_key = SigningKey::generate(&mut rng, components);
  let verifying_key = signing_key.verifying_key();

  Ok((
    signing_key
      .to_pkcs8_der()
      .map_err(|_| type_error("Not valid pkcs8"))?
      .as_bytes()
      .to_vec()
      .into(),
    verifying_key
      .to_public_key_der()
      .map_err(|_| type_error("Not valid spki"))?
      .to_vec()
      .into(),
  ))
}

#[op]
pub fn op_node_dsa_generate(
  modulus_length: usize,
  divisor_length: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  dsa_generate(modulus_length, divisor_length)
}

#[op]
pub async fn op_node_dsa_generate_async(
  modulus_length: usize,
  divisor_length: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(move || dsa_generate(modulus_length, divisor_length)).await?
}

fn ec_generate(
  named_curve: &str,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  use ring::signature::EcdsaKeyPair;
  use ring::signature::KeyPair;

  let curve = match named_curve {
    "P-256" => &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
    "P-384" => &ring::signature::ECDSA_P384_SHA384_FIXED_SIGNING,
    _ => return Err(type_error("Unsupported named curve")),
  };

  let rng = ring::rand::SystemRandom::new();

  let pkcs8 = EcdsaKeyPair::generate_pkcs8(curve, &rng)
    .map_err(|_| type_error("Failed to generate EC key"))?;

  let public_key = EcdsaKeyPair::from_pkcs8(curve, pkcs8.as_ref())
    .map_err(|_| type_error("Failed to generate EC key"))?
    .public_key()
    .as_ref()
    .to_vec();
  Ok((pkcs8.as_ref().to_vec().into(), public_key.into()))
}

#[op]
pub fn op_node_ec_generate(
  named_curve: &str,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  ec_generate(named_curve)
}

#[op]
pub async fn op_node_ec_generate_async(
  named_curve: String,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(move || ec_generate(&named_curve)).await?
}

fn ed25519_generate() -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  use ring::signature::Ed25519KeyPair;
  use ring::signature::KeyPair;

  let mut rng = thread_rng();
  let mut seed = vec![0u8; 32];
  rng.fill(seed.as_mut_slice());

  let pair = Ed25519KeyPair::from_seed_unchecked(&seed)
    .map_err(|_| type_error("Failed to generate Ed25519 key"))?;

  let public_key = pair.public_key().as_ref().to_vec();
  Ok((seed.into(), public_key.into()))
}

#[op]
pub fn op_node_ed25519_generate() -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError>
{
  ed25519_generate()
}

#[op]
pub async fn op_node_ed25519_generate_async(
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(ed25519_generate).await?
}

fn x25519_generate() -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  // u-coordinate of the base point.
  const X25519_BASEPOINT_BYTES: [u8; 32] = [
    9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
  ];

  let mut pkey = [0; 32];

  let mut rng = thread_rng();
  rng.fill(pkey.as_mut_slice());

  let pkey_copy = pkey.to_vec();
  // https://www.rfc-editor.org/rfc/rfc7748#section-6.1
  // pubkey = x25519(a, 9) which is constant-time Montgomery ladder.
  //   https://eprint.iacr.org/2014/140.pdf page 4
  //   https://eprint.iacr.org/2017/212.pdf algorithm 8
  // pubkey is in LE order.
  let pubkey = x25519_dalek::x25519(pkey, X25519_BASEPOINT_BYTES);

  Ok((pkey_copy.into(), pubkey.to_vec().into()))
}

#[op]
pub fn op_node_x25519_generate() -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError>
{
  x25519_generate()
}

#[op]
pub async fn op_node_x25519_generate_async(
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(x25519_generate).await?
}

fn dh_generate_group(
  group_name: &str,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  let dh = match group_name {
    "modp5" => dh::DiffieHellman::group::<dh::Modp1536>(),
    "modp14" => dh::DiffieHellman::group::<dh::Modp2048>(),
    "modp15" => dh::DiffieHellman::group::<dh::Modp3072>(),
    "modp16" => dh::DiffieHellman::group::<dh::Modp4096>(),
    "modp17" => dh::DiffieHellman::group::<dh::Modp6144>(),
    "modp18" => dh::DiffieHellman::group::<dh::Modp8192>(),
    _ => return Err(type_error("Unsupported group name")),
  };

  Ok((
    dh.private_key.into_vec().into(),
    dh.public_key.into_vec().into(),
  ))
}

#[op]
pub fn op_node_dh_generate_group(
  group_name: &str,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  dh_generate_group(group_name)
}

#[op]
pub async fn op_node_dh_generate_group_async(
  group_name: String,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(move || dh_generate_group(&group_name)).await?
}

fn dh_generate(
  prime: Option<&[u8]>,
  prime_len: usize,
  generator: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  let prime = prime
    .map(|p| p.into())
    .unwrap_or_else(|| Prime::generate(prime_len));
  let dh = dh::DiffieHellman::new(prime, generator);

  Ok((
    dh.private_key.into_vec().into(),
    dh.public_key.into_vec().into(),
  ))
}

#[op]
pub fn op_node_dh_generate(
  prime: Option<&[u8]>,
  prime_len: usize,
  generator: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  dh_generate(prime, prime_len, generator)
}

// TODO(lev): This duplication should be avoided.
#[op]
pub fn op_node_dh_generate2(
  prime: ZeroCopyBuf,
  prime_len: usize,
  generator: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  dh_generate(Some(prime).as_deref(), prime_len, generator)
}

#[op]
pub fn op_node_dh_compute_secret(
  prime: ZeroCopyBuf,
  private_key: ZeroCopyBuf,
  their_public_key: ZeroCopyBuf,
) -> Result<RustToV8Buf, AnyError> {
  let pubkey: BigUint = BigUint::from_bytes_be(their_public_key.as_ref());
  let privkey: BigUint = BigUint::from_bytes_be(private_key.as_ref());
  let primei: BigUint = BigUint::from_bytes_be(prime.as_ref());
  let shared_secret: BigUint = pubkey.modpow(&privkey, &primei);

  Ok(shared_secret.to_bytes_be().into())
}

#[op]
pub async fn op_node_dh_generate_async(
  prime: Option<ZeroCopyBuf>,
  prime_len: usize,
  generator: usize,
) -> Result<(ZeroCopyBuf, ZeroCopyBuf), AnyError> {
  spawn_blocking(move || dh_generate(prime.as_deref(), prime_len, generator))
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

#[allow(clippy::too_many_arguments)]
fn scrypt(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  keylen: u32,
  cost: u32,
  block_size: u32,
  parallelization: u32,
  _maxmem: u32,
  output_buffer: &mut [u8],
) -> Result<(), AnyError> {
  // Construct Params
  let params = scrypt::Params::new(
    cost as u8,
    block_size,
    parallelization,
    keylen as usize,
  )
  .unwrap();

  // Call into scrypt
  let res = scrypt::scrypt(&password, &salt, &params, output_buffer);
  if res.is_ok() {
    Ok(())
  } else {
    // TODO(lev): key derivation failed, so what?
    Err(generic_error("scrypt key derivation failed"))
  }
}

#[op]
pub fn op_node_scrypt_sync(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  keylen: u32,
  cost: u32,
  block_size: u32,
  parallelization: u32,
  maxmem: u32,
  output_buffer: &mut [u8],
) -> Result<(), AnyError> {
  scrypt(
    password,
    salt,
    keylen,
    cost,
    block_size,
    parallelization,
    maxmem,
    output_buffer,
  )
}

#[op]
pub async fn op_node_scrypt_async(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  keylen: u32,
  cost: u32,
  block_size: u32,
  parallelization: u32,
  maxmem: u32,
) -> Result<RustToV8Buf, AnyError> {
  spawn_blocking(move || {
    let mut output_buffer = vec![0u8; keylen as usize];
    let res = scrypt(
      password,
      salt,
      keylen,
      cost,
      block_size,
      parallelization,
      maxmem,
      &mut output_buffer,
    );

    if res.is_ok() {
      Ok(output_buffer.into())
    } else {
      // TODO(lev): rethrow the error?
      Err(generic_error("scrypt failure"))
    }
  })
  .await?
}

#[op]
pub fn op_node_ecdh_generate_keys(
  curve: &str,
  pubbuf: &mut [u8],
  privbuf: &mut [u8],
) -> Result<ResourceId, AnyError> {
  let mut rng = rand::thread_rng();
  match curve {
    "secp256k1" => {
      let secp = Secp256k1::new();
      let (privkey, pubkey) = secp.generate_keypair(&mut rng);
      pubbuf.copy_from_slice(&pubkey.serialize_uncompressed());
      privbuf.copy_from_slice(&privkey.secret_bytes());

      Ok(0)
    }
    "prime256v1" | "secp256r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP256>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_sec1_bytes().as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());
      Ok(0)
    }
    "secp384r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP384>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_sec1_bytes().as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());
      Ok(0)
    }
    "secp224r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP224>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_sec1_bytes().as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());
      Ok(0)
    }
    &_ => todo!(),
  }
}

#[op]
pub fn op_node_ecdh_compute_secret(
  curve: &str,
  this_priv: Option<ZeroCopyBuf>,
  their_pub: &mut [u8],
  secret: &mut [u8],
) -> Result<(), AnyError> {
  match curve {
    "secp256k1" => {
      let this_secret_key = SecretKey::from_slice(
        this_priv.expect("no private key provided?").as_ref(),
      )
      .unwrap();
      let their_public_key =
        secp256k1::PublicKey::from_slice(their_pub).unwrap();
      let shared_secret =
        SharedSecret::new(&their_public_key, &this_secret_key);

      secret.copy_from_slice(&shared_secret.secret_bytes());
      Ok(())
    }
    "prime256v1" | "secp256r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP256>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP256>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    "secp384r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP384>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP384>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    "secp224r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP224>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP224>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    &_ => todo!(),
  }
}

#[op]
pub fn op_node_ecdh_compute_public_key(
  curve: &str,
  privkey: &[u8],
  pubkey: &mut [u8],
) -> Result<(), AnyError> {
  match curve {
    "secp256k1" => {
      let secp = Secp256k1::new();
      let secret_key = SecretKey::from_slice(privkey).unwrap();
      let public_key =
        secp256k1::PublicKey::from_secret_key(&secp, &secret_key);

      pubkey.copy_from_slice(&public_key.serialize_uncompressed());

      Ok(())
    }
    "prime256v1" | "secp256r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP256>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    "secp384r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP384>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    "secp224r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP224>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    &_ => todo!(),
  }
}

#[inline]
fn gen_prime(size: usize) -> RustToV8Buf {
  primes::Prime::generate(size).0.to_bytes_be().into()
}

#[op]
pub fn op_node_gen_prime(size: usize) -> RustToV8Buf {
  gen_prime(size)
}

#[op]
pub async fn op_node_gen_prime_async(
  size: usize,
) -> Result<RustToV8Buf, AnyError> {
  Ok(spawn_blocking(move || gen_prime(size)).await?)
}
