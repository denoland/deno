// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use aes_kw::KekAes128;
use aes_kw::KekAes192;
use aes_kw::KekAes256;

use deno_core::error::custom_error;
use deno_core::error::not_supported;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;

use deno_core::Extension;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use shared::operation_error;

use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use rand::rngs::OsRng;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;
use ring::digest;
use ring::hkdf;
use ring::hmac::Algorithm as HmacAlgorithm;
use ring::hmac::Key as HmacKey;
use ring::pbkdf2;
use ring::rand as RingRand;
use ring::signature::EcdsaKeyPair;
use ring::signature::EcdsaSigningAlgorithm;
use ring::signature::EcdsaVerificationAlgorithm;
use ring::signature::KeyPair;
use rsa::padding::PaddingScheme;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::PublicKey;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use sha1::Sha1;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;
use std::convert::TryFrom;
use std::num::NonZeroU32;
use std::path::PathBuf;

pub use rand; // Re-export rand

mod decrypt;
mod ed25519;
mod encrypt;
mod export_key;
mod generate_key;
mod import_key;
mod key;
mod shared;
mod x25519;

pub use crate::decrypt::op_crypto_decrypt;
pub use crate::encrypt::op_crypto_encrypt;
pub use crate::export_key::op_crypto_export_key;
pub use crate::generate_key::op_crypto_generate_key;
pub use crate::import_key::op_crypto_import_key;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::key::HkdfOutput;
use crate::shared::RawKeyData;

pub fn init(maybe_seed: Option<u64>) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec!["deno_webidl", "deno_web"])
    .js(include_js_files!(
      prefix "deno:ext/crypto",
      "00_crypto.js",
      "01_webidl.js",
    ))
    .ops(vec![
      op_crypto_get_random_values::decl(),
      op_crypto_generate_key::decl(),
      op_crypto_sign_key::decl(),
      op_crypto_verify_key::decl(),
      op_crypto_derive_bits::decl(),
      op_crypto_import_key::decl(),
      op_crypto_export_key::decl(),
      op_crypto_encrypt::decl(),
      op_crypto_decrypt::decl(),
      op_crypto_subtle_digest::decl(),
      op_crypto_random_uuid::decl(),
      op_crypto_wrap_key::decl(),
      op_crypto_unwrap_key::decl(),
      op_crypto_base64url_decode::decl(),
      op_crypto_base64url_encode::decl(),
      x25519::op_generate_x25519_keypair::decl(),
      x25519::op_derive_bits_x25519::decl(),
      x25519::op_import_spki_x25519::decl(),
      x25519::op_import_pkcs8_x25519::decl(),
      ed25519::op_generate_ed25519_keypair::decl(),
      ed25519::op_import_spki_ed25519::decl(),
      ed25519::op_import_pkcs8_ed25519::decl(),
      ed25519::op_sign_ed25519::decl(),
      ed25519::op_verify_ed25519::decl(),
      ed25519::op_export_spki_ed25519::decl(),
      ed25519::op_export_pkcs8_ed25519::decl(),
      ed25519::op_jwk_x_ed25519::decl(),
      x25519::op_export_spki_x25519::decl(),
      x25519::op_export_pkcs8_x25519::decl(),
    ])
    .state(move |state| {
      if let Some(seed) = maybe_seed {
        state.put(StdRng::seed_from_u64(seed));
      }
      Ok(())
    })
    .build()
}

#[op]
pub fn op_crypto_base64url_decode(data: String) -> ZeroCopyBuf {
  let data: Vec<u8> =
    base64::decode_config(data, base64::URL_SAFE_NO_PAD).unwrap();
  data.into()
}

#[op]
pub fn op_crypto_base64url_encode(data: ZeroCopyBuf) -> String {
  let data: String = base64::encode_config(data, base64::URL_SAFE_NO_PAD);
  data
}

