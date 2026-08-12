// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/36499.
//
// A threadsafe function repeatedly creates `napi_wrap`ped objects (with a
// native finalizer) and churns allocations so the GC runs while wraps are
// still pending. The background thread outlives the synchronous test body, so
// the test runner both drains the NAPI finalizer queue at worker teardown and
// keeps garbage-collecting during settling. A wrap finalized at teardown must
// not have its finalizer invoked a second time when it is later collected —
// the native finalizer aborts the process if that happens.

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test({
  name: "napi wrap finalizers run at most once (#36499)",
  // The threadsafe function intentionally outlives the test body.
  sanitizeOps: false,
  sanitizeResources: false,
}, () => {
  lib.test_tsfn_wrap_finalizer();
});
