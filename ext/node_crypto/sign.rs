// Copyright 2018-2026 the Deno authors. MIT license.
use core::ops::Add;

use ecdsa::der::MaxOverhead;
use ecdsa::der::MaxSize;
use elliptic_curve::FieldBytesSize;
use elliptic_curve::generic_array::ArrayLength;
use rand::rngs::OsRng;
use rsa::signature::hazmat::PrehashSigner as _;
use rsa::signature::hazmat::PrehashVerifier as _;
use rsa::traits::PublicKeyParts as _;
use rsa::traits::SignatureScheme as _;
use spki::der::Decode;

use super::keys::AsymmetricPrivateKey;
use super::keys::AsymmetricPublicKey;
use super::keys::EcPrivateKey;
use super::keys::EcPublicKey;
use super::keys::KeyObjectHandle;
use super::keys::RsaPssHashAlgorithm;
use crate::digest::match_fixed_digest;
use crate::digest::match_fixed_digest_with_oid;

/// OpenSSL RSA_PKCS1_PADDING constant value.
const RSA_PKCS1_PADDING: u32 = 1;
/// OpenSSL RSA_PKCS1_PSS_PADDING constant value.
const RSA_PKCS1_PSS_PADDING: u32 = 6;

fn dsa_signature<C: elliptic_curve::PrimeCurve>(
  encoding: u32,
  signature: ecdsa::Signature<C>,
) -> Result<Box<[u8]>, KeyObjectHandlePrehashedSignAndVerifyError>
where
  MaxSize<C>: ArrayLength<u8>,
  <FieldBytesSize<C> as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
  match encoding {
    // DER
    0 => Ok(signature.to_der().to_bytes().to_vec().into_boxed_slice()),
    // IEEE P1363
    1 => Ok(signature.to_bytes().to_vec().into_boxed_slice()),
    _ => Err(
      KeyObjectHandlePrehashedSignAndVerifyError::InvalidDsaSignatureEncoding,
    ),
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum KeyObjectHandlePrehashedSignAndVerifyError {
  #[error("invalid DSA signature encoding")]
  InvalidDsaSignatureEncoding,
  #[error("key is not a private key")]
  KeyIsNotPrivate,
  #[error("digest not allowed for RSA signature: {0}")]
  DigestNotAllowedForRsaSignature(String),
  #[class(generic)]
  #[error("failed to sign digest with RSA")]
  FailedToSignDigestWithRsa,
  #[class(generic)]
  #[error("digest too big for rsa key")]
  DigestTooBigForRsaKey,
  #[error("digest not allowed for RSA-PSS signature: {0}")]
  DigestNotAllowedForRsaPssSignature(String),
  #[class(generic)]
  #[error("failed to sign digest with RSA-PSS")]
  FailedToSignDigestWithRsaPss,
  #[error("failed to sign digest with DSA")]
  FailedToSignDigestWithDsa,
  #[error(
    "rsa-pss with different mf1 hash algorithm and hash algorithm is not supported"
  )]
  RsaPssHashAlgorithmUnsupported,
  #[error(
    "private key does not allow {actual} to be used, expected {expected}"
  )]
  PrivateKeyDisallowsUsage { actual: String, expected: String },
  #[error("failed to sign digest")]
  FailedToSignDigest,
  #[error("x25519 key cannot be used for signing")]
  X25519KeyCannotBeUsedForSigning,
  #[error("Ed25519 key cannot be used for prehashed signing")]
  Ed25519KeyCannotBeUsedForPrehashedSigning,
  #[error("DH key cannot be used for signing")]
  DhKeyCannotBeUsedForSigning,
  #[error("key is not a public or private key")]
  KeyIsNotPublicOrPrivate,
  #[error("Invalid DSA signature")]
  InvalidDsaSignature,
  #[error("x25519 key cannot be used for verification")]
  X25519KeyCannotBeUsedForVerification,
  #[error("Ed25519 key cannot be used for prehashed verification")]
  Ed25519KeyCannotBeUsedForPrehashedVerification,
  #[error("DH key cannot be used for verification")]
  DhKeyCannotBeUsedForVerification,
  #[class(generic)]
  #[error("illegal or unsupported padding mode")]
  IllegalOrUnsupportedPaddingMode,
}

/// Constructs a PSS scheme for the given digest type and optional salt length.
/// Used by both sign and verify operations on RSA keys with PSS padding.
///
/// When `key_size_bits` is provided and `pss_salt_length` is `None`,
/// the default salt length is max (key_bytes - hash_len - 2), matching
/// Node.js's documented default of `RSA_PSS_SALTLEN_MAX_SIGN`.
/// When `key_size_bits` is `None` (verify path), the default salt length
/// is the digest length.
/// OpenSSL RSA_PSS_SALTLEN_DIGEST: use digest length as salt length.
const RSA_PSS_SALTLEN_DIGEST: i32 = -1;
/// OpenSSL RSA_PSS_SALTLEN_MAX_SIGN: use maximum possible salt length.
const RSA_PSS_SALTLEN_MAX_SIGN: i32 = -2;