#[op(fast)]
pub fn op_crypto_get_random_values(
  state: &mut OpState,
  out: &mut [u8],
) -> Result<(), AnyError> {
  if out.len() > 65536 {
    return Err(
      deno_web::DomExceptionQuotaExceededError::new(&format!("The ArrayBufferView's byte length ({}) exceeds the number of bytes of entropy available via this API (65536)", out.len()))
        .into(),
    );
  }

  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  if let Some(seeded_rng) = maybe_seeded_rng {
    seeded_rng.fill(out);
  } else {
    let mut rng = thread_rng();
    rng.fill(out);
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyFormat {
  Raw,
  Pkcs8,
  Spki,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyType {
  Secret,
  Private,
  Public,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct KeyData {
  r#type: KeyType,
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

#[op]
pub async fn op_crypto_sign_key(
  args: SignArg,
  zero_copy: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  let signature = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let private_key = RsaPrivateKey::from_pkcs1_der(&args.key.data)?;
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA1),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_256),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_384),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(data);
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
      let private_key = RsaPrivateKey::from_pkcs1_der(&args.key.data)?;

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
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha1, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha256, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha384, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(data);
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

      let key_pair = EcdsaKeyPair::from_pkcs8(curve, &args.key.data)?;
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

      let key = HmacKey::new(hash, &args.key.data);

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
  named_curve: Option<CryptoNamedCurve>,
}

#[op]
pub async fn op_crypto_verify_key(
  args: VerifyArg,
  zero_copy: ZeroCopyBuf,
) -> Result<bool, AnyError> {
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  let verification = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let public_key = read_rsa_public_key(args.key)?;
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA1),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_256),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_384),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(data);
          (
            PaddingScheme::PKCS1v15Sign {
              hash: Some(rsa::hash::Hash::SHA2_512),
            },
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      public_key.verify(padding, &hashed, &args.signature).is_ok()
    }
    Algorithm::RsaPss => {
      let salt_len = args
        .salt_length
        .ok_or_else(|| type_error("Missing argument saltLength".to_string()))?
        as usize;
      let public_key = read_rsa_public_key(args.key)?;

      let rng = OsRng;
      let (padding, hashed) = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha1, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha256, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha384, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        CryptoHash::Sha512 => {
          let mut hasher = Sha512::new();
          hasher.update(data);
          (
            PaddingScheme::new_pss_with_salt::<Sha512, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
      };

      public_key.verify(padding, &hashed, &args.signature).is_ok()
    }
    Algorithm::Hmac => {
      let hash: HmacAlgorithm = args.hash.ok_or_else(not_supported)?.into();
      let key = HmacKey::new(hash, &args.key.data);
      ring::hmac::verify(&key, data, &args.signature).is_ok()
    }
    Algorithm::Ecdsa => {
      let signing_alg: &EcdsaSigningAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.try_into()?;
      let verify_alg: &EcdsaVerificationAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.try_into()?;

      let private_key;

      let public_key_bytes = match args.key.r#type {
        KeyType::Private => {
          private_key = EcdsaKeyPair::from_pkcs8(signing_alg, &args.key.data)?;

          private_key.public_key().as_ref()
        }
        KeyType::Public => &*args.key.data,
        _ => return Err(type_error("Invalid Key format".to_string())),
      };

      let public_key =
        ring::signature::UnparsedPublicKey::new(verify_alg, public_key_bytes);

      public_key.verify(data, &args.signature).is_ok()
    }
    _ => return Err(type_error("Unsupported algorithm".to_string())),
  };

  Ok(verification)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeriveKeyArg {
  key: KeyData,
  algorithm: Algorithm,
  hash: Option<CryptoHash>,
  length: usize,
  iterations: Option<u32>,
  // ECDH
  public_key: Option<KeyData>,
  named_curve: Option<CryptoNamedCurve>,
  // HKDF
  info: Option<ZeroCopyBuf>,
}

