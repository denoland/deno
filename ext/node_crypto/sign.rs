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
/// OpenSSL RSA_PKCS1_OAEP_PADDING constant value.
const RSA_PKCS1_OAEP_PADDING: u32 = 4;

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

/// Encode a DSA signature to IEEE P1363 format: r || s, each zero-padded to
/// q_len bytes. Spec: https://www.w3.org/TR/WebCryptoAPI/#convert-an-ecdsa-signature
fn dsa_sig_to_p1363(sig: &dsa::Signature, q_len: usize) -> Box<[u8]> {
  let mut result = vec![0u8; q_len * 2];
  let r_bytes = sig.r().to_bytes_be();
  let s_bytes = sig.s().to_bytes_be();
  if r_bytes.len() <= q_len {
    result[q_len - r_bytes.len()..q_len].copy_from_slice(&r_bytes);
  }
  if s_bytes.len() <= q_len {
    result[2 * q_len - s_bytes.len()..2 * q_len].copy_from_slice(&s_bytes);
  }
  result.into_boxed_slice()
}

/// Decode a DSA signature from IEEE P1363 format: r || s, each of q_len bytes.
/// Spec: https://www.w3.org/TR/WebCryptoAPI/#convert-an-ecdsa-signature
fn dsa_sig_from_p1363(
  bytes: &[u8],
  q_len: usize,
) -> Option<dsa::Signature> {
  if bytes.len() != 2 * q_len {
    return None;
  }
  let r = dsa::BigUint::from_bytes_be(&bytes[..q_len]);
  let s = dsa::BigUint::from_bytes_be(&bytes[q_len..]);
  dsa::Signature::from_components(r, s).ok()
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
  #[property("code" = "ERR_OSSL_RSA_DIGEST_TOO_BIG_FOR_RSA_KEY")]
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
  #[class(generic)]
  #[error("{actual} digest not allowed")]
  PrivateKeyDisallowsUsage { actual: String, expected: String },
  #[class(generic)]
  #[error("pss saltlen too small")]
  PssSaltLenTooSmall,
  #[error("failed to sign digest")]
  FailedToSignDigest,
  #[class(generic)]
  #[error("operation not supported for this keytype")]
  #[property("code" = "ERR_OSSL_EVP_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE")]
  X25519KeyCannotBeUsedForSigning,
  #[class(generic)]
  #[error("Unsupported crypto operation")]
  #[property("code" = "ERR_CRYPTO_UNSUPPORTED_OPERATION")]
  Ed25519KeyCannotBeUsedForPrehashedSigning,
  #[class(generic)]
  #[error("operation not supported for this keytype")]
  #[property("code" = "ERR_OSSL_EVP_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE")]
  DhKeyCannotBeUsedForSigning,
  #[error("key is not a public or private key")]
  KeyIsNotPublicOrPrivate,
  #[error("Invalid DSA signature")]
  InvalidDsaSignature,
  #[class(generic)]
  #[error("operation not supported for this keytype")]
  #[property("code" = "ERR_OSSL_EVP_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE")]
  X25519KeyCannotBeUsedForVerification,
  #[class(generic)]
  #[error("Unsupported crypto operation")]
  #[property("code" = "ERR_CRYPTO_UNSUPPORTED_OPERATION")]
  Ed25519KeyCannotBeUsedForPrehashedVerification,
  #[class(generic)]
  #[error("operation not supported for this keytype")]
  #[property("code" = "ERR_OSSL_EVP_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE")]
  DhKeyCannotBeUsedForVerification,
  #[class(generic)]
  #[error(
    "error:1C8000A5:Provider routines::illegal or unsupported padding mode"
  )]
  #[property("code" = "ERR_OSSL_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE")]
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

