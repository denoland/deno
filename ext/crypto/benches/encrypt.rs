// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // `op_crypto_encrypt_sync` is what this PR adds: for AES inputs that
  // fit in a few cache lines (the dominant real-world use - JWT/cookie
  // signing payloads), running on the calling thread is strictly
  // cheaper than the async-op + `spawn_blocking` round-trip. The JS
  // dispatch in `00_crypto.js` selects this op for AES below 64 KiB.
  // Bench exercises the op directly so reviewers can see the per-call
  // cost without measuring the async runtime drain step.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
          const { core } = __bootstrap;
          // Static 32-byte AES-256 key + 12-byte GCM nonce.
          const keyBytes = new Uint8Array(32);
          for (let i = 0; i < 32; i++) keyBytes[i] = i + 1;
          const iv = new Uint8Array(12);
          for (let i = 0; i < 12; i++) iv[i] = i;
          const key = { type: "secret", data: keyBytes };

          // 16 B - the absolute hot path for AES (one block). AES-NI
          // throughput is well under 100 ns; dispatch dominates total
          // wall time at this size on the async path.
          globalThis.input16 = new Uint8Array(16).fill(0x41);
          // 1 KiB - typical JWT / sealed-cookie payload.
          globalThis.input1k = new Uint8Array(1024).fill(0x41);
          // 16 KiB - upper end of the routed range.
          globalThis.input16k = new Uint8Array(16384).fill(0x41);

          const optsGcm = {
            key,
            algorithm: "AES-GCM",
            iv,
            additionalData: null,
            length: 256,
            tagLength: 128,
          };
          globalThis.encrypt16 = () =>
            core.ops.op_crypto_encrypt_sync(optsGcm, globalThis.input16);
          globalThis.encrypt1k = () =>
            core.ops.op_crypto_encrypt_sync(optsGcm, globalThis.input1k);
          globalThis.encrypt16k = () =>
            core.ops.op_crypto_encrypt_sync(optsGcm, globalThis.input16k);
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

fn bench_aes_gcm_encrypt_sync_16(b: &mut Bencher) {
  bench_js_sync(b, r#"encrypt16();"#, setup);
}

fn bench_aes_gcm_encrypt_sync_1k(b: &mut Bencher) {
  bench_js_sync(b, r#"encrypt1k();"#, setup);
}

fn bench_aes_gcm_encrypt_sync_16k(b: &mut Bencher) {
  bench_js_sync(b, r#"encrypt16k();"#, setup);
}

benchmark_group!(
  benches,
  bench_aes_gcm_encrypt_sync_16,
  bench_aes_gcm_encrypt_sync_1k,
  bench_aes_gcm_encrypt_sync_16k,
);
bench_or_profile!(benches);