/// Resolves the effective salt length for PSS operations.
///
/// Handles Node.js special constants:
/// - `-1` (RSA_PSS_SALTLEN_DIGEST): use the digest output size
/// - `-2` (RSA_PSS_SALTLEN_MAX_SIGN / RSA_PSS_SALTLEN_AUTO): use maximum
///   possible salt length (key_bytes - hash_len - 2)
/// - `None`: defaults to max salt when `key_size_bits` is provided (sign),
///   or digest length otherwise (verify)
/// - Positive values: use as-is
fn resolve_pss_salt_length<D: digest::Digest>(
  pss_salt_length: Option<i32>,
  key_size_bits: Option<usize>,
) -> usize {
  match pss_salt_length {
    Some(RSA_PSS_SALTLEN_DIGEST) => <D as digest::Digest>::output_size(),
    Some(RSA_PSS_SALTLEN_MAX_SIGN) => {
      let hash_len = <D as digest::Digest>::output_size();
      if let Some(key_bits) = key_size_bits {
        let key_bytes = key_bits / 8;
        key_bytes.saturating_sub(hash_len + 2)
      } else {
        hash_len
      }
    }
    Some(len) if len >= 0 => len as usize,
    Some(_) => <D as digest::Digest>::output_size(), // Unknown negative, fallback to digest length
    None => {
      if let Some(key_bits) = key_size_bits {
        // Default to max salt length for signing (RSA_PSS_SALTLEN_MAX_SIGN)
        let key_bytes = key_bits / 8;
        let hash_len = <D as digest::Digest>::output_size();
        key_bytes.saturating_sub(hash_len + 2)
      } else {
        <D as digest::Digest>::output_size()
      }
    }
  }
}

fn new_pss_scheme(
  digest_type: &str,
  pss_salt_length: Option<i32>,
  key_size_bits: Option<usize>,
) -> Result<rsa::pss::Pss, KeyObjectHandlePrehashedSignAndVerifyError> {
  let pss = match_fixed_digest_with_oid!(
    digest_type,
    fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
      let _: Option<RsaPssHashAlgorithm> = algorithm;
      let salt_len = resolve_pss_salt_length::<D>(pss_salt_length, key_size_bits);
      rsa::pss::Pss::new_with_salt::<D>(salt_len)
    },
    _ => {
      return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaPssSignature(digest_type.to_string()));
    }
  );
  Ok(pss)
}

impl KeyObjectHandle {
  pub fn sign_prehashed(
    &self,
    digest_type: &str,
    digest: &[u8],
    pss_salt_length: Option<i32>,
    padding: Option<u32>,
    dsa_signature_encoding: u32,
  ) -> Result<Box<[u8]>, KeyObjectHandlePrehashedSignAndVerifyError> {
    let private_key = self
      .as_private_key()
      .ok_or(KeyObjectHandlePrehashedSignAndVerifyError::KeyIsNotPrivate)?;

    match private_key {
      AsymmetricPrivateKey::Rsa(key) => {
        if padding == Some(RSA_PKCS1_PSS_PADDING) {
          let pss = new_pss_scheme(
            digest_type,
            pss_salt_length,
            Some(key.n().bits()),
          )?;
          let signature = pss
            .sign(Some(&mut OsRng), key, digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigestWithRsaPss)?;
          return Ok(signature.into());
        }

        let signer = if digest_type == "md5-sha1" {
          rsa::pkcs1v15::Pkcs1v15Sign::new_unprefixed()
        } else {
          match_fixed_digest_with_oid!(
            digest_type,
            fn <D>() {
              rsa::pkcs1v15::Pkcs1v15Sign::new::<D>()
            },
            _ => {
              return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaSignature(digest_type.to_string()))
            }
          )
        };

        let signature = signer.sign(Some(&mut OsRng), key, digest).map_err(
          |e| {
            if e == rsa::Error::MessageTooLong {
              KeyObjectHandlePrehashedSignAndVerifyError::DigestTooBigForRsaKey
            } else {
              KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigestWithRsa
            }
          },
        )?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::RsaPss(key) => {
        if padding == Some(RSA_PKCS1_PADDING) {
          return Err(KeyObjectHandlePrehashedSignAndVerifyError::IllegalOrUnsupportedPaddingMode);
        }
        let mut hash_algorithm = None;
        let mut salt_length = None;
        if let Some(details) = &key.details {
          if details.hash_algorithm != details.mf1_hash_algorithm {
            return Err(KeyObjectHandlePrehashedSignAndVerifyError::RsaPssHashAlgorithmUnsupported);
          }
          hash_algorithm = Some(details.hash_algorithm);
          salt_length = Some(details.salt_length as usize);
        }
        let pss = match_fixed_digest_with_oid!(
          digest_type,
          fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
            if let Some(hash_algorithm) = hash_algorithm.take()
              && Some(hash_algorithm) != algorithm {
                return Err(KeyObjectHandlePrehashedSignAndVerifyError::PrivateKeyDisallowsUsage {
                  actual: digest_type.to_string(),
                  expected: hash_algorithm.as_str().to_string(),
                });
              }
            // Resolve salt length: explicit pss_salt_length takes priority,
            // then key details, then default (digest length)
            let resolved = if pss_salt_length.is_some() {
              resolve_pss_salt_length::<D>(pss_salt_length, Some(key.key.n().bits()))
            } else if let Some(sl) = salt_length {
              sl
            } else {
              <D as digest::Digest>::output_size()
            };
            rsa::pss::Pss::new_with_salt::<D>(resolved)
          },
          _ => {
            return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaPssSignature(digest_type.to_string()));
          }
        );
        let signature = pss
          .sign(Some(&mut OsRng), &key.key, digest)
          .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigestWithRsaPss)?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::Dsa(key) => {
        let res = match_fixed_digest!(
          digest_type,
          fn <D>() {
            key.sign_prehashed_rfc6979::<D>(digest)
          },
          _ => {
            return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaSignature(digest_type.to_string()))
          }
        );

