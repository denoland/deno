// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals } from "./common.js";

Deno.test("napi addon survives worker termination", async () => {
  // Spawn a worker that loads the NAPI addon and does work.
  // Terminate it and verify no crash occurs.
  const worker = new Worker(
    new URL("./worker_termination_worker.js", import.meta.url),
    { type: "module" },
  );

  // Wait for the worker to signal it has loaded the addon
  const loaded = await new Promise((resolve) => {
    worker.onmessage = (e) => resolve(e.data);
  });
  assertEquals(loaded, "ready");

  // Terminate the worker while the addon is loaded
  worker.terminate();

  // If we get here without crashing, the test passes.
  // Give a moment for any deferred cleanup/destructor work.
  await new Promise((r) => setTimeout(r, 100));
});

Deno.test("napi external buffer finalizer runs after worker termination", async () => {
  const worker = new Worker(
    new URL("./worker_termination_worker.js", import.meta.url),
    { type: "module" },
  );

  const loaded = await new Promise((resolve) => {
    worker.onmessage = (e) => resolve(e.data);
  });
  assertEquals(loaded, "ready");

  // Ask the worker to create external buffers before we terminate
  worker.postMessage("create_externals");
  const created = await new Promise((resolve) => {
    worker.onmessage = (e) => resolve(e.data);
  });
  assertEquals(created, "created");

  // Terminate -- finalizers for external buffers should not crash
  worker.terminate();
  await new Promise((r) => setTimeout(r, 100));
});
