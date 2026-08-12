// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi finalizer runs after gc", () => {
  // Create an external value with a finalizer that sets a flag when called.
  // deno-lint-ignore no-unused-vars
  let ext = lib.test_deferred_finalizer();
  assertEquals(lib.test_deferred_finalizer_check(), false);

  // Drop the reference and trigger GC. rusty_v8's `Weak::with_finalizer`
  // delivers the finalizer in V8's second-pass weak callback, which runs
  // synchronously inside `gc()` once the GC cycle finishes — analogous to
  // Node's `DrainFinalizerQueue` running off a SetImmediate. The finalizer
  // is therefore observable as having run by the time `gc()` returns.
  ext = null;
  globalThis.gc();

  assertEquals(lib.test_deferred_finalizer_check(), true);
});
