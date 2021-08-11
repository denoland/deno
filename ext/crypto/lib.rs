// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::not_supported;
use deno_core::error::null_opbuf;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;

use std::cell::RefCell;
use std::convert::TryInto;
use std::rc::Rc;

use lazy_static::lazy_static;
use num_traits::cast::FromPrimitive;
use rand::rngs::OsRng;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;
use ring::digest;
use ring::hmac::Algorithm as HmacAlgorithm;
use ring::hmac::Key as HmacKey;
use ring::rand as RingRand;
use ring::rand::SecureRandom;
use ring::signature::EcdsaKeyPair;
use ring::signature::EcdsaSigningAlgorithm;
use rsa::padding::PaddingScheme;
use rsa::pkcs8::FromPrivateKey;
use rsa::pkcs8::ToPrivateKey;
use rsa::BigUint;
use rsa::PublicKey;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use sha1::Sha1;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;
use std::path::PathBuf;

pub use rand; // Re-export rand

mod key;

use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;

// Allowlist for RSA public exponents.
lazy_static! {
  static ref PUB_EXPONENT_1: BigUint = BigUint::from_u64(3).unwrap();
  static ref PUB_EXPONENT_2: BigUint = BigUint::from_u64(65537).unwrap();
}

pub fn init(maybe_seed: Option<u64>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/crypto",
      "00_crypto.js",
      "01_webidl.js",
    ))
    .ops(vec![
      (
        "op_crypto_get_random_values",
        op_sync(op_crypto_get_random_values),
      ),
      ("op_crypto_generate_key", op_async(op_crypto_generate_key)),
      ("op_crypto_sign_key", op_async(op_crypto_sign_key)),
      ("op_crypto_verify_key", op_async(op_crypto_verify_key)),
      ("op_crypto_subtle_digest", op_async(op_crypto_subtle_digest)),
      ("op_crypto_random_uuid", op_sync(op_crypto_random_uuid)),
    ])
    .state(move |state| {
      if let Some(seed) = maybe_seed {
        state.put(StdRng::seed_from_u64(seed));
      }
      Ok(())
    })
    .build()
}

