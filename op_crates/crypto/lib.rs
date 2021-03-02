// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//#![deny(warnings)]

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use rand::rngs::OsRng;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use ring::agreement::Algorithm as RingAlgorithm;
use ring::agreement::EphemeralPrivateKey;
use ring::hmac::Key as HmacKey;
use ring::hmac::Algorithm as HmacAlgorithm;
use ring::rand as RingRand;
use ring::signature::EcdsaKeyPair;
use ring::signature::EcdsaSigningAlgorithm;
use rsa::padding::PaddingScheme;
use rsa::BigUint;
use rsa::RSAPrivateKey;
use rsa::RSAPublicKey;
use std::path::PathBuf;

pub use rand; // Re-export rand

mod key;

use crate::key::Algorithm;
use crate::key::CryptoKeyPair;
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
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  assert_eq!(zero_copy.len(), 1);
  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  if let Some(seeded_rng) = maybe_seeded_rng {
    seeded_rng.fill(&mut *zero_copy[0]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy[0]);
  }

  Ok(json!({}))
}

struct CryptoKeyPairResource<A, B> {
  crypto_key: WebCryptoKeyPair,
  key: CryptoKeyPair<A, B>,
}

impl Resource for CryptoKeyPairResource<RSAPublicKey, RSAPrivateKey> {
  fn name(&self) -> Cow<str> {
    "RSACryptoKeyPair".into()
  }
}

impl Resource for CryptoKeyResource<EcdsaKeyPair> {
  fn name(&self) -> Cow<str> {
    "ECDSACryptoKeyPair".into()
  }
}

impl Resource
  for CryptoKeyPairResource<ring::agreement::PublicKey, EphemeralPrivateKey>
{
  fn name(&self) -> Cow<str> {
    "ECDHCryptoKeyPair".into()
  }
}

struct CryptoKeyResource<K> {
  crypto_key: WebCryptoKey,
  key: K,
}

