// Copyright 2018-2026 the Deno authors. MIT license.

use std::num::NonZeroU32;

use aws_lc_rs::hkdf;
use aws_lc_rs::hmac::Algorithm as HmacAlgorithm;
use aws_lc_rs::hmac::Key as HmacKey;
use aws_lc_rs::pbkdf2;
use deno_core::convert::Uint8Array;
use deno_error::JsErrorBox;
use p256::ecdsa::Signature as P256Signature;
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::ecdsa::VerifyingKey as P256VerifyingKey;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use p384::ecdsa::Signature as P384Signature;
use p384::ecdsa::SigningKey as P384SigningKey;
use p384::ecdsa::VerifyingKey as P384VerifyingKey;
use p521::ecdsa::Signature as P521Signature;
use p521::ecdsa::SigningKey as P521SigningKey;
use p521::ecdsa::VerifyingKey as P521VerifyingKey;
pub use rand;
use rand::SeedableRng;
use rand::rngs::OsRng;
use rand::rngs::StdRng;
use rsa::Pss;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::signature::SignatureEncoding;
use rsa::signature::Signer;
use rsa::signature::Verifier;
use rsa::traits::SignatureScheme;
use serde::Deserialize;
use sha1::Sha1;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;
use sha3::Sha3_256;
use sha3::Sha3_384;
use sha3::Sha3_512;
use signature::hazmat::PrehashSigner;
use signature::hazmat::PrehashVerifier;

mod algorithm;
mod crypto;
mod crypto_key;
mod decrypt;
mod digest;
mod ed25519;
mod encrypt;
mod export_key;
mod generate_key;
mod import_key;
mod key;
mod key_store;
mod make_key;
mod mldsa;
mod mlkem;
mod node_interop;
mod shared;
mod subtle_crypto;
mod subtle_decrypt;
mod subtle_derive_bits;
mod subtle_derive_key;
mod subtle_encapsulate;
mod subtle_encapsulate_key;
mod subtle_encrypt;
mod subtle_export_key;
mod subtle_generate_key;
mod subtle_get_public_key;
mod subtle_import_key;
mod subtle_key;
mod subtle_sign;
mod subtle_verify;
mod subtle_wrap_key;
mod x25519;
mod x448;

pub use crate::decrypt::DecryptError;
pub use crate::ed25519::Ed25519Error;
pub use crate::encrypt::EncryptError;
pub use crate::export_key::ExportKeyError;
pub use crate::generate_key::GenerateKeyError;
pub use crate::import_key::ImportKeyError;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::key::HkdfOutput;
pub use crate::mldsa::MlDsaError;
pub use crate::mlkem::MlKemError;
pub use crate::shared::RawKeyData;
pub use crate::shared::SharedError;
pub use crate::x448::X448Error;
pub use crate::x25519::X25519Error;