        let signature =
          res.map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigestWithDsa)?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::Ec(key) => match key {
        EcPrivateKey::P224(key) => {
          let signing_key = p224::ecdsa::SigningKey::from(key);
          let signature: p224::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;

          dsa_signature(dsa_signature_encoding, signature)
        }
        EcPrivateKey::P256(key) => {
          let signing_key = p256::ecdsa::SigningKey::from(key);
          let signature: p256::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;

          dsa_signature(dsa_signature_encoding, signature)
        }
        EcPrivateKey::P384(key) => {
          let signing_key = p384::ecdsa::SigningKey::from(key);
          let signature: p384::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;

          dsa_signature(dsa_signature_encoding, signature)
        }
        EcPrivateKey::P521(key) => {
          let signing_key = p521::ecdsa::SigningKey::from_bytes(&key.to_bytes())
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;
          let signature: p521::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;

          dsa_signature(dsa_signature_encoding, signature)
        }
        EcPrivateKey::Secp256k1(key) => {
          let signing_key = k256::ecdsa::SigningKey::from(key);
          let signature: k256::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| KeyObjectHandlePrehashedSignAndVerifyError::FailedToSignDigest)?;

          dsa_signature(dsa_signature_encoding, signature)
        }
      },
      AsymmetricPrivateKey::X25519(_) | AsymmetricPrivateKey::X448(_) => {
        Err(KeyObjectHandlePrehashedSignAndVerifyError::X25519KeyCannotBeUsedForSigning)
      }
      AsymmetricPrivateKey::Ed25519(_) | AsymmetricPrivateKey::Ed448(_) => Err(KeyObjectHandlePrehashedSignAndVerifyError::Ed25519KeyCannotBeUsedForPrehashedSigning),
      AsymmetricPrivateKey::Dh(_) => {
        Err(KeyObjectHandlePrehashedSignAndVerifyError::DhKeyCannotBeUsedForSigning)
      }
    }
  }

  pub fn verify_prehashed(
    &self,
    digest_type: &str,
    digest: &[u8],
    signature: &[u8],
    pss_salt_length: Option<i32>,
    padding: Option<u32>,
    dsa_signature_encoding: u32,
  ) -> Result<bool, KeyObjectHandlePrehashedSignAndVerifyError> {
    let public_key = self.as_public_key().ok_or(
      KeyObjectHandlePrehashedSignAndVerifyError::KeyIsNotPublicOrPrivate,
    )?;

    match &*public_key {
      AsymmetricPublicKey::Rsa(key) => {
        if padding == Some(RSA_PKCS1_PSS_PADDING) {
          let pss = new_pss_scheme(
            digest_type,
            pss_salt_length,
            Some(key.n().bits()),
          )?;
          return Ok(pss.verify(key, digest, signature).is_ok());
        }

        let signer = if digest_type == "md5-sha1" {
          rsa::pkcs1v15::Pkcs1v15Sign::new_unprefixed()
        } else {
          match_fixed_digest_with_oid!(
            digest_type,
            fn <D>() {
              rsa::pkcs1v15::Pkcs1v15Sign::new::<D>()
            },
            _ => {
              return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaSignature(digest_type.to_string()))
            }
          )
        };

        Ok(signer.verify(key, digest, signature).is_ok())
      }
      AsymmetricPublicKey::RsaPss(key) => {
        if padding == Some(RSA_PKCS1_PADDING) {
          return Err(KeyObjectHandlePrehashedSignAndVerifyError::IllegalOrUnsupportedPaddingMode);
        }
        let mut hash_algorithm = None;
        let mut salt_length = None;
        if let Some(details) = &key.details {
          if details.hash_algorithm != details.mf1_hash_algorithm {
            return Err(KeyObjectHandlePrehashedSignAndVerifyError::RsaPssHashAlgorithmUnsupported);
          }
          hash_algorithm = Some(details.hash_algorithm);
          salt_length = Some(details.salt_length as usize);
        }
        let pss = match_fixed_digest_with_oid!(
          digest_type,
          fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
            if let Some(hash_algorithm) = hash_algorithm.take()
              && Some(hash_algorithm) != algorithm {
                return Err(KeyObjectHandlePrehashedSignAndVerifyError::PrivateKeyDisallowsUsage {
                  actual: digest_type.to_string(),
                  expected: hash_algorithm.as_str().to_string(),
                });
              }
            let resolved = if pss_salt_length.is_some() {
              resolve_pss_salt_length::<D>(pss_salt_length, Some(key.key.n().bits()))
            } else if let Some(sl) = salt_length {
              sl
            } else {
              <D as digest::Digest>::output_size()
            };
            rsa::pss::Pss::new_with_salt::<D>(resolved)
          },
          _ => {
            return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaPssSignature(digest_type.to_string()));
          }
        );
        Ok(pss.verify(&key.key, digest, signature).is_ok())
      }
      AsymmetricPublicKey::Dsa(key) => {
        let Ok(signature) = dsa::Signature::from_der(signature) else {
          return Ok(false);
        };
        Ok(key.verify_prehash(digest, &signature).is_ok())
      }
      AsymmetricPublicKey::Ec(key) => match key {
        EcPublicKey::P224(key) => {
          let verifying_key = p224::ecdsa::VerifyingKey::from(key);
          let signature = if dsa_signature_encoding == 0 {
            p224::ecdsa::Signature::from_der(signature)
          } else {
            p224::ecdsa::Signature::from_bytes(signature.into())
          };
          let Ok(signature) = signature else {
            return Ok(false);
          };
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::P256(key) => {
          let verifying_key = p256::ecdsa::VerifyingKey::from(key);
          let signature = if dsa_signature_encoding == 0 {
            p256::ecdsa::Signature::from_der(signature)
          } else {
            p256::ecdsa::Signature::from_bytes(signature.into())
          };
          let Ok(signature) = signature else {
            return Ok(false);
          };
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::P384(key) => {
          let verifying_key = p384::ecdsa::VerifyingKey::from(key);
          let signature = if dsa_signature_encoding == 0 {
            p384::ecdsa::Signature::from_der(signature)
          } else {
            p384::ecdsa::Signature::from_bytes(signature.into())
          };
          let Ok(signature) = signature else {
            return Ok(false);
          };
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::P521(key) => {
          let Ok(verifying_key) = p521::ecdsa::VerifyingKey::from_affine(*key.as_affine()) else {
            return Ok(false);
          };
          let signature = if dsa_signature_encoding == 0 {
            p521::ecdsa::Signature::from_der(signature)
          } else {
            p521::ecdsa::Signature::from_bytes(signature.into())
          };
          let Ok(signature) = signature else {
            return Ok(false);
          };
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::Secp256k1(key) => {
          let verifying_key = k256::ecdsa::VerifyingKey::from(key);
          let signature = if dsa_signature_encoding == 0 {
            k256::ecdsa::Signature::from_der(signature)
          } else {
            k256::ecdsa::Signature::from_bytes(signature.into())
          };
          let Ok(signature) = signature else {
            return Ok(false);
          };
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
      },
      AsymmetricPublicKey::X25519(_) | AsymmetricPublicKey::X448(_) => {
        Err(KeyObjectHandlePrehashedSignAndVerifyError::X25519KeyCannotBeUsedForVerification)
      }
      AsymmetricPublicKey::Ed25519(_) | AsymmetricPublicKey::Ed448(_) => Err(KeyObjectHandlePrehashedSignAndVerifyError::Ed25519KeyCannotBeUsedForPrehashedVerification),
      AsymmetricPublicKey::Dh(_) => {
        Err(KeyObjectHandlePrehashedSignAndVerifyError::DhKeyCannotBeUsedForVerification)
      }
    }
  }
}
