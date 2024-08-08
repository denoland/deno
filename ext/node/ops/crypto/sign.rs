// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use rand::rngs::OsRng;
use rsa::signature::hazmat::PrehashSigner as _;
use rsa::signature::hazmat::PrehashVerifier as _;
use rsa::traits::SignatureScheme as _;
use spki::der::Decode;

use crate::ops::crypto::digest::match_fixed_digest;
use crate::ops::crypto::digest::match_fixed_digest_with_oid;

use super::keys::AsymmetricPrivateKey;
use super::keys::AsymmetricPublicKey;
use super::keys::EcPrivateKey;
use super::keys::EcPublicKey;
use super::keys::KeyObjectHandle;
use super::keys::RsaPssHashAlgorithm;

impl KeyObjectHandle {
  pub fn sign_prehashed(
    &self,
    digest_type: &str,
    digest: &[u8],
  ) -> Result<Box<[u8]>, AnyError> {
    let private_key = self
      .as_private_key()
      .ok_or_else(|| type_error("key is not a private key"))?;

    match private_key {
      AsymmetricPrivateKey::Rsa(key) => {
        let signer = if digest_type == "md5-sha1" {
          rsa::pkcs1v15::Pkcs1v15Sign::new_unprefixed()
        } else {
          match_fixed_digest_with_oid!(
            digest_type,
            fn <D>() {
              rsa::pkcs1v15::Pkcs1v15Sign::new::<D>()
            },
            _ => {
              return Err(type_error(format!(
                "digest not allowed for RSA signature: {}",
                digest_type
              )))
            }
          )
        };

        let signature = signer
          .sign(Some(&mut OsRng), key, digest)
          .map_err(|_| generic_error("failed to sign digest with RSA"))?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::RsaPss(key) => {
        let mut hash_algorithm = None;
        let mut salt_length = None;
        match &key.details {
          Some(details) => {
            if details.hash_algorithm != details.mf1_hash_algorithm {
              return Err(type_error(
                "rsa-pss with different mf1 hash algorithm and hash algorithm is not supported",
              ));
            }
            hash_algorithm = Some(details.hash_algorithm);
            salt_length = Some(details.salt_length as usize);
          }
          None => {}
        };
        let pss = match_fixed_digest_with_oid!(
          digest_type,
          fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
            if let Some(hash_algorithm) = hash_algorithm.take() {
              if Some(hash_algorithm) != algorithm {
                return Err(type_error(format!(
                  "private key does not allow {} to be used, expected {}",
                  digest_type, hash_algorithm.as_str()
                )));
              }
            }
            if let Some(salt_length) = salt_length {
              rsa::pss::Pss::new_with_salt::<D>(salt_length)
            } else {
              rsa::pss::Pss::new::<D>()
            }
          },
          _ => {
            return Err(type_error(format!(
              "digest not allowed for RSA-PSS signature: {}",
              digest_type
            )))
          }
        );
        let signature = pss
          .sign(Some(&mut OsRng), &key.key, digest)
          .map_err(|_| generic_error("failed to sign digest with RSA-PSS"))?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::Dsa(key) => {
        let res = match_fixed_digest!(
          digest_type,
          fn <D>() {
            key.sign_prehashed_rfc6979::<D>(digest)
          },
          _ => {
            return Err(type_error(format!(
              "digest not allowed for RSA signature: {}",
              digest_type
            )))
          }
        );

        let signature =
          res.map_err(|_| generic_error("failed to sign digest with DSA"))?;
        Ok(signature.into())
      }
      AsymmetricPrivateKey::Ec(key) => match key {
        EcPrivateKey::P224(key) => {
          let signing_key = p224::ecdsa::SigningKey::from(key);
          let signature: p224::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| type_error("failed to sign digest"))?;
          Ok(signature.to_der().to_bytes())
        }
        EcPrivateKey::P256(key) => {
          let signing_key = p256::ecdsa::SigningKey::from(key);
          let signature: p256::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| type_error("failed to sign digest"))?;
          Ok(signature.to_der().to_bytes())
        }
        EcPrivateKey::P384(key) => {
          let signing_key = p384::ecdsa::SigningKey::from(key);
          let signature: p384::ecdsa::Signature = signing_key
            .sign_prehash(digest)
            .map_err(|_| type_error("failed to sign digest"))?;
          Ok(signature.to_der().to_bytes())
        }
      },
      AsymmetricPrivateKey::X25519(_) => {
        Err(type_error("x25519 key cannot be used for signing"))
      }
      AsymmetricPrivateKey::Ed25519(_) => Err(type_error(
        "Ed25519 key cannot be used for prehashed signing",
      )),
      AsymmetricPrivateKey::Dh(_) => {
        Err(type_error("DH key cannot be used for signing"))
      }
    }
  }

  pub fn verify_prehashed(
    &self,
    digest_type: &str,
    digest: &[u8],
    signature: &[u8],
  ) -> Result<bool, AnyError> {
    let public_key = self
      .as_public_key()
      .ok_or_else(|| type_error("key is not a public or private key"))?;

    match &*public_key {
      AsymmetricPublicKey::Rsa(key) => {
        let signer = if digest_type == "md5-sha1" {
          rsa::pkcs1v15::Pkcs1v15Sign::new_unprefixed()
        } else {
          match_fixed_digest_with_oid!(
            digest_type,
            fn <D>() {
              rsa::pkcs1v15::Pkcs1v15Sign::new::<D>()
            },
            _ => {
              return Err(type_error(format!(
                "digest not allowed for RSA signature: {}",
                digest_type
              )))
            }
          )
        };

        Ok(signer.verify(key, digest, signature).is_ok())
      }
      AsymmetricPublicKey::RsaPss(key) => {
        let mut hash_algorithm = None;
        let mut salt_length = None;
        match &key.details {
          Some(details) => {
            if details.hash_algorithm != details.mf1_hash_algorithm {
              return Err(type_error(
                "rsa-pss with different mf1 hash algorithm and hash algorithm is not supported",
              ));
            }
            hash_algorithm = Some(details.hash_algorithm);
            salt_length = Some(details.salt_length as usize);
          }
          None => {}
        };
        let pss = match_fixed_digest_with_oid!(
          digest_type,
          fn <D>(algorithm: Option<RsaPssHashAlgorithm>) {
            if let Some(hash_algorithm) = hash_algorithm.take() {
              if Some(hash_algorithm) != algorithm {
                return Err(type_error(format!(
                  "private key does not allow {} to be used, expected {}",
                  digest_type, hash_algorithm.as_str()
                )));
              }
            }
            if let Some(salt_length) = salt_length {
              rsa::pss::Pss::new_with_salt::<D>(salt_length)
            } else {
              rsa::pss::Pss::new::<D>()
            }
          },
          _ => {
            return Err(type_error(format!(
              "digest not allowed for RSA-PSS signature: {}",
              digest_type
            )))
          }
        );
        Ok(pss.verify(&key.key, digest, signature).is_ok())
      }
      AsymmetricPublicKey::Dsa(key) => {
        let signature = dsa::Signature::from_der(signature)
          .map_err(|_| type_error("Invalid DSA signature"))?;
        Ok(key.verify_prehash(digest, &signature).is_ok())
      }
      AsymmetricPublicKey::Ec(key) => match key {
        EcPublicKey::P224(key) => {
          let verifying_key = p224::ecdsa::VerifyingKey::from(key);
          let signature = p224::ecdsa::Signature::from_der(signature)
            .map_err(|_| type_error("Invalid ECDSA signature"))?;
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::P256(key) => {
          let verifying_key = p256::ecdsa::VerifyingKey::from(key);
          let signature = p256::ecdsa::Signature::from_der(signature)
            .map_err(|_| type_error("Invalid ECDSA signature"))?;
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
        EcPublicKey::P384(key) => {
          let verifying_key = p384::ecdsa::VerifyingKey::from(key);
          let signature = p384::ecdsa::Signature::from_der(signature)
            .map_err(|_| type_error("Invalid ECDSA signature"))?;
          Ok(verifying_key.verify_prehash(digest, &signature).is_ok())
        }
      },
      AsymmetricPublicKey::X25519(_) => {
        Err(type_error("x25519 key cannot be used for verification"))
      }
      AsymmetricPublicKey::Ed25519(_) => Err(type_error(
        "Ed25519 key cannot be used for prehashed verification",
      )),
      AsymmetricPublicKey::Dh(_) => {
        Err(type_error("DH key cannot be used for verification"))
      }
    }
  }
}
