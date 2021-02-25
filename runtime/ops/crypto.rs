// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_crypto::op_crypto_get_random_values;
use deno_crypto::op_webcrypto_generate_key;
use deno_crypto::rand::rngs::StdRng;
use deno_crypto::rand::SeedableRng;

pub fn init(rt: &mut deno_core::JsRuntime, maybe_seed: Option<u64>) {
  if let Some(seed) = maybe_seed {
    let rng = StdRng::seed_from_u64(seed);
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<StdRng>(rng);
  }
  super::reg_json_sync(
    rt,
    "op_crypto_get_random_values",
    op_crypto_get_random_values,
  );
  super::reg_json_sync(
    rt,
    "op_webcrypto_generate_key",
    op_webcrypto_generate_key,
  );
}