#[op]
pub async fn op_crypto_derive_bits(
  args: DeriveKeyArg,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<ZeroCopyBuf, AnyError> {
  let algorithm = args.algorithm;
  match algorithm {
    Algorithm::Pbkdf2 => {
      let zero_copy = zero_copy.ok_or_else(not_supported)?;
      let salt = &*zero_copy;
      // The caller must validate these cases.
      assert!(args.length > 0);
      assert!(args.length % 8 == 0);

      let algorithm = match args.hash.ok_or_else(not_supported)? {
        CryptoHash::Sha1 => pbkdf2::PBKDF2_HMAC_SHA1,
        CryptoHash::Sha256 => pbkdf2::PBKDF2_HMAC_SHA256,
        CryptoHash::Sha384 => pbkdf2::PBKDF2_HMAC_SHA384,
        CryptoHash::Sha512 => pbkdf2::PBKDF2_HMAC_SHA512,
      };

      // This will never panic. We have already checked length earlier.
      let iterations =
        NonZeroU32::new(args.iterations.ok_or_else(not_supported)?).unwrap();
      let secret = args.key.data;
      let mut out = vec![0; args.length / 8];
      pbkdf2::derive(algorithm, iterations, salt, &secret, &mut out);
      Ok(out.into())
    }
    Algorithm::Ecdh => {
      let named_curve = args
        .named_curve
        .ok_or_else(|| type_error("Missing argument namedCurve".to_string()))?;

      let public_key = args
        .public_key
        .ok_or_else(|| type_error("Missing argument publicKey"))?;

      match named_curve {
        CryptoNamedCurve::P256 => {
          let secret_key = p256::SecretKey::from_pkcs8_der(&args.key.data)
            .map_err(|_| type_error("Unexpected error decoding private key"))?;

          let public_key = match public_key.r#type {
            KeyType::Private => {
              p256::SecretKey::from_pkcs8_der(&public_key.data)
                .map_err(|_| {
                  type_error("Unexpected error decoding private key")
                })?
                .public_key()
            }
            KeyType::Public => {
              let point = p256::EncodedPoint::from_bytes(public_key.data)
                .map_err(|_| {
                  type_error("Unexpected error decoding private key")
                })?;

              let pk = p256::PublicKey::from_encoded_point(&point);
              // pk is a constant time Option.
              if pk.is_some().into() {
                pk.unwrap()
              } else {
                return Err(type_error(
                  "Unexpected error decoding private key",
                ));
              }
            }
            _ => unreachable!(),
          };

          let shared_secret = p256::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_nonzero_scalar(),
            public_key.as_affine(),
          );

          // raw serialized x-coordinate of the computed point
          Ok(shared_secret.raw_secret_bytes().to_vec().into())
        }
        CryptoNamedCurve::P384 => {
          let secret_key = p384::SecretKey::from_pkcs8_der(&args.key.data)
            .map_err(|_| type_error("Unexpected error decoding private key"))?;

          let public_key = match public_key.r#type {
            KeyType::Private => {
              p384::SecretKey::from_pkcs8_der(&public_key.data)
                .map_err(|_| {
                  type_error("Unexpected error decoding private key")
                })?
                .public_key()
            }
            KeyType::Public => {
              let point = p384::EncodedPoint::from_bytes(public_key.data)
                .map_err(|_| {
                  type_error("Unexpected error decoding private key")
                })?;

              let pk = p384::PublicKey::from_encoded_point(&point);
              // pk is a constant time Option.
              if pk.is_some().into() {
                pk.unwrap()
              } else {
                return Err(type_error(
                  "Unexpected error decoding private key",
                ));
              }
            }
            _ => unreachable!(),
          };

          let shared_secret = p384::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_nonzero_scalar(),
            public_key.as_affine(),
          );

          // raw serialized x-coordinate of the computed point
          Ok(shared_secret.raw_secret_bytes().to_vec().into())
        }
      }
    }
    Algorithm::Hkdf => {
      let zero_copy = zero_copy.ok_or_else(not_supported)?;
      let salt = &*zero_copy;
      let algorithm = match args.hash.ok_or_else(not_supported)? {
        CryptoHash::Sha1 => hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY,
        CryptoHash::Sha256 => hkdf::HKDF_SHA256,
        CryptoHash::Sha384 => hkdf::HKDF_SHA384,
        CryptoHash::Sha512 => hkdf::HKDF_SHA512,
      };

      let info = args
        .info
        .ok_or_else(|| type_error("Missing argument info".to_string()))?;
      // IKM
      let secret = args.key.data;
      // L
      let length = args.length / 8;

      let salt = hkdf::Salt::new(algorithm, salt);
      let prk = salt.extract(&secret);
      let info = &[&*info];
      let okm = prk.expand(info, HkdfOutput(length)).map_err(|_e| {
        custom_error(
          "DOMExceptionOperationError",
          "The length provided for HKDF is too large",
        )
      })?;
      let mut r = vec![0u8; length];
      okm.fill(&mut r)?;
      Ok(r.into())
    }
    _ => Err(type_error("Unsupported algorithm".to_string())),
  }
}

