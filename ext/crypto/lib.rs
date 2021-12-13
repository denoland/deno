// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::not_supported;
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
use std::num::NonZeroU32;
use std::rc::Rc;

use block_modes::BlockMode;
use lazy_static::lazy_static;
use num_traits::cast::FromPrimitive;
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
use rsa::pkcs1::der::Decodable;
use rsa::pkcs1::der::Encodable;
use rsa::pkcs1::FromRsaPrivateKey;
use rsa::pkcs1::FromRsaPublicKey;
use rsa::pkcs8::der::asn1;
use rsa::pkcs8::FromPrivateKey;
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

mod export_key;
mod generate_key;
mod import_key;
mod key;
mod shared;

pub use crate::export_key::op_crypto_export_key;
pub use crate::generate_key::op_crypto_generate_key;
pub use crate::import_key::op_crypto_import_key;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::key::HkdfOutput;

use crate::shared::ID_MFG1;
use crate::shared::ID_P_SPECIFIED;
use crate::shared::ID_SHA1_OID;

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
      ("op_crypto_derive_bits", op_async(op_crypto_derive_bits)),
      ("op_crypto_import_key", op_sync(op_crypto_import_key)),
      ("op_crypto_export_key", op_sync(op_crypto_export_key)),
      ("op_crypto_encrypt_key", op_async(op_crypto_encrypt_key)),
      ("op_crypto_decrypt_key", op_async(op_crypto_decrypt_key)),
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

