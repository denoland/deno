// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const asyncTask = loadTestLibrary();

Deno.test("napi async task schedule", async () => {
  let called = false;
  await new Promise((resolve) => {
    asyncTask.test_async_work(() => {
      called = true;
      resolve();
    });
  });
  assertEquals(called, true);
});

Deno.test("napi async work with threadsafe function from execute", async () => {
  // This tests that the execute callback runs on a worker thread by calling
  // a threadsafe function from it. Previously this would deadlock because
  // execute ran on the main thread.
  let called = false;
  await new Promise((resolve) => {
    asyncTask.test_async_work_with_tsfn(() => {
      called = true;
      resolve();
    });
  });
  assertEquals(called, true);
});

Deno.test("napi tsfn call_js_cb receives valid env after close race", async () => {
  // Reproduces a crash seen with node-pty: when a tsfn is released while
  // calls are still pending, the call_js_cb must receive a valid env (not
  // null). The native test spawns two threads (one calling the tsfn and
  // one releasing it) to trigger the race. The call_js_cb asserts env is
  // not null and uses it, which would SIGSEGV before the fix.
  await new Promise((resolve) => {
    asyncTask.test_tsfn_close_race(() => {
      resolve();
    });
  });
});

Deno.test("napi tsfn call_js_cb receives valid env after abort race", async () => {
  // Same race as above, but uses napi_tsfn_abort mode to close the tsfn.
  // Abort forces immediate close regardless of thread_count.
  await new Promise((resolve) => {
    asyncTask.test_tsfn_abort_race(() => {
      resolve();
    });
  });
});

Deno.test("napi tsfn acquire and release", async function () {
  await new Promise((resolve) => {
    asyncTask.test_tsfn_acquire_release(resolve);
  });
});

Deno.test("napi tsfn get context", function () {
  assertEquals(asyncTask.test_tsfn_get_context(), true);
});

Deno.test("napi cancel async work", async function () {
  const result = await new Promise((resolve) => {
    asyncTask.test_cancel_async_work((cancelled) => {
      resolve(cancelled);
    });
  });
  assertEquals(result, true);
});
