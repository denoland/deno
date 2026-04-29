// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi deferred finalizer runs after gc, not during", async () => {
  // Create an external value with a finalizer that sets a flag when called.
  // deno-lint-ignore no-unused-vars
  let ext = lib.test_deferred_finalizer();
  assertEquals(lib.test_deferred_finalizer_check(), false);

  // Drop the reference and trigger GC.
  ext = null;
  globalThis.gc();

  // The finalizer should be deferred — not yet called synchronously after gc().
  assertEquals(lib.test_deferred_finalizer_check(), false);

  // Yield to the event loop so the deferred finalizer can run.
  await new Promise((resolve) => setTimeout(resolve, 0));

  // Now the finalizer should have run.
  assertEquals(lib.test_deferred_finalizer_check(), true);
});