/// Recover the PSS salt length from a signature using AUTO detection.
/// Performs the RSA raw operation (sig^e mod n) to get the encoded message,
/// then parses the PSS structure to find the salt length.
fn recover_pss_salt_len<D>(key: &rsa::RsaPublicKey, sig: &[u8]) -> Option<usize>
where
  D: digest::Digest,
{
  use rsa::traits::PublicKeyParts;

  let h_len = <D as digest::Digest>::output_size();
  let key_bits = key.n().bits();
  let em_bits = key_bits - 1;
  let em_len = em_bits.div_ceil(8);
  let key_len = key_bits.div_ceil(8);

  // Compute em = sig^e mod n (raw RSA public key operation)
  let sig_bn = rsa::BigUint::from_bytes_be(sig);
  let em_bn = rsa::hazmat::rsa_encrypt(key, &sig_bn).ok()?;
  let em_be = em_bn.to_bytes_be();

  // Pad em to key_len bytes (big-endian, left-pad with zeros)
  if em_be.len() > key_len {
    return None;
  }
  let mut em_padded = vec![0u8; key_len];
  em_padded[key_len - em_be.len()..].copy_from_slice(&em_be);

  // Take the relevant em_len bytes from the right
  let em = &mut em_padded[key_len - em_len..];

  // Check 0xBC trailer
  if em[em_len - 1] != 0xBC {
    return None;
  }
  if em_len < h_len + 2 {
    return None;
  }

  let db_len = em_len - h_len - 1;

  // H is at em[db_len..db_len+h_len]
  let h = em[db_len..db_len + h_len].to_vec();

  // Unmask maskedDB using MGF1(H, db_len)
  let masked_db = &mut em[..db_len];
  let mut counter: u32 = 0;
  let mut pos = 0;
  while pos < db_len {
    let mut hasher = D::new();
    hasher.update(h.as_slice());
    hasher.update(counter.to_be_bytes());
    let hash_out = hasher.finalize();
    let hash_bytes: &[u8] = hash_out.as_ref();
    let end = (pos + h_len).min(db_len);
    for j in pos..end {
      masked_db[j] ^= hash_bytes[j - pos];
    }
    pos += h_len;
    counter += 1;
  }

  // Clear top bits: db[0] &= 0xFF >> (8*em_len - em_bits)
  let db = masked_db;
  let top_bits_to_clear = 8 * em_len - em_bits;
  if top_bits_to_clear < 8 {
    db[0] &= 0xFF_u8 >> top_bits_to_clear;
  }

  // Find the 0x01 separator byte: DB = PS (zeros) || 0x01 || salt
  let mut salt_start = None;
  for (i, &b) in db.iter().enumerate() {
    if b == 0x01 {
      salt_start = Some(i + 1);
      break;
    } else if b != 0x00 {
      return None; // Invalid DB structure
    }
  }

  let salt_start = salt_start?;
  Some(db_len - salt_start)
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
        if padding == Some(RSA_PKCS1_OAEP_PADDING) {
          return Err(KeyObjectHandlePrehashedSignAndVerifyError::IllegalOrUnsupportedPaddingMode);
        }

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
          // Note: the rsa crate's PSS uses the same hash for the message and
          // for MGF1. We honor the key's enforced message hash but fall back
          // to using it for MGF1 too, even if the key specifies a distinct
          // mgf1 hash. RFC 4055 / PKCS#1 v2.1 permits this combination but
          // signatures produced here will not be byte-compatible with those
          // produced by OpenSSL when mgf1 hash != message hash.
          hash_algorithm = Some(details.hash_algorithm);
          salt_length = Some(details.salt_length as usize);
        }
        // Match Node.js / OpenSSL ordering for RSA-PSS signing: validate
        // the requested salt length against the key's enforced minimum
        // before validating the digest itself.
        if let Some(min_salt) = salt_length
          && let Some(requested) = pss_salt_length
          && requested >= 0
          && (requested as usize) < min_salt
        {
          return Err(
            KeyObjectHandlePrehashedSignAndVerifyError::PssSaltLenTooSmall,
          );
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
              let r = resolve_pss_salt_length::<D>(pss_salt_length, Some(key.key.n().bits()));
              // Enforce key's minimum salt length
              if let Some(min_salt) = salt_length && r < min_salt {
                return Err(KeyObjectHandlePrehashedSignAndVerifyError::PssSaltLenTooSmall);
              }
              r
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

        if dsa_signature_encoding == 0 {
          Ok(signature.into())
        } else {
          let q_len =
            key.verifying_key().components().q().bits().div_ceil(8);
          Ok(dsa_sig_to_p1363(&signature, q_len))
        }
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
          // AUTO mode: when pss_salt_length == -2 (RSA_PSS_SALTLEN_AUTO),
          // recover the actual salt length from the signature structure.
          let effective_salt_length = if pss_salt_length == Some(RSA_PSS_SALTLEN_MAX_SIGN) {
            let recovered = match_fixed_digest_with_oid!(
              digest_type,
              fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
                let _: Option<RsaPssHashAlgorithm> = algorithm;
                recover_pss_salt_len::<D>(key, signature)
              },
              _ => {
                return Err(KeyObjectHandlePrehashedSignAndVerifyError::DigestNotAllowedForRsaPssSignature(digest_type.to_string()));
              }
            );
            match recovered {
              Some(s) => Some(s as i32),
              None => return Ok(false),
            }
          } else {
            pss_salt_length
          };

          let pss = new_pss_scheme(
            digest_type,
            effective_salt_length,
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
          // Mirror the sign path: when mgf1 hash != message hash, fall back
          // to using the message hash for MGF1 too. Round-trips with our own
          // sign implementation but is not byte-compatible with OpenSSL.
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
        let sig = if dsa_signature_encoding == 0 {
          // DER encoding
          let Ok(sig) = dsa::Signature::from_der(signature) else {
            return Ok(false);
          };
          sig
        } else {
          let q_len = key.components().q().bits().div_ceil(8);
          let Some(sig) = dsa_sig_from_p1363(signature, q_len) else {
            return Ok(false);
          };
          sig
        };
        Ok(key.verify_prehash(digest, &sig).is_ok())
      }
      AsymmetricPublicKey::Ec(key) => match key {
        EcPublicKey::P224(key) => {
          let verifying_key = p224::ecdsa::VerifyingKey::from(key);
          let signature = if dsa_signature_encoding == 0 {
            p224::ecdsa::Signature::from_der(signature)
          } else {
            p224::ecdsa::Signature::try_from(signature)
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
            p256::ecdsa::Signature::try_from(signature)
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
            p384::ecdsa::Signature::try_from(signature)
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
            p521::ecdsa::Signature::try_from(signature)
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
            k256::ecdsa::Signature::try_from(signature)
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
