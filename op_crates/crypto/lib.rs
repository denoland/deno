// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryInto;
use std::rc::Rc;

use rand::rngs::OsRng;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use ring::agreement::Algorithm as RingAlgorithm;
use ring::agreement::EphemeralPrivateKey;
use ring::hmac::Algorithm as HmacAlgorithm;
use ring::hmac::Key as HmacKey;
use ring::rand as RingRand;
use ring::signature::EcdsaKeyPair;
use ring::signature::EcdsaSigningAlgorithm;
use rsa::padding::PaddingScheme;
use rsa::BigUint;
use rsa::RSAPrivateKey;
use rsa::RSAPublicKey;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::path::PathBuf;

pub use rand; // Re-export rand

mod error;
mod key;

use crate::error::DomError;
use crate::error::WebCryptoError;
use crate::key::Algorithm;
use crate::key::KeyUsage;
use crate::key::WebCryptoHash;
use crate::key::WebCryptoKey;
use crate::key::WebCryptoKeyPair;
use crate::key::WebCryptoNamedCurve;

/// Execute this crates' JS source files.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![(
    "deno:op_crates/crypto/01_crypto.js",
    include_str!("01_crypto.js"),
  )];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

pub fn op_crypto_get_random_values(
  state: &mut OpState,
  _args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let mut zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  if let Some(seeded_rng) = maybe_seeded_rng {
    seeded_rng.fill(&mut *zero_copy);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy);
  }

  Ok(json!({}))
}

struct CryptoKeyResource<A> {
  crypto_key: WebCryptoKey,
  key: A,
  hash: Option<WebCryptoHash>,
}

// `impl_resource` will use the type name as the resource name.
macro_rules! impl_resource {
  ($($t:ty),+) => {
    $(impl Resource for CryptoKeyResource<$t> {
      fn name(&self) -> Cow<str> {
        stringify!($t).into()
      }
    })*
  }
}