pub async fn op_crypto_sign_key(
  _state: Rc<RefCell<OpState>>,
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

pub async fn op_crypto_verify_key(
  _state: Rc<RefCell<OpState>>,
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

      let private_key = EcdsaKeyPair::from_pkcs8(signing_alg, &*args.key.data)?;
      let public_key_bytes = private_key.public_key().as_ref();
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

pub async fn op_crypto_derive_bits(
  _state: Rc<RefCell<OpState>>,
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
        .ok_or_else(|| type_error("Missing argument publicKey".to_string()))?;

      match named_curve {
        CryptoNamedCurve::P256 => {
          let secret_key = p256::SecretKey::from_pkcs8_der(&args.key.data)?;
          let public_key =
            p256::SecretKey::from_pkcs8_der(&public_key.data)?.public_key();

          let shared_secret = p256::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_secret_scalar(),
            public_key.as_affine(),
          );

          Ok(shared_secret.as_bytes().to_vec().into())
        }
        // TODO(@littledivy): support for P384
        // https://github.com/RustCrypto/elliptic-curves/issues/240
        _ => Err(type_error("Unsupported namedCurve".to_string())),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptArg {
  key: KeyData,
  algorithm: Algorithm,
  // RSA-OAEP
  hash: Option<CryptoHash>,
  label: Option<ZeroCopyBuf>,
  // AES-CBC
  iv: Option<ZeroCopyBuf>,
  length: Option<usize>,
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

pub async fn op_crypto_encrypt_key(
  _state: Rc<RefCell<OpState>>,
  args: EncryptArg,
  zero_copy: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  match algorithm {
    Algorithm::RsaOaep => {
      let public_key = read_rsa_public_key(args.key)?;
      let label = args.label.map(|l| String::from_utf8_lossy(&*l).to_string());
      let mut rng = OsRng;
      let padding = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => PaddingScheme::OAEP {
          digest: Box::new(Sha1::new()),
          mgf_digest: Box::new(Sha1::new()),
          label,
        },
        CryptoHash::Sha256 => PaddingScheme::OAEP {
          digest: Box::new(Sha256::new()),
          mgf_digest: Box::new(Sha256::new()),
          label,
        },
        CryptoHash::Sha384 => PaddingScheme::OAEP {
          digest: Box::new(Sha384::new()),
          mgf_digest: Box::new(Sha384::new()),
          label,
        },
        CryptoHash::Sha512 => PaddingScheme::OAEP {
          digest: Box::new(Sha512::new()),
          mgf_digest: Box::new(Sha512::new()),
          label,
        },
      };

      Ok(
        public_key
          .encrypt(&mut rng, padding, data)
          .map_err(|e| {
            custom_error("DOMExceptionOperationError", e.to_string())
          })?
          .into(),
      )
    }
    Algorithm::AesCbc => {
      let key = &*args.key.data;
      let length = args
        .length
        .ok_or_else(|| type_error("Missing argument length".to_string()))?;
      let iv = args
        .iv
        .ok_or_else(|| type_error("Missing argument iv".to_string()))?;

      // 2-3.
      let ciphertext = match length {
        128 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes128Cbc =
            block_modes::Cbc<aes::Aes128, block_modes::block_padding::Pkcs7>;

          let cipher = Aes128Cbc::new_from_slices(key, &iv)?;
          cipher.encrypt_vec(data)
        }
        192 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes192Cbc =
            block_modes::Cbc<aes::Aes192, block_modes::block_padding::Pkcs7>;

          let cipher = Aes192Cbc::new_from_slices(key, &iv)?;
          cipher.encrypt_vec(data)
        }
        256 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes256Cbc =
            block_modes::Cbc<aes::Aes256, block_modes::block_padding::Pkcs7>;

          let cipher = Aes256Cbc::new_from_slices(key, &iv)?;
          cipher.encrypt_vec(data)
        }
        _ => unreachable!(),
      };

      Ok(ciphertext.into())
    }
    _ => Err(type_error("Unsupported algorithm".to_string())),
  }
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

lazy_static! {
  // Default HashAlgorithm for RSASSA-PSS-params (sha1)
  //
  // sha1 HashAlgorithm ::= {
  //   algorithm   id-sha1,
  //   parameters  SHA1Parameters : NULL
  // }
  //
  // SHA1Parameters ::= NULL
  static ref SHA1_HASH_ALGORITHM: rsa::pkcs8::AlgorithmIdentifier<'static> = rsa::pkcs8::AlgorithmIdentifier {
    // id-sha1
    oid: ID_SHA1_OID,
    // NULL
    parameters: Some(asn1::Any::from(asn1::Null)),
  };

  // TODO(@littledivy): `pkcs8` should provide AlgorithmIdentifier to Any conversion.
  static ref ENCODED_SHA1_HASH_ALGORITHM: Vec<u8> = SHA1_HASH_ALGORITHM.to_vec().unwrap();
  // Default MaskGenAlgrithm for RSASSA-PSS-params (mgf1SHA1)
  //
  // mgf1SHA1 MaskGenAlgorithm ::= {
  //   algorithm   id-mgf1,
  //   parameters  HashAlgorithm : sha1
  // }
  static ref MGF1_SHA1_MASK_ALGORITHM: rsa::pkcs8::AlgorithmIdentifier<'static> = rsa::pkcs8::AlgorithmIdentifier {
    // id-mgf1
    oid: ID_MFG1,
    // sha1
    parameters: Some(asn1::Any::from_der(&ENCODED_SHA1_HASH_ALGORITHM).unwrap()),
  };

  // Default PSourceAlgorithm for RSAES-OAEP-params
  // The default label is an empty string.
  //
  // pSpecifiedEmpty    PSourceAlgorithm ::= {
  //   algorithm   id-pSpecified,
  //   parameters  EncodingParameters : emptyString
  // }
  //
  // emptyString    EncodingParameters ::= ''H
  static ref P_SPECIFIED_EMPTY: rsa::pkcs8::AlgorithmIdentifier<'static> = rsa::pkcs8::AlgorithmIdentifier {
    // id-pSpecified
    oid: ID_P_SPECIFIED,
    // EncodingParameters
    parameters: Some(asn1::Any::from(asn1::OctetString::new(b"").unwrap())),
  };
}