impl Resource for CryptoKeyResource<HmacKey> {
  fn name(&self) -> Cow<str> {
    "cryptoKey".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebCryptoAlgorithmArg {
  name: Algorithm,
  public_exponent: Option<u32>,
  modulus_length: Option<u32>,
  hash: Option<WebCryptoHash>,
  length: Option<u32>,
  named_curve: Option<WebCryptoNamedCurve>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebCryptoGenerateKeyArg {
  algorithm: WebCryptoAlgorithmArg,
  extractable: bool,
  key_usages: Vec<KeyUsage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum JSCryptoKey {
  Single(WebCryptoKey),
  Pair(WebCryptoKeyPair),
}

pub async fn op_webcrypto_generate_key(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: WebCryptoGenerateKeyArg = serde_json::from_value(args)?;
  let extractable = args.extractable;
  let algorithm = args.algorithm.name;

  let mut state = state.borrow_mut();

  let (rid, js_key) = match algorithm {
    Algorithm::RsassaPkcs1v15 | Algorithm::RsaPss | Algorithm::RsaOaep => {
      let exp = args.algorithm.public_exponent.unwrap();
      let modulus_length = args.algorithm.modulus_length.unwrap();
      let exponent = BigUint::from(exp);

      // Generate RSA private key based of exponent, bits and Rng.
      let mut rng = OsRng;
      let private_key = RSAPrivateKey::new_with_exp(
        &mut rng,
        modulus_length as usize,
        &exponent,
      )?;
      // Extract public key from private key.
      let public_key = private_key.to_public_key();

      // Create webcrypto keypair.
      let webcrypto_key_public =
        WebCryptoKey::new_public(algorithm.clone(), extractable, vec![]);
      let webcrypto_key_private =
        WebCryptoKey::new_private(algorithm, extractable, vec![]);
      let crypto_key =
        WebCryptoKeyPair::new(webcrypto_key_public, webcrypto_key_private);

      let key = CryptoKeyPair {
        public_key,
        private_key,
      };
      let resource = CryptoKeyPairResource {
        crypto_key: crypto_key.clone(),
        key,
      };
      (
        state.resource_table.add(resource),
        JSCryptoKey::Pair(crypto_key),
      )
    }
    Algorithm::Ecdh => {
      // Determine agreement from algorithm named_curve.
      let agreement: &RingAlgorithm =
        args.algorithm.named_curve.unwrap().into();
      // Generate private key from agreement and ring rng.
      let rng = RingRand::SystemRandom::new();
      let private_key = EphemeralPrivateKey::generate(&agreement, &rng)?;
      // Extract public key.
      let public_key = private_key.compute_public_key()?;
      // Create webcrypto keypair.
      let webcrypto_key_public =
        WebCryptoKey::new_public(algorithm.clone(), extractable, vec![]);
      let webcrypto_key_private =
        WebCryptoKey::new_private(algorithm, extractable, vec![]);
      let crypto_key =
        WebCryptoKeyPair::new(webcrypto_key_public, webcrypto_key_private);

      let key = CryptoKeyPair {
        public_key,
        private_key,
      };
      let resource = CryptoKeyPairResource {
        crypto_key: crypto_key.clone(),
        key,
      };

      (
        state.resource_table.add(resource),
        JSCryptoKey::Pair(crypto_key),
      )
    }
    Algorithm::Ecdsa => {
      let curve: &EcdsaSigningAlgorithm =
        args.algorithm.named_curve.unwrap().into();

      let rng = RingRand::SystemRandom::new();
      let pkcs8 = EcdsaKeyPair::generate_pkcs8(curve, &rng)?;
      let private_key = EcdsaKeyPair::from_pkcs8(&curve, pkcs8.as_ref())?;
      // Create webcrypto keypair.
      let webcrypto_key_public =
        WebCryptoKey::new_public(algorithm.clone(), extractable, vec![]);
      let crypto_key =
        WebCryptoKey::new_private(algorithm, extractable, vec![]);

      let resource = CryptoKeyResource {
        crypto_key: crypto_key.clone(),
        key: private_key,
      };

      (
        state.resource_table.add(resource),
        JSCryptoKey::Single(crypto_key),
      )
    }
    Algorithm::Hmac => {
      let hash: HmacAlgorithm = args.algorithm.hash.unwrap().into();
      let rng = RingRand::SystemRandom::new();
      // TODO: change algorithm length when specified.
      let key = HmacKey::generate(hash, &rng)?;
      let crypto_key = WebCryptoKey::new_secret(algorithm, extractable, vec![]);
      let resource = CryptoKeyResource {
        crypto_key: crypto_key.clone(),
        key,
      };
      (
        state.resource_table.add(resource),
        JSCryptoKey::Single(crypto_key),
      )
    }
    _ => return Ok(json!({})),
  };

  Ok(json!({ "rid": rid, "key": js_key }))
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
  zero_copy: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(zero_copy.len(), 1);

  let state = state.borrow();
  let args: WebCryptoSignArg = serde_json::from_value(args)?;
  let data = &*zero_copy[0];
  let algorithm = args.algorithm;

  let signature = match algorithm {
    Algorithm::RsassaPkcs1v15 => {
      let resource = state
        .resource_table
        .get::<CryptoKeyPairResource<RSAPublicKey, RSAPrivateKey>>(args.rid)
        .ok_or_else(bad_resource_id)?;

      let private_key = &resource.key.private_key;
      // TODO(littledivy): Modify resource to store args from generateKey.
      // let hash = resource.crypto_key.private_key.hash;
      let padding = PaddingScheme::PKCS1v15Sign { hash: None };

      // Sign data based on computed padding and return buffer
      private_key.sign(padding, &data)?
    }
    Algorithm::Ecdsa => {
      let resource = state
        .resource_table
        .get::<CryptoKeyResource<EcdsaKeyPair>>(args.rid)
        .ok_or_else(bad_resource_id)?;
      let key_pair = &resource.key;

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

      let signature = ring::hmac::sign(&key, &data);
      signature.as_ref().to_vec()
    }
    _ => panic!(), // TODO: don't panic
  };

  Ok(json!({ "data": signature }))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