impl_resource! {
  RSAPublicKey,
  RSAPrivateKey,
  EcdsaKeyPair,
  ring::agreement::PublicKey,
  EphemeralPrivateKey,
  HmacKey
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebCryptoAlgorithmArg {
  name: Algorithm,
  modulus_length: Option<u32>,
  hash: Option<WebCryptoHash>,
  #[allow(dead_code)]
  length: Option<u32>,
  named_curve: Option<WebCryptoNamedCurve>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebCryptoGenerateKeyArg {
  algorithm: WebCryptoAlgorithmArg,
  extractable: bool,
  key_usages: Vec<KeyUsage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum JsCryptoKey {
  Single {
    key: WebCryptoKey,
    rid: u32,
  },
  Pair {
    key: WebCryptoKeyPair,
    private_rid: u32,
    public_rid: u32,
  },
}

macro_rules! validate_usage {
  ($e: expr, $u: expr) => {
    for usage in $e {
      if !$u.contains(&usage) {
        return Ok(json!({ "err": DomError("Invalid key usage".to_string()) }))
      }
    }
  }
}

pub async fn op_webcrypto_generate_key(
  state: Rc<RefCell<OpState>>,
  args: WebCryptoGenerateKeyArg,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let extractable = args.extractable;
  let algorithm = args.algorithm.name;

  let mut state = state.borrow_mut();
  let key = match algorithm {
    Algorithm::RsassaPkcs1v15 | Algorithm::RsaPss => {
      validate_usage!(&args.key_usages, vec![KeyUsage::Sign, KeyUsage::Verify]);
      let exp = zero_copy.ok_or_else(|| {
        WebCryptoError::MissingArgument("publicExponent".to_string())
      })?;
      let modulus_length = args.algorithm.modulus_length.ok_or_else(|| {
        WebCryptoError::MissingArgument("modulusLength".to_string())
      })?;

      let exponent = BigUint::from_bytes_be(&exp);

      // Generate RSA private key based of exponent, bits and Rng.
      let mut rng = OsRng;

      let private_key: RSAPrivateKey = tokio::task::spawn_blocking(
        move || -> Result<RSAPrivateKey, rsa::errors::Error> {
          RSAPrivateKey::new_with_exp(
            &mut rng,
            modulus_length as usize,
            &exponent,
          )
        },
      )
      .await
      .unwrap()?;

      // Extract public key from private key.
      let public_key = private_key.to_public_key();

      // Create webcrypto keypair.
      let webcrypto_key_public = WebCryptoKey::new_public(
        algorithm,
        extractable,
        args.key_usages.clone(),
      );
      let webcrypto_key_private =
        WebCryptoKey::new_private(algorithm, extractable, args.key_usages);
      let crypto_key = WebCryptoKeyPair::new(
        webcrypto_key_public.clone(),
        webcrypto_key_private.clone(),
      );

      JsCryptoKey::Pair {
        key: crypto_key,
        private_rid: state.resource_table.add(CryptoKeyResource {
          crypto_key: webcrypto_key_private,
          key: private_key,
          hash: args.algorithm.hash,
        }),
        public_rid: state.resource_table.add(CryptoKeyResource {
          crypto_key: webcrypto_key_public,
          key: public_key,
          hash: args.algorithm.hash,
        }),
      }
    }
    Algorithm::Ecdh => {
      validate_usage!(
        &args.key_usages,
        vec![KeyUsage::DeriveKey, KeyUsage::DeriveBits]
      );

      // Determine agreement from algorithm named_curve.
      let agreement: Result<&RingAlgorithm, WebCryptoError> = args
        .algorithm
        .named_curve
        .ok_or_else(|| {
          WebCryptoError::MissingArgument("namedCurve".to_string())
        })?
        .try_into();
      if agreement.is_err() {
        return Ok(
          json!({ "err": DomError("namedCurve not supported".to_string()) }),
        );
      }
      // Generate private key from agreement and ring rng.
      let rng = RingRand::SystemRandom::new();

      let private_key: EphemeralPrivateKey = tokio::task::spawn_blocking(
        move || -> Result<EphemeralPrivateKey, ring::error::Unspecified> {
          EphemeralPrivateKey::generate(&agreement.unwrap(), &rng)
        },
      )
      .await
      .unwrap()?;

      // Extract public key.
      let public_key = private_key.compute_public_key()?;
      // Create webcrypto keypair.
      let webcrypto_key_public = WebCryptoKey::new_public(
        algorithm,
        extractable,
        args.key_usages.clone(),
      );
      let webcrypto_key_private =
        WebCryptoKey::new_private(algorithm, extractable, args.key_usages);
      let crypto_key = WebCryptoKeyPair::new(
        webcrypto_key_public.clone(),
        webcrypto_key_private.clone(),
      );

      JsCryptoKey::Pair {
        key: crypto_key,
        private_rid: state.resource_table.add(CryptoKeyResource {
          crypto_key: webcrypto_key_private,
          key: private_key,
          hash: args.algorithm.hash,
        }),
        public_rid: state.resource_table.add(CryptoKeyResource {
          crypto_key: webcrypto_key_public,
          key: public_key,
          hash: args.algorithm.hash,
        }),
      }
    }
    Algorithm::Ecdsa => {
      validate_usage!(&args.key_usages, vec![KeyUsage::Sign, KeyUsage::Verify]);

      let curve: Result<&EcdsaSigningAlgorithm, WebCryptoError> = args
        .algorithm
        .named_curve
        .ok_or_else(|| {
          WebCryptoError::MissingArgument("namedCurve".to_string())
        })?
        .try_into();
      if curve.is_err() {
        return Ok(
          json!({ "err": DomError("namedCurve not supported".to_string()) }),
        );
      }
      let rng = RingRand::SystemRandom::new();
      let private_key: EcdsaKeyPair = tokio::task::spawn_blocking(
        move || -> Result<EcdsaKeyPair, ring::error::Unspecified> {
          let curve = curve.unwrap();
          let pkcs8 = EcdsaKeyPair::generate_pkcs8(curve, &rng)?;
          Ok(EcdsaKeyPair::from_pkcs8(&curve, pkcs8.as_ref())?)
        },
      )
      .await
      .unwrap()?;

      // Create webcrypto keypair.
      let webcrypto_key_public = WebCryptoKey::new_public(
        algorithm,
        extractable,
        args.key_usages.clone(),
      );
      let webcrypto_key_private =
        WebCryptoKey::new_private(algorithm, extractable, args.key_usages);
      let crypto_key = WebCryptoKeyPair::new(
        webcrypto_key_public,
        webcrypto_key_private.clone(),
      );

      let rid = state.resource_table.add(CryptoKeyResource {
        crypto_key: webcrypto_key_private,
        key: private_key,
        hash: args.algorithm.hash,
      });

      JsCryptoKey::Pair {
        key: crypto_key,
        private_rid: rid,
        // NOTE: We're using the same Resource for public and private key since they are part
        //       of the same interface in `ring`.
        public_rid: rid,
      }
    }
    Algorithm::Hmac => {
      validate_usage!(&args.key_usages, vec![KeyUsage::Sign, KeyUsage::Verify]);

      let hash: HmacAlgorithm = args
        .algorithm
        .hash
        .ok_or_else(|| WebCryptoError::MissingArgument("hash".to_string()))?
        .into();
      let rng = RingRand::SystemRandom::new();
      let key: HmacKey = tokio::task::spawn_blocking(
        move || -> Result<HmacKey, ring::error::Unspecified> {
          HmacKey::generate(hash, &rng)
        },
      )
      .await
      .unwrap()?;

      let crypto_key =
        WebCryptoKey::new_secret(algorithm, extractable, args.key_usages);
      let resource = CryptoKeyResource {
        crypto_key: crypto_key.clone(),
        key,
        hash: args.algorithm.hash,
      };
      JsCryptoKey::Single {
        key: crypto_key,
        rid: state.resource_table.add(resource),
      }
    }
    _ => return Err(WebCryptoError::Unsupported.into()),
  };

  Ok(json!({ "key": key }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebCryptoSignArg {
  rid: u32,
  algorithm: Algorithm,
  salt_length: Option<u32>,
  hash: Option<WebCryptoHash>,
}

pub async fn op_webcrypto_sign_key(
  state: Rc<RefCell<OpState>>,
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let state = state.borrow();
  let args: WebCryptoSignArg = serde_json::from_value(args)?;
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  let signature = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let resource = state
        .resource_table
        .get::<CryptoKeyResource<RSAPrivateKey>>(args.rid)
        .ok_or_else(bad_resource_id)?;

      let private_key = &resource.key;

      if !resource.crypto_key.usages.contains(&KeyUsage::Sign) {
        return Ok(json!({ "err": DomError("Invalid key usage".to_string()) }));
      }

      let padding = match resource
        .hash
        .ok_or_else(|| WebCryptoError::MissingArgument("hash".to_string()))?
      {
        WebCryptoHash::Sha1 => PaddingScheme::PKCS1v15Sign {
          hash: Some(rsa::hash::Hash::SHA1),
        },
        WebCryptoHash::Sha256 => PaddingScheme::PKCS1v15Sign {
          hash: Some(rsa::hash::Hash::SHA2_256),
        },
        WebCryptoHash::Sha384 => PaddingScheme::PKCS1v15Sign {
          hash: Some(rsa::hash::Hash::SHA2_384),
        },
        WebCryptoHash::Sha512 => PaddingScheme::PKCS1v15Sign {
          hash: Some(rsa::hash::Hash::SHA2_512),
        },
      };

      // Sign data based on computed padding and return buffer
      private_key.sign(padding, &data)?
    }
    Algorithm::RsaPss => {
      let resource = state
        .resource_table
        .get::<CryptoKeyResource<RSAPrivateKey>>(args.rid)
        .ok_or_else(bad_resource_id)?;

      let private_key = &resource.key;

      if !resource.crypto_key.usages.contains(&KeyUsage::Sign) {
        return Ok(json!({ "err": DomError("Invalid key usage".to_string()) }));
      }

      let rng = OsRng;
      let salt_len = args.salt_length.ok_or_else(|| {
        WebCryptoError::MissingArgument("saltLength".to_string())
      })? as usize;

      let (padding, digest_in) = match resource
        .hash
        .ok_or_else(|| WebCryptoError::MissingArgument("hash".to_string()))?
      {
        WebCryptoHash::Sha1 => {
          let mut hasher = Sha1::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha1, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        WebCryptoHash::Sha256 => {
          let mut hasher = Sha256::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha256, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        WebCryptoHash::Sha384 => {
          let mut hasher = Sha384::new();
          hasher.update(&data);
          (
            PaddingScheme::new_pss_with_salt::<Sha384, _>(rng, salt_len),
            hasher.finalize()[..].to_vec(),
          )
        }
        WebCryptoHash::Sha512 => {
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
      let resource = state
        .resource_table
        .get::<CryptoKeyResource<EcdsaKeyPair>>(args.rid)
        .ok_or_else(bad_resource_id)?;
      let key_pair = &resource.key;

      if !resource.crypto_key.usages.contains(&KeyUsage::Sign) {
        return Ok(json!({ "err": DomError("Invalid key usage".to_string()) }));
      }

      // We only support P256-SHA256 & P384-SHA384. These are recommended signature pairs.
      // https://briansmith.org/rustdoc/ring/signature/index.html#statics
      if let Some(hash) = args.hash {
        match hash {
          WebCryptoHash::Sha256 | WebCryptoHash::Sha384 => (),
          _ => return Err(WebCryptoError::UnsupportedHash.into()),
        }
      };

      // Sign data using SecureRng and key.
      let rng = RingRand::SystemRandom::new();
      let signature = key_pair.sign(&rng, &data)?;

      // Signature data as buffer.
      signature.as_ref().to_vec()
    }
    Algorithm::Hmac => {
      let resource = state
        .resource_table
        .get::<CryptoKeyResource<HmacKey>>(args.rid)
        .ok_or_else(bad_resource_id)?;
      let key = &resource.key;

      if !resource.crypto_key.usages.contains(&KeyUsage::Sign) {
        return Ok(json!({ "err": DomError("Invalid key usage".to_string()) }));
      }

      let signature = ring::hmac::sign(&key, &data);
      signature.as_ref().to_vec()
    }
    _ => return Err(WebCryptoError::Unsupported.into()),
  };

  Ok(json!({ "signature": signature }))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
