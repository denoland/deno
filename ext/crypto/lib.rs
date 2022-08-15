// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
use rsa::pkcs1::der::Decode;
use rsa::pkcs1::der::Encode;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::der::asn1;
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
mod encrypt;
mod export_key;
mod generate_key;
mod import_key;
mod key;
mod shared;

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
use crate::shared::ID_MFG1;
use crate::shared::ID_P_SPECIFIED;
use crate::shared::ID_SHA1_OID;
use once_cell::sync::Lazy;

pub fn init(maybe_seed: Option<u64>) -> Extension {
  Extension::builder()
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
pub fn op_crypto_get_random_values(
  state: &mut OpState,
  mut zero_copy: ZeroCopyBuf,
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
      let private_key = RsaPrivateKey::from_pkcs1_der(&*args.key.data)?;
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
      let private_key = RsaPrivateKey::from_pkcs1_der(&*args.key.data)?;

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
      let public_key = read_rsa_public_key(args.key)?;

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
    Algorithm::Ecdsa => {
      let signing_alg: &EcdsaSigningAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.try_into()?;
      let verify_alg: &EcdsaVerificationAlgorithm =
        args.named_curve.ok_or_else(not_supported)?.try_into()?;

      let private_key;

      let public_key_bytes = match args.key.r#type {
        KeyType::Private => {
          private_key = EcdsaKeyPair::from_pkcs8(signing_alg, &*args.key.data)?;

          private_key.public_key().as_ref()
        }
        KeyType::Public => &*args.key.data,
        _ => return Err(type_error("Invalid Key format".to_string())),
      };

      let public_key =
        ring::signature::UnparsedPublicKey::new(verify_alg, public_key_bytes);

      public_key.verify(data, &*args.signature).is_ok()
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
      RsaPrivateKey::from_pkcs1_der(&*key_data.data)?.to_public_key()
    }
    KeyType::Public => RsaPublicKey::from_pkcs1_der(&*key_data.data)?,
    KeyType::Secret => unreachable!("unexpected KeyType::Secret"),
  };
  Ok(public_key)
}

// The parameters field associated with OID id-RSASSA-PSS
// Defined in RFC 3447, section A.2.3
//
// RSASSA-PSS-params ::= SEQUENCE {
//   hashAlgorithm      [0] HashAlgorithm    DEFAULT sha1,
//   maskGenAlgorithm   [1] MaskGenAlgorithm DEFAULT mgf1SHA1,
//   saltLength         [2] INTEGER          DEFAULT 20,
//   trailerField       [3] TrailerField     DEFAULT trailerFieldBC
// }
pub struct PssPrivateKeyParameters<'a> {
  pub hash_algorithm: rsa::pkcs8::AlgorithmIdentifier<'a>,
  pub mask_gen_algorithm: rsa::pkcs8::AlgorithmIdentifier<'a>,
  pub salt_length: u32,
}

// Context-specific tag number for hashAlgorithm.
const HASH_ALGORITHM_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(0);

// Context-specific tag number for maskGenAlgorithm.
const MASK_GEN_ALGORITHM_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(1);

// Context-specific tag number for saltLength.
const SALT_LENGTH_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(2);

// Context-specific tag number for pSourceAlgorithm
const P_SOURCE_ALGORITHM_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(2);

// Default HashAlgorithm for RSASSA-PSS-params (sha1)
//
// sha1 HashAlgorithm ::= {
//   algorithm   id-sha1,
//   parameters  SHA1Parameters : NULL
// }
//
// SHA1Parameters ::= NULL
static SHA1_HASH_ALGORITHM: Lazy<rsa::pkcs8::AlgorithmIdentifier<'static>> =
  Lazy::new(|| {
    rsa::pkcs8::AlgorithmIdentifier {
      // id-sha1
      oid: ID_SHA1_OID,
      // NULL
      parameters: Some(asn1::AnyRef::from(asn1::Null)),
    }
  });

// TODO(@littledivy): `pkcs8` should provide AlgorithmIdentifier to Any conversion.
static ENCODED_SHA1_HASH_ALGORITHM: Lazy<Vec<u8>> =
  Lazy::new(|| SHA1_HASH_ALGORITHM.to_vec().unwrap());
// Default MaskGenAlgrithm for RSASSA-PSS-params (mgf1SHA1)
//
// mgf1SHA1 MaskGenAlgorithm ::= {
//   algorithm   id-mgf1,
//   parameters  HashAlgorithm : sha1
// }
static MGF1_SHA1_MASK_ALGORITHM: Lazy<
  rsa::pkcs8::AlgorithmIdentifier<'static>,
> = Lazy::new(|| {
  rsa::pkcs8::AlgorithmIdentifier {
    // id-mgf1
    oid: ID_MFG1,
    // sha1
    parameters: Some(
      asn1::AnyRef::from_der(&ENCODED_SHA1_HASH_ALGORITHM).unwrap(),
    ),
  }
});

