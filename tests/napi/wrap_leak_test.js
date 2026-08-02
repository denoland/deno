// Copyright 2018-2026 the Deno authors. MIT license.

// Test that napi_wrap finalizers run at shutdown even when the wrapped JS
// object is still reachable and the wrapping happens inside a `Deno.test`.
// This mirrors `wrap_leak.js` (which uses `deno run`) and guards against the
// regression where `deno test` tore down the worker without draining the
// NAPI finalizer queue (see #35692).

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi wrap finalizer runs at test worker shutdown", () => {
  // Create an object and wrap it with a native finalizer. Keep the reference
  // alive (in global scope) so GC won't collect it during the test run. The
  // wrap finalizer should still be called when the test worker shuts down,
  // printing the message.
  globalThis._leaked = lib.test_wrap_leak({});
});
