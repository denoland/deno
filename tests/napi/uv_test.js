// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

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
//
// The callback must also fire *near* its 5ms deadline. The event loop has to
// arm a wakeup for the next uv timer deadline; without it, the timer only
// fires when some unrelated event happens to wake the loop, which regressed
// this to a ~30s delay (see #36454). Assert a generous upper bound that is
// still far below that stalled behavior so a regression fails loudly instead
// of merely running slowly.
Deno.test({
  name: "napi uv timer callback fires",
  fn: async () => {
    let called = false;
    const start = performance.now();
    await new Promise((resolve) => {
      uv.test_uv_timer_fires(() => {
        called = true;
        resolve();
      });
    });
    const elapsed = performance.now() - start;
    assertEquals(called, true);
    assert(
      elapsed < 5000,
      `uv timer fired after ${
        elapsed.toFixed(0)
      }ms, expected it near its 5ms deadline`,
    );
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

function uvPollTest(name, run) {
  Deno.test({
    name,
    ignore: Deno.build.os === "windows",
    fn: async () => {
      const passed = await new Promise((resolve) => run(resolve));
      assertEquals(passed, true);
    },
  });
}

Deno.test({
  name: "napi uv poll init sets fd nonblocking",
  ignore: Deno.build.os === "windows",
  fn: () => uv.test_uv_poll_init_sets_nonblocking(),
});

uvPollTest("napi uv poll reports actual writable events", (done) => {
  uv.test_uv_poll_reports_actual_writable_events(done);
});

uvPollTest("napi uv poll dispatches hangup-only readiness", (done) => {
  uv.test_uv_poll_dispatches_hangup_only(done);
});

Deno.test({
  name: "napi uv poll reports peer disconnect",
  ignore: Deno.build.os !== "linux",
  async fn() {
    const passed = await new Promise((resolve) => {
      uv.test_uv_poll_reports_disconnect(resolve);
    });
    assertEquals(passed, true);
  },
});

uvPollTest("napi uv poll reports invalid fd error", (done) => {
  uv.test_uv_poll_invalid_fd_reports_ebadf(done);
});

uvPollTest("napi uv poll repeats while fd remains readable", (done) => {
  uv.test_uv_poll_repeats_while_readable(done);
});

uvPollTest("napi uv poll stop suppresses subsequent callback", (done) => {
  uv.test_uv_poll_stop_suppresses_ready_callback(done);
});

uvPollTest("napi uv poll applies callback back-pressure", (done) => {
  uv.test_uv_poll_does_not_flood_callbacks(done);
});

uvPollTest("napi uv poll restart replaces active watch", (done) => {
  uv.test_uv_poll_restart_replaces_watch(done);
});

uvPollTest("napi uv poll allows one active handle per fd", (done) => {
  uv.test_uv_poll_allows_one_active_handle_per_fd(done);
});

uvPollTest("napi uv poll close suppresses subsequent callback", (done) => {
  uv.test_uv_poll_close_suppresses_ready_callback(done);
});