pub fn op_crypto_get_random_values(
  state: &mut OpState,
  mut zero_copy: ZeroCopyBuf,
  _: (),
) -> Result<(), AnyError> {
  if zero_copy.len() > 65536 {
    return Err(
      deno_web::DomExceptionQuotaExceededError::new(&format!("The ArrayBufferView's byte length ({}) exceeds the number of bytes of entropy available via this API (65536)", zero_copy.len()))
        .into(),
    );
  }

  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  if let Some(seeded_rng) = maybe_seeded_rng {
    seeded_rng.fill(&mut *zero_copy);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy);
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlgorithmArg {
  name: Algorithm,
  modulus_length: Option<u32>,
  public_exponent: Option<ZeroCopyBuf>,
  named_curve: Option<CryptoNamedCurve>,
  hash: Option<CryptoHash>,
  length: Option<usize>,
}

pub async fn op_crypto_generate_key(
  _state: Rc<RefCell<OpState>>,
  args: AlgorithmArg,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let algorithm = args.name;

  let key = match algorithm {
    Algorithm::RsassaPkcs1v15 | Algorithm::RsaPss => {
      let public_exponent = args.public_exponent.ok_or_else(not_supported)?;
      let modulus_length = args.modulus_length.ok_or_else(not_supported)?;

      let exponent = BigUint::from_bytes_be(&public_exponent);
      if exponent != *PUB_EXPONENT_1 && exponent != *PUB_EXPONENT_2 {
        return Err(custom_error(
          "DOMExceptionOperationError",
          "Bad public exponent",
        ));
      }

      let mut rng = OsRng;

      let private_key: RsaPrivateKey = tokio::task::spawn_blocking(
        move || -> Result<RsaPrivateKey, rsa::errors::Error> {
          RsaPrivateKey::new_with_exp(
            &mut rng,
            modulus_length as usize,
            &exponent,
          )
        },
      )
      .await
      .unwrap()
      .map_err(|e| custom_error("DOMExceptionOperationError", e.to_string()))?;

      private_key.to_pkcs8_der()?.as_ref().to_vec()
    }
    Algorithm::Ecdsa => {
      let curve: &EcdsaSigningAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.into();
      let rng = RingRand::SystemRandom::new();
      let private_key: Vec<u8> = tokio::task::spawn_blocking(
        move || -> Result<Vec<u8>, ring::error::Unspecified> {
          let pkcs8 = EcdsaKeyPair::generate_pkcs8(curve, &rng)?;
          Ok(pkcs8.as_ref().to_vec())
        },
      )
      .await
      .unwrap()
      .map_err(|_| {
        custom_error("DOMExceptionOperationError", "Key generation failed")
      })?;

      private_key
    }
    Algorithm::Hmac => {
      let hash: HmacAlgorithm = args.hash.ok_or_else(not_supported)?.into();

      let length = if let Some(length) = args.length {
        if (length % 8) != 0 {
          return Err(custom_error(
            "DOMExceptionOperationError",
            "hmac block length must be byte aligned",
          ));
        }
        let length = length / 8;
        if length > ring::digest::MAX_BLOCK_LEN {
          return Err(custom_error(
            "DOMExceptionOperationError",
            "hmac block length is too large",
          ));
        }
        length
      } else {
        hash.digest_algorithm().block_len
      };

      let rng = RingRand::SystemRandom::new();
      let mut key_bytes = [0; ring::digest::MAX_BLOCK_LEN];
      let key_bytes = &mut key_bytes[..length];
      rng.fill(key_bytes).map_err(|_| {
        custom_error("DOMExceptionOperationError", "Key generation failed")
      })?;

      key_bytes.to_vec()
    }
    _ => return Err(not_supported()),
  };

  Ok(key.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyFormat {
  Raw,
  Pkcs8,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct KeyData {
  // TODO(littledivy): Kept here to be used to importKey() in future.
  #[allow(dead_code)]
  r#type: KeyFormat,
  data: ZeroCopyBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignArg {
  key: KeyData,
  algorithm: Algorithm,
  salt_length: Option<u32>,
  hash: Option<CryptoHash>,
  named_curve: Option<CryptoNamedCurve>,
}

pub async fn op_crypto_sign_key(
  _state: Rc<RefCell<OpState>>,
  args: SignArg,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<ZeroCopyBuf, AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  let signature = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let private_key = RsaPrivateKey::from_pkcs8_der(&*args.key.data)?;
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA1),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_256),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_384),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_512),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      private_key.sign(padding, &hashed)?
    }
    Algorithm::RsaPss => {
      let private_key = RsaPrivateKey::from_pkcs8_der(&*args.key.data)?;

      let salt_len = args
        .salt_length
        .ok_or_else(|| type_error("Missing argument saltLength".to_string()))?
        as usize;

      let rng = OsRng;
      let (padding, digest_in) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha1, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha256, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha384, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha512, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      // Sign data based on computed padding and return buffer
      private_key.sign(padding, &digest_in)?
    }
    Algorithm::Ecdsa => {
      let curve: &EcdsaSigningAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.try_into()?;

      let key_pair = EcdsaKeyPair::from_pkcs8(curve, &*args.key.data)?;
      // We only support P256-SHA256 & P384-SHA384. These are recommended signature pairs.
      // https://briansmith.org/rustdoc/ring/signature/index.html#statics
      if let Some(hash) = args.hash {
        match hash {
          CryptoHash::Sha256 | CryptoHash::Sha384 => (),
          _ => return Err(type_error("Unsupported algorithm")),
        }
      };

      let rng = RingRand::SystemRandom::new();
      let signature = key_pair.sign(&rng, data)?;

      // Signature data as buffer.
      signature.as_ref().to_vec()
    }
    Algorithm::Hmac => {
      let hash: HmacAlgorithm = args.hash.ok_or_else(not_supported)?.into();

      let key = HmacKey::new(hash, &*args.key.data);

      let signature = ring::hmac::sign(&key, data);
      signature.as_ref().to_vec()
    }
    _ => return Err(type_error("Unsupported algorithm".to_string())),
  };

  Ok(signature.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyArg {
  key: KeyData,
  algorithm: Algorithm,
  salt_length: Option<u32>,
  hash: Option<CryptoHash>,
  signature: ZeroCopyBuf,
}

pub async fn op_crypto_verify_key(
  _state: Rc<RefCell<OpState>>,
  args: VerifyArg,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<bool, AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  let verification = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let public_key: RsaPublicKey =
        RsaPrivateKey::from_pkcs8_der(&*args.key.data)?.to_public_key();
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA1),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_256),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_384),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(&data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_512),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      public_key
        .verify(padding, &hashed, &*args.signature)
        .is_ok()
    }
    Algorithm::RsaPss => {
      let salt_len = args
        .salt_length
        .ok_or_else(|| type_error("Missing argument saltLength".to_string()))?
        as usize;
      let public_key: RsaPublicKey =
        RsaPrivateKey::from_pkcs8_der(&*args.key.data)?.to_public_key();

      let rng = OsRng;
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha1, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha256, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha384, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha512, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      public_key
        .verify(padding, &hashed, &*args.signature)
        .is_ok()
    }
    Algorithm::Hmac => {
      let hash: HmacAlgorithm = args.hash.ok_or_else(not_supported)?.into();
      let key = HmacKey::new(hash, &*args.key.data);
      ring::hmac::verify(&key, data, &*args.signature).is_ok()
    }
    _ => return Err(type_error("Unsupported algorithm".to_string())),
  };

  Ok(verification)
}

pub fn op_crypto_random_uuid(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<String, AnyError> {
  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  let uuid = if let Some(seeded_rng) = maybe_seeded_rng {
    let mut bytes = [0u8; 16];
    seeded_rng.fill(&mut bytes);
    uuid::Builder::from_bytes(bytes)
      .set_version(uuid::Version::Random)
      .build()
  } else {
    uuid::Uuid::new_v4()
  };

  Ok(uuid.to_string())
}

pub async fn op_crypto_subtle_digest(
  _state: Rc<RefCell<OpState>>,
  algorithm: CryptoHash,
  data: Option<ZeroCopyBuf>,
) -> Result<ZeroCopyBuf, AnyError> {
  let input = data.ok_or_else(null_opbuf)?;
  let output = tokio::task::spawn_blocking(move || {
    digest::digest(algorithm.into(), &input)
      .as_ref()
      .to_vec()
      .into()
  })
  .await?;

  Ok(output)
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
