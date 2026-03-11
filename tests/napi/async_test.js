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