fn read_rsa_public_key(key_data: KeyData) -> Result<RsaPublicKey, AnyError> {
  let public_key = match key_data.r#type {
    KeyType::Private => {
      RsaPrivateKey::from_pkcs1_der(&key_data.data)?.to_public_key()
    }
    KeyType::Public => RsaPublicKey::from_pkcs1_der(&key_data.data)?,
    KeyType::Secret => unreachable!("unexpected KeyType::Secret"),
  };
  Ok(public_key)
}

#[op]
pub fn op_crypto_random_uuid(state: &mut OpState) -> Result<String, AnyError> {
  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  let uuid = if let Some(seeded_rng) = maybe_seeded_rng {
    let mut bytes = [0u8; 16];
    seeded_rng.fill(&mut bytes);
    uuid::Builder::from_bytes(bytes)
      .with_version(uuid::Version::Random)
      .into_uuid()
  } else {
    uuid::Uuid::new_v4()
  };

  Ok(uuid.to_string())
}

#[op]
pub async fn op_crypto_subtle_digest(
  algorithm: CryptoHash,
  data: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let output = tokio::task::spawn_blocking(move || {
    digest::digest(algorithm.into(), &data)
      .as_ref()
      .to_vec()
      .into()
  })
  .await?;

  Ok(output)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapUnwrapKeyArg {
  key: RawKeyData,
  algorithm: Algorithm,
}

#[op]
pub fn op_crypto_wrap_key(
  args: WrapUnwrapKeyArg,
  data: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let algorithm = args.algorithm;

  match algorithm {
    Algorithm::AesKw => {
      let key = args.key.as_secret_key()?;

      if data.len() % 8 != 0 {
        return Err(type_error("Data must be multiple of 8 bytes"));
      }

      let wrapped_key = match key.len() {
        16 => KekAes128::new(key.into()).wrap_vec(&data),
        24 => KekAes192::new(key.into()).wrap_vec(&data),
        32 => KekAes256::new(key.into()).wrap_vec(&data),
        _ => return Err(type_error("Invalid key length")),
      }
      .map_err(|_| operation_error("encryption error"))?;

      Ok(wrapped_key.into())
    }
    _ => Err(type_error("Unsupported algorithm")),
  }
}

#[op]
pub fn op_crypto_unwrap_key(
  args: WrapUnwrapKeyArg,
  data: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let algorithm = args.algorithm;
  match algorithm {
    Algorithm::AesKw => {
      let key = args.key.as_secret_key()?;

      if data.len() % 8 != 0 {
        return Err(type_error("Data must be multiple of 8 bytes"));
      }

      let unwrapped_key = match key.len() {
        16 => KekAes128::new(key.into()).unwrap_vec(&data),
        24 => KekAes192::new(key.into()).unwrap_vec(&data),
        32 => KekAes256::new(key.into()).unwrap_vec(&data),
        _ => return Err(type_error("Invalid key length")),
      }
      .map_err(|_| {
        operation_error("decryption error - integrity check failed")
      })?;

      Ok(unwrapped_key.into())
    }
    _ => Err(type_error("Unsupported algorithm")),
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
