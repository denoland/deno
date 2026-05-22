// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // `op_crypto_sign_key_sync` is what this PR adds. For HMAC on
  // token / cookie / JWT-sized payloads, the actual HMAC work is in the
  // hundreds of nanoseconds; the async-op + `spawn_blocking` round-trip
  // (~30 us) dominated total wall time. The JS dispatch in
  // `00_crypto.js` selects this sync op for HMAC inputs <= 64 KiB.
  // Bench exercises the op directly so reviewers can see the per-call
  // cost without measuring the async runtime drain step.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
          const { core } = __bootstrap;
          // 32-byte HMAC-SHA256 key (one block worth, common shape).
          const keyBytes = new Uint8Array(32);
          for (let i = 0; i < 32; i++) keyBytes[i] = i + 1;
          const signOpts = {
            key: { type: "secret", data: keyBytes },
            algorithm: "HMAC",
            hash: "SHA-256",
          };

          // Same sizes as the encrypt bench - the small ones are the
          // hot path (JWT/cookie payloads), 16 KiB is the upper end of
          // the routed range.
          globalThis.input16 = new Uint8Array(16).fill(0x41);
          globalThis.input1k = new Uint8Array(1024).fill(0x41);
          globalThis.input16k = new Uint8Array(16384).fill(0x41);

          globalThis.sign16 = () =>
            core.ops.op_crypto_sign_key_sync(signOpts, globalThis.input16);
          globalThis.sign1k = () =>
            core.ops.op_crypto_sign_key_sync(signOpts, globalThis.input1k);
          globalThis.sign16k = () =>
            core.ops.op_crypto_sign_key_sync(signOpts, globalThis.input16k);
        "#
    },],
  );

  vec![
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      Default::default(),
      None,
      Default::default(),
      Default::default(),
    ),
    deno_crypto::deno_crypto::init(None),
    bench_setup::init(),
  ]
}

fn bench_hmac_sha256_sign_sync_16(b: &mut Bencher) {
  bench_js_sync(b, r#"sign16();"#, setup);
}

fn bench_hmac_sha256_sign_sync_1k(b: &mut Bencher) {
  bench_js_sync(b, r#"sign1k();"#, setup);
}

fn bench_hmac_sha256_sign_sync_16k(b: &mut Bencher) {
  bench_js_sync(b, r#"sign16k();"#, setup);
}

benchmark_group!(
  benches,
  bench_hmac_sha256_sign_sync_16,
  bench_hmac_sha256_sign_sync_1k,
  bench_hmac_sha256_sign_sync_16k,
);
bench_or_profile!(benches);
