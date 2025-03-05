// Copyright 2018-2025 the Deno authors. MIT license.

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