impl<'a> TryFrom<rsa::pkcs8::der::asn1::Any<'a>>
  for PssPrivateKeyParameters<'a>
{
  type Error = rsa::pkcs8::der::Error;

  fn try_from(
    any: rsa::pkcs8::der::asn1::Any<'a>,
  ) -> rsa::pkcs8::der::Result<PssPrivateKeyParameters> {
    any.sequence(|decoder| {
      let hash_algorithm = decoder
        .context_specific(HASH_ALGORITHM_TAG)?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*SHA1_HASH_ALGORITHM);

      let mask_gen_algorithm = decoder
        .context_specific(MASK_GEN_ALGORITHM_TAG)?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*MGF1_SHA1_MASK_ALGORITHM);

      let salt_length = decoder
        .context_specific(SALT_LENGTH_TAG)?
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

impl<'a> TryFrom<rsa::pkcs8::der::asn1::Any<'a>>
  for OaepPrivateKeyParameters<'a>
{
  type Error = rsa::pkcs8::der::Error;

  fn try_from(
    any: rsa::pkcs8::der::asn1::Any<'a>,
  ) -> rsa::pkcs8::der::Result<OaepPrivateKeyParameters> {
    any.sequence(|decoder| {
      let hash_algorithm = decoder
        .context_specific(HASH_ALGORITHM_TAG)?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*SHA1_HASH_ALGORITHM);

      let mask_gen_algorithm = decoder
        .context_specific(MASK_GEN_ALGORITHM_TAG)?
        .map(TryInto::try_into)
        .transpose()?
        .unwrap_or(*MGF1_SHA1_MASK_ALGORITHM);

      let p_source_algorithm = decoder
        .context_specific(P_SOURCE_ALGORITHM_TAG)?
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptArg {
  key: KeyData,
  algorithm: Algorithm,
  // RSA-OAEP
  hash: Option<CryptoHash>,
  label: Option<ZeroCopyBuf>,
  // AES-CBC
  iv: Option<ZeroCopyBuf>,
  length: Option<usize>,
}

pub async fn op_crypto_decrypt_key(
  _state: Rc<RefCell<OpState>>,
  args: DecryptArg,
  zero_copy: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  match algorithm {
    Algorithm::RsaOaep => {
      let private_key: RsaPrivateKey =
        RsaPrivateKey::from_pkcs1_der(&*args.key.data)?;
      let label = args.label.map(|l| String::from_utf8_lossy(&*l).to_string());
      let padding = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => PaddingScheme::OAEP {
          digest: Box::new(Sha1::new()),
          mgf_digest: Box::new(Sha1::new()),
          label,
        },
        CryptoHash::Sha256 => PaddingScheme::OAEP {
          digest: Box::new(Sha256::new()),
          mgf_digest: Box::new(Sha256::new()),
          label,
        },
        CryptoHash::Sha384 => PaddingScheme::OAEP {
          digest: Box::new(Sha384::new()),
          mgf_digest: Box::new(Sha384::new()),
          label,
        },
        CryptoHash::Sha512 => PaddingScheme::OAEP {
          digest: Box::new(Sha512::new()),
          mgf_digest: Box::new(Sha512::new()),
          label,
        },
      };

      Ok(
        private_key
          .decrypt(padding, data)
          .map_err(|e| {
            custom_error("DOMExceptionOperationError", e.to_string())
          })?
          .into(),
      )
    }
    Algorithm::AesCbc => {
      let key = &*args.key.data;
      let length = args
        .length
        .ok_or_else(|| type_error("Missing argument length".to_string()))?;
      let iv = args
        .iv
        .ok_or_else(|| type_error("Missing argument iv".to_string()))?;

      // 2.
      let plaintext = match length {
        128 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes128Cbc =
            block_modes::Cbc<aes::Aes128, block_modes::block_padding::Pkcs7>;
          let cipher = Aes128Cbc::new_from_slices(key, &iv)?;

          cipher.decrypt_vec(data).map_err(|_| {
            custom_error(
              "DOMExceptionOperationError",
              "Decryption failed".to_string(),
            )
          })?
        }
        192 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes192Cbc =
            block_modes::Cbc<aes::Aes192, block_modes::block_padding::Pkcs7>;
          let cipher = Aes192Cbc::new_from_slices(key, &iv)?;

          cipher.decrypt_vec(data).map_err(|_| {
            custom_error(
              "DOMExceptionOperationError",
              "Decryption failed".to_string(),
            )
          })?
        }
        256 => {
          // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
          type Aes256Cbc =
            block_modes::Cbc<aes::Aes256, block_modes::block_padding::Pkcs7>;
          let cipher = Aes256Cbc::new_from_slices(key, &iv)?;

          cipher.decrypt_vec(data).map_err(|_| {
            custom_error(
              "DOMExceptionOperationError",
              "Decryption failed".to_string(),
            )
          })?
        }
        _ => unreachable!(),
      };

      // 6.
      Ok(plaintext.into())
    }
    _ => Err(type_error("Unsupported algorithm".to_string())),
  }
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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