// Default PSourceAlgorithm for RSAES-OAEP-params
// The default label is an empty string.
//
// pSpecifiedEmpty    PSourceAlgorithm ::= {
//   algorithm   id-pSpecified,
//   parameters  EncodingParameters : emptyString
// }
//
// emptyString    EncodingParameters ::= ''H
static P_SPECIFIED_EMPTY: Lazy<rsa::pkcs8::AlgorithmIdentifier<'static>> =
  Lazy::new(|| {
    rsa::pkcs8::AlgorithmIdentifier {
      // id-pSpecified
      oid: ID_P_SPECIFIED,
      // EncodingParameters
      parameters: Some(asn1::AnyRef::from(
        asn1::OctetStringRef::new(b"").unwrap(),
      )),
    }
  });

fn decode_content_tag<'a, T>(
  decoder: &mut rsa::pkcs8::der::SliceReader<'a>,
  tag: rsa::pkcs8::der::TagNumber,
) -> rsa::pkcs8::der::Result<Option<T>>
where
  T: rsa::pkcs8::der::Decode<'a>,
{
  Ok(
    rsa::pkcs8::der::asn1::ContextSpecific::<T>::decode_explicit(decoder, tag)?
      .map(|field| field.value),
  )
}

impl<'a> TryFrom<rsa::pkcs8::der::asn1::AnyRef<'a>>
  for PssPrivateKeyParameters<'a>
{
  type Error = rsa::pkcs8::der::Error;

  fn try_from(
    any: rsa::pkcs8::der::asn1::AnyRef<'a>,
  ) -> rsa::pkcs8::der::Result<PssPrivateKeyParameters<'a>> {
    any.sequence(|decoder| {
      let hash_algorithm =
        decode_content_tag::<rsa::pkcs8::AlgorithmIdentifier>(
          decoder,
          HASH_ALGORITHM_TAG,
        )?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*SHA1_HASH_ALGORITHM);

      let mask_gen_algorithm = decode_content_tag::<
        rsa::pkcs8::AlgorithmIdentifier,
      >(decoder, MASK_GEN_ALGORITHM_TAG)?
      .map(TryInto::try_into)
      .transpose()?
      .unwrap_or(*MGF1_SHA1_MASK_ALGORITHM);

      let salt_length = decode_content_tag::<u32>(decoder, SALT_LENGTH_TAG)?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(20);

      Ok(Self {
        hash_algorithm,
        mask_gen_algorithm,
        salt_length,
      })
    })
  }
}

// The parameters field associated with OID id-RSAES-OAEP
// Defined in RFC 3447, section A.2.1
//
// RSAES-OAEP-params ::= SEQUENCE {
//   hashAlgorithm     [0] HashAlgorithm    DEFAULT sha1,
//   maskGenAlgorithm  [1] MaskGenAlgorithm DEFAULT mgf1SHA1,
//   pSourceAlgorithm  [2] PSourceAlgorithm DEFAULT pSpecifiedEmpty
// }
pub struct OaepPrivateKeyParameters<'a> {
  pub hash_algorithm: rsa::pkcs8::AlgorithmIdentifier<'a>,
  pub mask_gen_algorithm: rsa::pkcs8::AlgorithmIdentifier<'a>,
  pub p_source_algorithm: rsa::pkcs8::AlgorithmIdentifier<'a>,
}

impl<'a> TryFrom<rsa::pkcs8::der::asn1::AnyRef<'a>>
  for OaepPrivateKeyParameters<'a>
{
  type Error = rsa::pkcs8::der::Error;

  fn try_from(
    any: rsa::pkcs8::der::asn1::AnyRef<'a>,
  ) -> rsa::pkcs8::der::Result<OaepPrivateKeyParameters<'a>> {
    any.sequence(|decoder| {
      let hash_algorithm =
        decode_content_tag::<rsa::pkcs8::AlgorithmIdentifier>(
          decoder,
          HASH_ALGORITHM_TAG,
        )?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*SHA1_HASH_ALGORITHM);

      let mask_gen_algorithm = decode_content_tag::<
        rsa::pkcs8::AlgorithmIdentifier,
      >(decoder, MASK_GEN_ALGORITHM_TAG)?
      .map(TryInto::try_into)
      .transpose()?
      .unwrap_or(*MGF1_SHA1_MASK_ALGORITHM);

      let p_source_algorithm = decode_content_tag::<
        rsa::pkcs8::AlgorithmIdentifier,
      >(decoder, P_SOURCE_ALGORITHM_TAG)?
      .map(TryInto::try_into)
      .transpose()?
      .unwrap_or(*P_SPECIFIED_EMPTY);

      Ok(Self {
        hash_algorithm,
        mask_gen_algorithm,
        p_source_algorithm,
      })
    })
  }
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
