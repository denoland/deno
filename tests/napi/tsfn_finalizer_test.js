// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/36499.
//
// A threadsafe function repeatedly creates `napi_wrap`ped objects (with a
// native finalizer) and churns allocations so the GC runs while wraps are
// still pending. The background thread outlives the synchronous test body, so
// the test runner both drains the NAPI finalizer queue at worker teardown and
// keeps garbage-collecting during settling. A wrap finalized at teardown must
// not have its finalizer invoked a second time when it is later collected.
//
// HOW A REGRESSION LOOKS: the native finalizer detects the second invocation
// and calls `abort()`, so the whole napi test process dies with SIGABRT after
// printing "FATAL: napi_wrap finalizer invoked twice". That is a real
// double-free, not a flaky test — see tests/napi/src/tsfn_finalizer.rs.
//
// The repro is timing dependent (it needs a GC to land in the window between
// teardown and collection), so it does not catch every regression on every
// run. It does not produce false positives: nothing but a genuine double
// invocation can trip the abort.

import { assert, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test({
  name: "napi wrap finalizers run at most once (#36499)",
  // The threadsafe function intentionally outlives the test body.
  sanitizeOps: false,
  sanitizeResources: false,
}, async () => {
  lib.test_tsfn_wrap_finalizer();

  // Wait for the threadsafe function to actually start dispatching, otherwise
  // the test would pass without exercising anything. The remaining ticks still
  // outlive the test body, which is what the repro needs.
  const deadline = Date.now() + 10_000;
  let stats = lib.get_tsfn_wrap_finalizer_stats();
  while (stats.wrapped === 0 && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 10));
    stats = lib.get_tsfn_wrap_finalizer_stats();
  }

  assert(
    stats.wrapped > 0,
    "threadsafe function never dispatched; the repro did not run",
  );
});
