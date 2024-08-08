// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use digest::Digest;
use digest::FixedOutput;
use digest::FixedOutputReset;
use digest::OutputSizeUser;
use digest::Reset;
use digest::Update;
use rand::rngs::OsRng;
use rsa::signature::hazmat::PrehashSigner as _;
use rsa::signature::hazmat::PrehashVerifier as _;
use rsa::traits::SignatureScheme as _;
use sha2::Sha512;
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
      AsymmetricPrivateKey::Ed25519(key) => {
        if !matches!(
          digest_type,
          "rsa-sha512" | "sha512" | "sha512withrsaencryption"
        ) {
          return Err(type_error(format!(
            "digest not allowed for Ed25519 signature: {}",
            digest_type
          )));
        }

        // let mut precomputed_digest = PrecomputedDigest([0; 64]);
        // if digest.len() != precomputed_digest.0.len() {
        //   return Err(type_error("Invalid sha512 digest"));
        // }
        // precomputed_digest.0.copy_from_slice(digest);

        let mut precomputed_digest = Sha512::new();
        Digest::update(&mut precomputed_digest, b"Hello, world!");

        let signature = key
          .sign_prehashed(precomputed_digest, None)
          .map_err(|_| generic_error("failed to sign digest with Ed25519"))?;

        let mut bytes = signature.to_bytes().to_vec();
        for byte in bytes.iter_mut() {
          *byte = byte.swap_bytes();
        }

        Ok(bytes.into_boxed_slice())
      }
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
      AsymmetricPublicKey::Ed25519(key) => {
        if !matches!(
          digest_type,
          "rsa-sha512" | "sha512" | "sha512withrsaencryption"
        ) {
          return Err(type_error(format!(
            "digest not allowed for Ed25519 signature: {}",
            digest_type
          )));
        }

        let mut signature_fixed = [0u8; 64];
        if signature.len() != signature_fixed.len() {
          return Err(type_error("Invalid Ed25519 signature"));
        }
        signature_fixed.copy_from_slice(signature);

        let signature = ed25519_dalek::Signature::from_bytes(&signature_fixed);

        let mut precomputed_digest = PrecomputedDigest([0; 64]);
        precomputed_digest.0.copy_from_slice(digest);

        Ok(
          key
            .verify_prehashed_strict(precomputed_digest, None, &signature)
            .is_ok(),
        )
      }
      AsymmetricPublicKey::Dh(_) => {
        Err(type_error("DH key cannot be used for verification"))
      }
    }
  }
}

struct PrecomputedDigest([u8; 64]);

impl OutputSizeUser for PrecomputedDigest {
  type OutputSize = <sha2::Sha512 as OutputSizeUser>::OutputSize;
}

impl Digest for PrecomputedDigest {
  fn new() -> Self {
    unreachable!()
  }

  fn new_with_prefix(_data: impl AsRef<[u8]>) -> Self {
    unreachable!()
  }

  fn update(&mut self, _data: impl AsRef<[u8]>) {
    unreachable!()
  }

  fn chain_update(self, _data: impl AsRef<[u8]>) -> Self {
    unreachable!()
  }

  fn finalize(self) -> digest::Output<Self> {
    self.0.into()
  }

  fn finalize_into(self, _out: &mut digest::Output<Self>) {
    unreachable!()
  }

  fn finalize_reset(&mut self) -> digest::Output<Self>
  where
    Self: digest::FixedOutputReset,
  {
    unreachable!()
  }

  fn finalize_into_reset(&mut self, _out: &mut digest::Output<Self>)
  where
    Self: digest::FixedOutputReset,
  {
    unreachable!()
  }

  fn reset(&mut self)
  where
    Self: digest::Reset,
  {
    unreachable!()
  }

  fn output_size() -> usize {
    unreachable!()
  }

  fn digest(_data: impl AsRef<[u8]>) -> digest::Output<Self> {
    unreachable!()
  }
}

impl Reset for PrecomputedDigest {
  fn reset(&mut self) {
    unreachable!()
  }
}

impl FixedOutputReset for PrecomputedDigest {
  fn finalize_into_reset(&mut self, _out: &mut digest::Output<Self>) {
    unreachable!()
  }
}

impl FixedOutput for PrecomputedDigest {
  fn finalize_into(self, _out: &mut digest::Output<Self>) {
    unreachable!()
  }
}

impl Update for PrecomputedDigest {
  fn update(&mut self, _data: &[u8]) {
    unreachable!()
  }
}
