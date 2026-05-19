// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_async;
use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
        // Short message (token-signing-sized): exercises the sync fast path.
        globalThis.shortMsg = new Uint8Array([
          0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
        ]);
        // 64 KB: at the sync threshold (still small enough that the sync
        // op wins).
        globalThis.medMsg = new Uint8Array(64 * 1024).fill(0x41);
        // 1 MB: exceeds the threshold, exercises the async/spawn_blocking
        // path.
        globalThis.bigMsg = new Uint8Array(1024 * 1024).fill(0x41);
        globalThis.digestSync = (name, data) =>
          Deno.core.ops.op_crypto_subtle_digest_sync(name, data);
        globalThis.digestAsync = (name, data) =>
          Deno.core.ops.op_crypto_subtle_digest(name, data);
      "#
    }],
  );

  vec![
    deno_webidl::deno_webidl::init(),
    deno_crypto::deno_crypto::init(None),
    bench_setup::init(),
  ]
}

fn bench_digest_sync_sha256_short(b: &mut Bencher) {
  // Sync-op fast path: 11-byte SHA-256 with no async-op overhead.
  bench_js_sync(b, r#"digestSync("SHA-256", shortMsg);"#, setup);
}

fn bench_digest_sync_sha1_short(b: &mut Bencher) {
  bench_js_sync(b, r#"digestSync("SHA-1", shortMsg);"#, setup);
}

fn bench_digest_sync_sha256_64k(b: &mut Bencher) {
  // Sync op at the JS-side threshold (64 KB) — measures where switching
  // to spawn_blocking starts to be worth its dispatch.
  bench_js_sync(b, r#"digestSync("SHA-256", medMsg);"#, setup);
}

fn bench_digest_async_sha256_1m(b: &mut Bencher) {
  // Async/spawn_blocking path for an above-threshold buffer.
  bench_js_async(b, r#"await digestAsync("SHA-256", bigMsg);"#, setup);
}

benchmark_group!(
  benches,
  bench_digest_sync_sha256_short,
  bench_digest_sync_sha1_short,
  bench_digest_sync_sha256_64k,
  bench_digest_async_sha256_1m,
);
bench_or_profile!(benches);
