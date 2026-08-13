// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi finalizer runs after gc", async () => {
  // Create an external value with a finalizer that sets a flag when called.
  // deno-lint-ignore no-unused-vars
  let ext = lib.test_deferred_finalizer();
  assertEquals(lib.test_deferred_finalizer_check(), false);

  // Drop the reference and trigger GC. The finalizer is delivered in V8's
  // second-pass weak callback, where JavaScript execution is still forbidden,
  // so — like Node's `DrainFinalizerQueue` off a SetImmediate — we defer it to
  // the next JS-safe point rather than running it during GC. It is therefore
  // observable once the event loop turns, not necessarily the instant `gc()`
  // returns.
  ext = null;
  globalThis.gc();

  for (let i = 0; i < 100 && !lib.test_deferred_finalizer_check(); i++) {
    globalThis.gc();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  assertEquals(lib.test_deferred_finalizer_check(), true);
});

// Regression test for #36568: a Node-API finalizer is allowed to call back
// into JavaScript. Running it in V8's GC weak callback (inside a
// DisallowJavascriptExecutionScope) aborts the process, so the finalizer must
// be deferred to a point where JS execution is legal, as Node does.
Deno.test("napi finalizer can call into JS", async () => {
  let ran = false;
  (function () {
  lib.test_external_finalizer_calls_js(() => {
    ran = true;
  });
  })();

  for (let i = 0; i < 100 && !ran; i++) {
    globalThis.gc();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  assert(ran);
});