// No standalone ops — every WebCrypto entry point is implemented as a
// method on the `Crypto`/`SubtleCrypto`/`CryptoKey` cppgc classes
// registered below under `objects`. The per-algorithm helpers
// (`subtle_*::run`, `import_key_inner`, `export_key_inner`, etc.) are
// pure-Rust and called directly from those method bodies.
deno_core::extension!(deno_crypto,
  deps = [ deno_webidl, deno_web ],
  objects = [
    crypto::Crypto,
    subtle_crypto::SubtleCrypto,
    crypto_key::CryptoKey,
  ],
  lazy_loaded_js = [ "00_crypto.js" ],
  options = {
    maybe_seed: Option<u64>,
  },
  state = |state, options| {
    if let Some(seed) = options.maybe_seed {
      state.put(StdRng::seed_from_u64(seed));
    }
  },
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CryptoError {
  #[class(inherit)]
  #[error(transparent)]
  General(
    #[from]
    #[inherit]
    SharedError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  JoinError(
    #[from]
    #[inherit]
    tokio::task::JoinError,
  ),
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] rsa::pkcs1::der::Error),
  #[class(type)]
  #[error("Missing argument hash")]
  MissingArgumentHash,
  #[class(type)]
  #[error("Missing argument saltLength")]
  MissingArgumentSaltLength,
  #[class(type)]
  #[error("unsupported algorithm")]
  UnsupportedAlgorithm,
  #[class(generic)]
  #[error(transparent)]
  KeyRejected(#[from] aws_lc_rs::error::KeyRejected),
  #[class(generic)]
  #[error(transparent)]
  RSA(#[from] rsa::Error),
  #[class(generic)]
  #[error(transparent)]
  Pkcs1(#[from] rsa::pkcs1::Error),
  #[class(generic)]
  #[error(transparent)]
  Unspecified(#[from] aws_lc_rs::error::Unspecified),
  #[class(type)]
  #[error("Invalid key format")]
  InvalidKeyFormat,
  #[class(generic)]
  #[error(transparent)]
  P256Ecdsa(#[from] p256::ecdsa::Error),
  #[class(type)]
  #[error("Unexpected error decoding private key")]
  DecodePrivateKey,
  #[class(type)]
  #[error("Missing argument publicKey")]
  MissingArgumentPublicKey,
  #[class(type)]
  #[error("Missing argument namedCurve")]
  MissingArgumentNamedCurve,
  #[class(type)]
  #[error("Missing argument info")]
  MissingArgumentInfo,
  #[class("DOMExceptionOperationError")]
  #[error("The length provided for HKDF is too large")]
  HKDFLengthTooLarge,
  #[class(generic)]
  #[error(transparent)]
  Base64Decode(#[from] base64::DecodeError),
  #[class(type)]
  #[error("Data must be multiple of 8 bytes")]
  DataInvalidSize,
  #[class(type)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class("DOMExceptionOperationError")]
  #[error("encryption error")]
  EncryptionError,
  #[class("DOMExceptionOperationError")]
  #[error("decryption error - integrity check failed")]
  DecryptionError,
  #[class("DOMExceptionOperationError")]
  #[error("Invalid XOF parameters")]
  InvalidXofParameters,
  #[class("DOMExceptionQuotaExceededError")]
  #[error(
    "The ArrayBufferView's byte length ({0}) exceeds the number of bytes of entropy available via this API (65536)"
  )]
  ArrayBufferViewLengthExceeded(usize),
  #[class("DOMExceptionTypeMismatchError")]
  #[error("The provided value is not an integer-type TypedArray")]
  TypedArrayNotInteger,
  #[class("DOMExceptionNotSupportedError")]
  #[error("Algorithm '{0}' is not supported")]
  UnsupportedDigestAlgorithm(String),
  #[class(inherit)]
  #[error(transparent)]
  Other(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
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

/// Owned key material handed to the sign/verify/derive ops after being looked
/// up from the Rust-side [`KeyStore`] by handle. Previously the key bytes were
/// serialized and passed from JavaScript on every operation.
pub struct KeyData {
  r#type: KeyType,
  data: Box<[u8]>,
}

impl From<&RawKeyData> for KeyData {
  fn from(raw: &RawKeyData) -> Self {
    let (r#type, data) = match raw {
      RawKeyData::Secret(d) => (KeyType::Secret, d),
      RawKeyData::Private(d) => (KeyType::Private, d),
      RawKeyData::Public(d) => (KeyType::Public, d),
      RawKeyData::Raw(d) => (KeyType::Secret, d),
      // Composite (ML-KEM/ML-DSA) keys are never handed to these ops.
      RawKeyData::SeededPrivate { .. } => unreachable!(),
    };
    KeyData {
      r#type,
      data: data.as_ref().into(),
    }
  }
}

#[derive(deno_core::FromV8)]
pub struct SignArg {
  #[from_v8(serde)]
  algorithm: Algorithm,
  salt_length: Option<u32>,
  #[from_v8(serde)]
  hash: Option<CryptoHash>,
  #[from_v8(serde)]
  named_curve: Option<CryptoNamedCurve>,
}

impl SignArg {
  pub(crate) fn new(
    algorithm: Algorithm,
    salt_length: Option<u32>,
    hash: Option<CryptoHash>,
    named_curve: Option<CryptoNamedCurve>,
  ) -> Self {
    Self {
      algorithm,
      salt_length,
      hash,
      named_curve,
    }
  }
}

/// Synchronous per-algorithm sign dispatch. Called from
/// [`crate::subtle_sign::run`] inside `spawn_blocking`; the op-layer
/// wrapper that previously fronted this for the JS shim has been
/// retired alongside the JS `SubtleCrypto.prototype.sign` body.
pub(crate) fn sign_key_sync(
  key: KeyData,
  args: SignArg,
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  {
    let algorithm = args.algorithm;

    let signature = match algorithm {
      Algorithm::RsassaPkcs1v15 => {
        use rsa::pkcs1v15::SigningKey;
        let private_key = RsaPrivateKey::from_pkcs1_der(&key.data)?;
        match args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)? {
          CryptoHash::Sha1 => {
            let signing_key = SigningKey::<Sha1>::new(private_key);
            signing_key.sign(data)
          }
          CryptoHash::Sha256 => {
            let signing_key = SigningKey::<Sha256>::new(private_key);
            signing_key.sign(data)
          }
          CryptoHash::Sha384 => {
            let signing_key = SigningKey::<Sha384>::new(private_key);
            signing_key.sign(data)
          }
          CryptoHash::Sha512 => {
            let signing_key = SigningKey::<Sha512>::new(private_key);
            signing_key.sign(data)
          }
          _ => return Err(CryptoError::UnsupportedAlgorithm),
        }
        .to_vec()
      }
      Algorithm::RsaPss => {
        let private_key = RsaPrivateKey::from_pkcs1_der(&key.data)?;

        let salt_len = args
          .salt_length
          .ok_or_else(|| CryptoError::MissingArgumentSaltLength)?
          as usize;

        let mut rng = OsRng;
        match args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)? {
          CryptoHash::Sha1 => {
            let signing_key = Pss::new_with_salt::<Sha1>(salt_len);
            let hashed = Sha1::digest(data);
            signing_key.sign(Some(&mut rng), &private_key, &hashed)?
          }
          CryptoHash::Sha256 => {
            let signing_key = Pss::new_with_salt::<Sha256>(salt_len);
            let hashed = Sha256::digest(data);
            signing_key.sign(Some(&mut rng), &private_key, &hashed)?
          }
          CryptoHash::Sha384 => {
            let signing_key = Pss::new_with_salt::<Sha384>(salt_len);
            let hashed = Sha384::digest(data);
            signing_key.sign(Some(&mut rng), &private_key, &hashed)?
          }
          CryptoHash::Sha512 => {
            let signing_key = Pss::new_with_salt::<Sha512>(salt_len);
            let hashed = Sha512::digest(data);
            signing_key.sign(Some(&mut rng), &private_key, &hashed)?
          }
          _ => return Err(CryptoError::UnsupportedAlgorithm),
        }
        .to_vec()
      }
      Algorithm::Ecdsa => {
        let hash = args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)?;
        let named_curve =
          args.named_curve.ok_or_else(JsErrorBox::not_supported)?;
        match named_curve {
          CryptoNamedCurve::P256 => {
            // Decode PKCS#8 private key.
            let secret_key = p256::SecretKey::from_pkcs8_der(&key.data)
              .map_err(|_| CryptoError::InvalidKeyFormat)?;
            let signing_key = P256SigningKey::from(secret_key);
            let prehash = match hash {
              CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
              CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
              CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
              CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
              _ => return Err(CryptoError::UnsupportedAlgorithm),
            };
            // Sign the prehashed message, producing a raw r||s signature.
            let signature: P256Signature =
              signing_key.sign_prehash(&prehash)?;
            signature.to_bytes().to_vec()
          }
          CryptoNamedCurve::P384 => {
            let secret_key = p384::SecretKey::from_pkcs8_der(&key.data)
              .map_err(|_| CryptoError::InvalidKeyFormat)?;
            let signing_key = P384SigningKey::from(secret_key);
            let prehash = match hash {
              CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
              CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
              CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
              CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
              _ => return Err(CryptoError::UnsupportedAlgorithm),
            };
            let signature: P384Signature =
              signing_key.sign_prehash(&prehash)?;
            signature.to_bytes().to_vec()
          }
          CryptoNamedCurve::P521 => {
            let secret_key = p521::SecretKey::from_pkcs8_der(&key.data)
              .map_err(|_| CryptoError::InvalidKeyFormat)?;
            let signing_key =
              P521SigningKey::from_bytes(&secret_key.to_bytes())
                .map_err(|_| CryptoError::InvalidKeyFormat)?;
            let prehash = match hash {
              CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
              CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
              CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
              CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
              _ => return Err(CryptoError::UnsupportedAlgorithm),
            };
            // P-521 field size is 66 bytes; bits2field requires at least
            // half that (33 bytes). Left-pad shorter hashes to meet the
            // minimum.
            let prehash = if prehash.len() < 33 {
              let mut padded = vec![0u8; 33 - prehash.len()];
              padded.extend_from_slice(&prehash);
              padded
            } else {
              prehash
            };
            let signature: P521Signature =
              signing_key.sign_prehash(&prehash)?;
            signature.to_bytes().to_vec()
          }
        }
      }
      Algorithm::Hmac => {
        let hash = args.hash.ok_or_else(JsErrorBox::not_supported)?;

        match hash {
          CryptoHash::Sha3_256 => {
            hmac_sign::<hmac::Hmac<Sha3_256>>(&key.data, data)?
          }
          CryptoHash::Sha3_384 => {
            hmac_sign::<hmac::Hmac<Sha3_384>>(&key.data, data)?
          }
          CryptoHash::Sha3_512 => {
            hmac_sign::<hmac::Hmac<Sha3_512>>(&key.data, data)?
          }
          _ => {
            let hash: HmacAlgorithm = hash.into();
            let key = HmacKey::new(hash, &key.data);
            let signature = aws_lc_rs::hmac::sign(&key, data);
            signature.as_ref().to_vec()
          }
        }
      }
      _ => return Err(CryptoError::UnsupportedAlgorithm),
    };

    Ok(signature)
  }
}

fn hmac_sign<M: hmac::Mac + hmac::digest::KeyInit>(
  key: &[u8],
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  let mut mac = <M as hmac::Mac>::new_from_slice(key)
    .map_err(|_| CryptoError::InvalidKeyLength)?;
  mac.update(data);
  Ok(mac.finalize().into_bytes().to_vec())
}

fn hmac_verify<M: hmac::Mac + hmac::digest::KeyInit>(
  key: &[u8],
  data: &[u8],
  signature: &[u8],
) -> Result<bool, CryptoError> {
  let mut mac = <M as hmac::Mac>::new_from_slice(key)
    .map_err(|_| CryptoError::InvalidKeyLength)?;
  mac.update(data);
  Ok(mac.verify_slice(signature).is_ok())
}

#[derive(deno_core::FromV8)]
pub struct VerifyArg {
  #[from_v8(serde)]
  algorithm: Algorithm,
  salt_length: Option<u32>,
  #[from_v8(serde)]
  hash: Option<CryptoHash>,
  signature: Uint8Array,
  #[from_v8(serde)]
  named_curve: Option<CryptoNamedCurve>,
}

impl VerifyArg {
  pub(crate) fn new(
    algorithm: Algorithm,
    salt_length: Option<u32>,
    hash: Option<CryptoHash>,
    signature: Vec<u8>,
    named_curve: Option<CryptoNamedCurve>,
  ) -> Self {
    Self {
      algorithm,
      salt_length,
      hash,
      signature: signature.into(),
      named_curve,
    }
  }
}

/// Synchronous per-algorithm verify dispatch. Called from
/// [`crate::subtle_verify::run`] inside `spawn_blocking`; the op-layer
/// wrapper that previously fronted this for the JS shim has been
/// retired alongside the JS `SubtleCrypto.prototype.verify` body.
pub(crate) fn verify_key_sync(
  key: KeyData,
  args: VerifyArg,
  data: &[u8],
) -> Result<bool, CryptoError> {
  {
    let algorithm = args.algorithm;

    let verification = match algorithm {
      Algorithm::RsassaPkcs1v15 => {
        use rsa::pkcs1v15::Signature;
        use rsa::pkcs1v15::VerifyingKey;
        let public_key = read_rsa_public_key(key)?;
        let signature: Signature = (&*args.signature).try_into()?;
        match args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)? {
          CryptoHash::Sha1 => {
            let verifying_key = VerifyingKey::<Sha1>::new(public_key);
            verifying_key.verify(data, &signature).is_ok()
          }
          CryptoHash::Sha256 => {
            let verifying_key = VerifyingKey::<Sha256>::new(public_key);
            verifying_key.verify(data, &signature).is_ok()
          }
          CryptoHash::Sha384 => {
            let verifying_key = VerifyingKey::<Sha384>::new(public_key);
            verifying_key.verify(data, &signature).is_ok()
          }
          CryptoHash::Sha512 => {
            let verifying_key = VerifyingKey::<Sha512>::new(public_key);
            verifying_key.verify(data, &signature).is_ok()
          }
          _ => return Err(CryptoError::UnsupportedAlgorithm),
        }
      }
      Algorithm::RsaPss => {
        let public_key = read_rsa_public_key(key)?;
        let signature = args.signature.as_ref();

        let salt_len = args
          .salt_length
          .ok_or_else(|| CryptoError::MissingArgumentSaltLength)?
          as usize;

        match args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)? {
          CryptoHash::Sha1 => {
            let pss = Pss::new_with_salt::<Sha1>(salt_len);
            let hashed = Sha1::digest(data);
            pss.verify(&public_key, &hashed, signature).is_ok()
          }
          CryptoHash::Sha256 => {
            let pss = Pss::new_with_salt::<Sha256>(salt_len);
            let hashed = Sha256::digest(data);
            pss.verify(&public_key, &hashed, signature).is_ok()
          }
          CryptoHash::Sha384 => {
            let pss = Pss::new_with_salt::<Sha384>(salt_len);
            let hashed = Sha384::digest(data);
            pss.verify(&public_key, &hashed, signature).is_ok()
          }
          CryptoHash::Sha512 => {
            let pss = Pss::new_with_salt::<Sha512>(salt_len);
            let hashed = Sha512::digest(data);
            pss.verify(&public_key, &hashed, signature).is_ok()
          }
          _ => return Err(CryptoError::UnsupportedAlgorithm),
        }
      }
      Algorithm::Hmac => {
        let hash = args.hash.ok_or_else(JsErrorBox::not_supported)?;
        match hash {
          CryptoHash::Sha3_256 => hmac_verify::<hmac::Hmac<Sha3_256>>(
            &key.data,
            data,
            &args.signature,
          )?,
          CryptoHash::Sha3_384 => hmac_verify::<hmac::Hmac<Sha3_384>>(
            &key.data,
            data,
            &args.signature,
          )?,
          CryptoHash::Sha3_512 => hmac_verify::<hmac::Hmac<Sha3_512>>(
            &key.data,
            data,
            &args.signature,
          )?,
          _ => {
            let hash: HmacAlgorithm = hash.into();
            let key = HmacKey::new(hash, &key.data);
            aws_lc_rs::hmac::verify(&key, data, &args.signature).is_ok()
          }
        }
      }
      Algorithm::Ecdsa => {
        let hash = args.hash.ok_or_else(|| CryptoError::MissingArgumentHash)?;
        let named_curve =
          args.named_curve.ok_or_else(JsErrorBox::not_supported)?;
        match named_curve {
          CryptoNamedCurve::P256 => {
            let verifying_key = match key.r#type {
              KeyType::Public => P256VerifyingKey::from_sec1_bytes(&key.data)
                .map_err(|_| CryptoError::InvalidKeyFormat)?,
              KeyType::Private => {
                let secret_key = p256::SecretKey::from_pkcs8_der(&key.data)
                  .map_err(|_| CryptoError::InvalidKeyFormat)?;
                let signing_key = P256SigningKey::from(secret_key);
                *signing_key.verifying_key()
              }
              _ => return Err(CryptoError::InvalidKeyFormat),
            };
            match P256Signature::from_slice(&args.signature) {
              Ok(signature) => {
                let prehash = match hash {
                  CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
                  CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
                  CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
                  CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
                  _ => return Err(CryptoError::UnsupportedAlgorithm),
                };
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
          CryptoNamedCurve::P384 => {
            let verifying_key = match key.r#type {
              KeyType::Public => P384VerifyingKey::from_sec1_bytes(&key.data)
                .map_err(|_| CryptoError::InvalidKeyFormat)?,
              KeyType::Private => {
                let secret_key = p384::SecretKey::from_pkcs8_der(&key.data)
                  .map_err(|_| CryptoError::InvalidKeyFormat)?;
                let signing_key = P384SigningKey::from(secret_key);
                *signing_key.verifying_key()
              }
              _ => return Err(CryptoError::InvalidKeyFormat),
            };
            match P384Signature::from_slice(&args.signature) {
              Ok(signature) => {
                let prehash = match hash {
                  CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
                  CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
                  CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
                  CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
                  _ => return Err(CryptoError::UnsupportedAlgorithm),
                };
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
          CryptoNamedCurve::P521 => {
            let verifying_key = match key.r#type {
              KeyType::Public => P521VerifyingKey::from_sec1_bytes(&key.data)
                .map_err(|_| CryptoError::InvalidKeyFormat)?,
              KeyType::Private => {
                let secret_key = p521::SecretKey::from_pkcs8_der(&key.data)
                  .map_err(|_| CryptoError::InvalidKeyFormat)?;
                // Construct inner ecdsa signing key to get verifying key
                let inner_signing_key =
                  ecdsa::SigningKey::<p521::NistP521>::from(secret_key);
                P521VerifyingKey::from(*inner_signing_key.verifying_key())
              }
              _ => return Err(CryptoError::InvalidKeyFormat),
            };
            match P521Signature::from_slice(&args.signature) {
              Ok(signature) => {
                let prehash = match hash {
                  CryptoHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
                  CryptoHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
                  CryptoHash::Sha384 => sha2::Sha384::digest(data).to_vec(),
                  CryptoHash::Sha512 => sha2::Sha512::digest(data).to_vec(),
                  _ => return Err(CryptoError::UnsupportedAlgorithm),
                };
                // P-521 field size is 66 bytes; bits2field requires at least
                // half that (33 bytes). Left-pad shorter hashes to meet the
                // minimum.
                let prehash = if prehash.len() < 33 {
                  let mut padded = vec![0u8; 33 - prehash.len()];
                  padded.extend_from_slice(&prehash);
                  padded
                } else {
                  prehash
                };
                verifying_key.verify_prehash(&prehash, &signature).is_ok()
              }
              _ => false,
            }
          }
        }
      }
      _ => return Err(CryptoError::UnsupportedAlgorithm),
    };

    Ok(verification)
  }
}

/// Synchronous body of the old `op_crypto_derive_bits`; called directly
/// from [`crate::subtle_derive_bits::run`] inside `spawn_blocking`. The
/// `salt` is `Some` for PBKDF2 / HKDF and `None` for ECDH.
#[allow(
  clippy::too_many_arguments,
  reason = "internal sync dispatch lifted out of op_crypto_derive_bits; \
  the per-algorithm DeriveKeyArg fields are unpacked rather than \
  reintroduced as a struct just for this lint"
)]
pub(crate) fn derive_bits_sync(
  key: KeyData,
  public_key: Option<KeyData>,
  algorithm: Algorithm,
  hash: Option<CryptoHash>,
  length: usize,
  iterations: Option<u32>,
  named_curve: Option<CryptoNamedCurve>,
  info: Option<Vec<u8>>,
  salt: Option<Vec<u8>>,
) -> Result<Vec<u8>, CryptoError> {
  match algorithm {
    Algorithm::Pbkdf2 => {
      let salt = salt.ok_or_else(JsErrorBox::not_supported)?;
      assert!(length > 0);
      assert!(length.is_multiple_of(8));

      let algorithm = match hash.ok_or_else(JsErrorBox::not_supported)? {
        CryptoHash::Sha1 => pbkdf2::PBKDF2_HMAC_SHA1,
        CryptoHash::Sha256 => pbkdf2::PBKDF2_HMAC_SHA256,
        CryptoHash::Sha384 => pbkdf2::PBKDF2_HMAC_SHA384,
        CryptoHash::Sha512 => pbkdf2::PBKDF2_HMAC_SHA512,
        _ => return Err(CryptoError::UnsupportedAlgorithm),
      };

      let iterations =
        NonZeroU32::new(iterations.ok_or_else(JsErrorBox::not_supported)?)
          .unwrap();
      let secret = key.data;
      let mut out = vec![0; length / 8];
      pbkdf2::derive(algorithm, iterations, &salt, &secret, &mut out);
      Ok(out)
    }
    Algorithm::Ecdh => {
      let named_curve =
        named_curve.ok_or(CryptoError::MissingArgumentNamedCurve)?;

      let public_key =
        public_key.ok_or(CryptoError::MissingArgumentPublicKey)?;

      match named_curve {
        CryptoNamedCurve::P256 => {
          let secret_key = p256::SecretKey::from_pkcs8_der(&key.data)
            .map_err(|_| CryptoError::DecodePrivateKey)?;

          let public_key = match public_key.r#type {
            KeyType::Private => {
              p256::SecretKey::from_pkcs8_der(&public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?
                .public_key()
            }
            KeyType::Public => {
              let point = p256::EncodedPoint::from_bytes(public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?;

              let pk = p256::PublicKey::from_encoded_point(&point);
              // pk is a constant time Option.
              if pk.is_some().into() {
                pk.unwrap()
              } else {
                return Err(CryptoError::DecodePrivateKey);
              }
            }
            _ => unreachable!(),
          };

          let shared_secret = p256::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_nonzero_scalar(),
            public_key.as_affine(),
          );

          // raw serialized x-coordinate of the computed point
          Ok(shared_secret.raw_secret_bytes().to_vec())
        }
        CryptoNamedCurve::P384 => {
          let secret_key = p384::SecretKey::from_pkcs8_der(&key.data)
            .map_err(|_| CryptoError::DecodePrivateKey)?;

          let public_key = match public_key.r#type {
            KeyType::Private => {
              p384::SecretKey::from_pkcs8_der(&public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?
                .public_key()
            }
            KeyType::Public => {
              let point = p384::EncodedPoint::from_bytes(public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?;

              let pk = p384::PublicKey::from_encoded_point(&point);
              // pk is a constant time Option.
              if pk.is_some().into() {
                pk.unwrap()
              } else {
                return Err(CryptoError::DecodePrivateKey);
              }
            }
            _ => unreachable!(),
          };

          let shared_secret = p384::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_nonzero_scalar(),
            public_key.as_affine(),
          );

          // raw serialized x-coordinate of the computed point
          Ok(shared_secret.raw_secret_bytes().to_vec())
        }
        CryptoNamedCurve::P521 => {
          let secret_key = p521::SecretKey::from_pkcs8_der(&key.data)
            .map_err(|_| CryptoError::DecodePrivateKey)?;

          let public_key = match public_key.r#type {
            KeyType::Private => {
              p521::SecretKey::from_pkcs8_der(&public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?
                .public_key()
            }
            KeyType::Public => {
              let point = p521::EncodedPoint::from_bytes(public_key.data)
                .map_err(|_| CryptoError::DecodePrivateKey)?;

              let pk = p521::PublicKey::from_encoded_point(&point);
              // pk is a constant time Option.
              if pk.is_some().into() {
                pk.unwrap()
              } else {
                return Err(CryptoError::DecodePrivateKey);
              }
            }
            _ => unreachable!(),
          };

          let shared_secret = p521::elliptic_curve::ecdh::diffie_hellman(
            secret_key.to_nonzero_scalar(),
            public_key.as_affine(),
          );

          // raw serialized x-coordinate of the computed point
          Ok(shared_secret.raw_secret_bytes().to_vec())
        }
      }
    }
    Algorithm::Hkdf => {
      let salt = salt.ok_or_else(JsErrorBox::not_supported)?;
      let algorithm = match hash.ok_or_else(JsErrorBox::not_supported)? {
        CryptoHash::Sha1 => hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY,
        CryptoHash::Sha256 => hkdf::HKDF_SHA256,
        CryptoHash::Sha384 => hkdf::HKDF_SHA384,
        CryptoHash::Sha512 => hkdf::HKDF_SHA512,
        _ => return Err(CryptoError::UnsupportedAlgorithm),
      };

      let info = info.ok_or(CryptoError::MissingArgumentInfo)?;
      let secret = key.data;
      let length = length / 8;

      let salt = hkdf::Salt::new(algorithm, &salt);
      let prk = salt.extract(&secret);
      let info_slice: &[&[u8]] = &[&info];
      let okm = prk
        .expand(info_slice, HkdfOutput(length))
        .map_err(|_e| CryptoError::HKDFLengthTooLarge)?;
      let mut r = vec![0u8; length];
      okm.fill(&mut r)?;
      Ok(r)
    }
    _ => Err(CryptoError::UnsupportedAlgorithm),
  }
}

fn read_rsa_public_key(key_data: KeyData) -> Result<RsaPublicKey, CryptoError> {
  let public_key = match key_data.r#type {
    KeyType::Private => {
      RsaPrivateKey::from_pkcs1_der(&key_data.data)?.to_public_key()
    }
    KeyType::Public => RsaPublicKey::from_pkcs1_der(&key_data.data)?,
    KeyType::Secret => unreachable!("unexpected KeyType::Secret"),
  };
  Ok(public_key)
}

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn fast_uuid_v4(bytes: &mut [u8; 16]) -> String {
  // Set UUID version to 4 and variant to 1.
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  let buf = [
    HEX_CHARS[(bytes[0] >> 4) as usize],
    HEX_CHARS[(bytes[0] & 0x0f) as usize],
    HEX_CHARS[(bytes[1] >> 4) as usize],
    HEX_CHARS[(bytes[1] & 0x0f) as usize],
    HEX_CHARS[(bytes[2] >> 4) as usize],
    HEX_CHARS[(bytes[2] & 0x0f) as usize],
    HEX_CHARS[(bytes[3] >> 4) as usize],
    HEX_CHARS[(bytes[3] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[4] >> 4) as usize],
    HEX_CHARS[(bytes[4] & 0x0f) as usize],
    HEX_CHARS[(bytes[5] >> 4) as usize],
    HEX_CHARS[(bytes[5] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[6] >> 4) as usize],
    HEX_CHARS[(bytes[6] & 0x0f) as usize],
    HEX_CHARS[(bytes[7] >> 4) as usize],
    HEX_CHARS[(bytes[7] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[8] >> 4) as usize],
    HEX_CHARS[(bytes[8] & 0x0f) as usize],
    HEX_CHARS[(bytes[9] >> 4) as usize],
    HEX_CHARS[(bytes[9] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[10] >> 4) as usize],
    HEX_CHARS[(bytes[10] & 0x0f) as usize],
    HEX_CHARS[(bytes[11] >> 4) as usize],
    HEX_CHARS[(bytes[11] & 0x0f) as usize],
    HEX_CHARS[(bytes[12] >> 4) as usize],
    HEX_CHARS[(bytes[12] & 0x0f) as usize],
    HEX_CHARS[(bytes[13] >> 4) as usize],
    HEX_CHARS[(bytes[13] & 0x0f) as usize],
    HEX_CHARS[(bytes[14] >> 4) as usize],
    HEX_CHARS[(bytes[14] & 0x0f) as usize],
    HEX_CHARS[(bytes[15] >> 4) as usize],
    HEX_CHARS[(bytes[15] & 0x0f) as usize],
  ];

  // Safety: the buffer is all valid UTF-8.
  unsafe { String::from_utf8_unchecked(buf.to_vec()) }
}

#[test]
fn test_fast_uuid_v4_correctness() {
  let mut rng = thread_rng();
  let mut bytes = [0u8; 16];
  rng.fill(&mut bytes);
  let uuid = fast_uuid_v4(&mut bytes.clone());
  let uuid_lib = uuid::Builder::from_bytes(bytes)
    .set_variant(uuid::Variant::RFC4122)
    .set_version(uuid::Version::Random)
    .as_uuid()
    .to_string();
  assert_eq!(uuid, uuid_lib);
}
