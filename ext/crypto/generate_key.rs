use crate::shared::*;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;
use elliptic_curve::rand_core::OsRng;
use num_traits::FromPrimitive;
use once_cell::sync::Lazy;
use ring::rand::SecureRandom;
use ring::signature::EcdsaKeyPair;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::BigUint;
use rsa::RsaPrivateKey;
use serde::Deserialize;

// Allowlist for RSA public exponents.
static PUB_EXPONENT_1: Lazy<BigUint> =
  Lazy::new(|| BigUint::from_u64(3).unwrap());
static PUB_EXPONENT_2: Lazy<BigUint> =
  Lazy::new(|| BigUint::from_u64(65537).unwrap());

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "algorithm")]
pub enum GenerateKeyOptions {
  #[serde(rename = "RSA", rename_all = "camelCase")]
  Rsa {
    modulus_length: u32,
    #[serde(with = "serde_bytes")]
    public_exponent: Vec<u8>,
  },
  #[serde(rename = "EC", rename_all = "camelCase")]
  Ec { named_curve: EcNamedCurve },
  #[serde(rename = "AES", rename_all = "camelCase")]
  Aes { length: usize },
  #[serde(rename = "HMAC", rename_all = "camelCase")]
  Hmac {
    hash: ShaHash,
    length: Option<usize>,
  },
}

#[op]
pub async fn op_crypto_generate_key(
  opts: GenerateKeyOptions,
) -> Result<ZeroCopyBuf, AnyError> {
  let fun = || match opts {
    GenerateKeyOptions::Rsa {
      modulus_length,
      public_exponent,
    } => generate_key_rsa(modulus_length, &public_exponent),
    GenerateKeyOptions::Ec { named_curve } => generate_key_ec(named_curve),
    GenerateKeyOptions::Aes { length } => generate_key_aes(length),
    GenerateKeyOptions::Hmac { hash, length } => {
      generate_key_hmac(hash, length)
    }
  };
  let buf = tokio::task::spawn_blocking(fun).await.unwrap()?;
  Ok(buf.into())
}

fn generate_key_rsa(
  modulus_length: u32,
  public_exponent: &[u8],
) -> Result<Vec<u8>, AnyError> {
  let exponent = BigUint::from_bytes_be(public_exponent);
  if exponent != *PUB_EXPONENT_1 && exponent != *PUB_EXPONENT_2 {
    return Err(operation_error("Bad public exponent"));
  }

  let mut rng = OsRng;

  let private_key =
    RsaPrivateKey::new_with_exp(&mut rng, modulus_length as usize, &exponent)
      .map_err(|_| operation_error("Failed to generate RSA key"))?;

  let private_key = private_key
    .to_pkcs1_der()
    .map_err(|_| operation_error("Failed to serialize RSA key"))?;

  Ok(private_key.as_bytes().to_vec())
}

fn generate_key_ec(named_curve: EcNamedCurve) -> Result<Vec<u8>, AnyError> {
  let curve = match named_curve {
    EcNamedCurve::P256 => &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
    EcNamedCurve::P384 => &ring::signature::ECDSA_P384_SHA384_FIXED_SIGNING,
    _ => return Err(not_supported_error("Unsupported named curve")),
  };

  let rng = ring::rand::SystemRandom::new();

  let pkcs8 = EcdsaKeyPair::generate_pkcs8(curve, &rng)
    .map_err(|_| operation_error("Failed to generate EC key"))?;

  Ok(pkcs8.as_ref().to_vec())
}

fn generate_key_aes(length: usize) -> Result<Vec<u8>, AnyError> {
  if length % 8 != 0 || length > 256 {
    return Err(operation_error("Invalid AES key length"));
  }

  let mut key = vec![0u8; length / 8];
  let rng = ring::rand::SystemRandom::new();
  rng
    .fill(&mut key)
    .map_err(|_| operation_error("Failed to generate key"))?;

  Ok(key)
}

fn generate_key_hmac(
  hash: ShaHash,
  length: Option<usize>,
) -> Result<Vec<u8>, AnyError> {
  let hash = match hash {
    ShaHash::Sha1 => &ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
    ShaHash::Sha256 => &ring::hmac::HMAC_SHA256,
    ShaHash::Sha384 => &ring::hmac::HMAC_SHA384,
    ShaHash::Sha512 => &ring::hmac::HMAC_SHA512,
  };

  let length = if let Some(length) = length {
    if length % 8 != 0 {
      return Err(operation_error("Invalid HMAC key length"));
    }

    let length = length / 8;
    if length > ring::digest::MAX_BLOCK_LEN {
      return Err(operation_error("Invalid HMAC key length"));
    }

    length
  } else {
    hash.digest_algorithm().block_len
  };

  let rng = ring::rand::SystemRandom::new();
  let mut key = vec![0u8; length];
  rng
    .fill(&mut key)
    .map_err(|_| operation_error("Failed to generate key"))?;

  Ok(key)
}
