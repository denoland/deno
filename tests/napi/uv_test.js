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

Deno.test({
  name: "napi uv async close cancels pending send",
  fn: async () => {
    let closed = false;
    await new Promise((resolve) => {
      uv.test_uv_async_close_after_send(() => {
        closed = true;
        resolve();
      });
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    assertEquals(closed, true);
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

// uv_timer scheduled by a NAPI addon must fire on the deno event loop —
// the ext/napi uv_timer_* polyfills bridge onto deno_core's uv_compat
// layer, the same layer driving Node-compat timers on top of tokio. This
// is what unblocks addons like @sentry/profiling-node, which uses a
// repeating uv_timer for periodic measurement ticks.
Deno.test({
  name: "napi uv timer callback fires",
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_timer_fires(() => {
        called = true;
        resolve();
      });
    });
    assertEquals(called, true);
  },
});

// Exercises native addons that schedule main-thread callbacks with libuv's
// check/idle handles and queue background work through uv_queue_work. ZeroMQ
// uses this path when constructing sockets and loading its addon.
Deno.test({
  name: "napi uv loop helpers",
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_loop_helpers(() => {
        called = true;
        resolve();
      });
    });
    assertEquals(called, true);
  },
});

// Exercises the uv_thread_* / uv_sem_* polyfills end to end: a worker
// thread increments a counter and posts a counting semaphore three times
// while the main thread drains the semaphore and joins the worker. If any
// of these symbols are missing from the deno binary the addon would fail
// to load and this test would error.
Deno.test({
  name: "napi uv thread + semaphore",
  fn: () => {
    uv.test_uv_threads();
  },
});

// Exercises the uv_cond_* polyfills end to end: a worker thread sets a
// predicate under the mutex and signals a condition variable the main thread
// is waiting on, then uv_cond_timedwait is checked to time out. If any of
// these symbols are missing from the deno binary the addon would fail to load
// and this test would error.
Deno.test({
  name: "napi uv condition variable",
  fn: () => {
    uv.test_uv_cond();
  },
});

// Exercises uv_cond_broadcast: several worker threads park in uv_cond_wait on
// the same condition variable and the main thread wakes them all with a single
// broadcast once they are all parked.
Deno.test({
  name: "napi uv condition variable broadcast",
  fn: () => {
    uv.test_uv_cond_broadcast();
  },
});
