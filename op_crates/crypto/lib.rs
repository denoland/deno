// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::serde::Deserialize;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub use rand; // Re-export rand

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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
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

#[derive(Deserialize)]
#[serde(tag = "name")]
enum DigestAlgorithmName {
  #[serde(rename = "SHA-1")]
  SHA1 = 0,
  #[serde(rename = "SHA-256")]
  SHA256 = 1,
  #[serde(rename = "SHA-384")]
  SHA384 = 2,
  #[serde(rename = "SHA-512")]
  SHA512 = 3,
}

static DIGEST_ALGORITHMS: [&ring::digest::Algorithm; 4] = [
  &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
  &ring::digest::SHA256,
  &ring::digest::SHA384,
  &ring::digest::SHA512,
];

pub async fn op_crypto_subtle_digest(
  _state: Rc<RefCell<OpState>>,
  args: Value,
  zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let mut zc = zero_copy;

  let alg: DigestAlgorithmName = serde_json::from_value(args)?;
  let digest_algorithm = DIGEST_ALGORITHMS[alg as usize];

  tokio::task::spawn_blocking(move || {
    let digest = ring::digest::digest(digest_algorithm, &zc[0]);
    // The buffer is allocated on the JS side, where we need to know in advance
    // the byte length of the output for a given digest algorithm.
    // This asserts that the anticipated length is equal to the actual length.
    assert_eq!(digest.algorithm().output_len, zc[1].as_ref().len());
    zc[1].copy_from_slice(digest.as_ref());
  }).await?;

  Ok(Value::Null)
}
