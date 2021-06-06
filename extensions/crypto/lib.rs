// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rand::rngs::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;
use ring::digest;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub use rand; // Re-export rand

pub fn init(maybe_seed: Option<u64>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/crypto",
      "00_webidl.js",
      "01_crypto.js",
    ))
    .ops(vec![
      (
        "op_crypto_get_random_values",
        op_sync(op_crypto_get_random_values),
      ),
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
  algorithm_id: i8,
  data: Option<ZeroCopyBuf>,
) -> Result<ZeroCopyBuf, AnyError> {
  let algorithm = match algorithm_id {
    0 => &digest::SHA1_FOR_LEGACY_USE_ONLY,
    1 => &digest::SHA256,
    2 => &digest::SHA384,
    3 => &digest::SHA512,
    _ => panic!("Invalid algorithm id"),
  };

  let input = data.ok_or_else(null_opbuf)?;
  let output = tokio::task::spawn_blocking(move || {
    digest::digest(algorithm, &input).as_ref().to_vec().into()
  })
  .await?;

  Ok(output)
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_crypto.d.ts")
}
