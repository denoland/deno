// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const uv = loadTestLibrary();

Deno.test({
  name: "napi uv async",
  ignore: true,
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_async((value) => {
        called = true;
        if (value === 5) {
          resolve();
        }
      });
    });
    assertEquals(called, true);
  },
});

Deno.test({
  name: "napi uv async keeps event loop alive",
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_async_ref(() => {
        called = true;
        resolve();
      });
    });
    assertEquals(called, true);
  },
});

// Exercises the uv polyfills added for native addons that link directly
// against libuv (e.g. @sentry/profiling-node). The Rust side asserts that
// uv_hrtime, uv_timer_*, uv_cpu_info, uv_handle_*, uv_default_loop,
// uv_ref/unref, and uv_is_active/closing are all resolvable and behave as
// expected. If any of these symbols are missing from the deno binary, the
// addon would fail to load and this test would error.
Deno.test({
  name: "napi uv polyfills (hrtime, timer stub, cpu_info, handle helpers)",
  fn: () => {
    uv.test_uv_polyfills();
  },
});
