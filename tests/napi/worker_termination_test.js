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

async function disposeWithBlockedThreadsafeCalls(scenario) {
  const worker = new Worker(
    new URL("./worker_termination_worker.js", import.meta.url),
    { type: "module" },
  );
  let message = new Promise((resolve) => {
    worker.onmessage = (e) => resolve(e.data);
  });
  assertEquals(await message, "ready");
  message = new Promise((resolve) => {
    worker.onmessage = (e) => resolve(e.data);
  });
  worker.postMessage(scenario);
  assertEquals(await message, "blocked");

  // Disposal joins the worker's blocking pool, where the execute callback is
  // waiting for a queue slot that only the stopped JS thread could free.
  let timer;
  try {
    await Promise.race([
      worker[Symbol.asyncDispose](),
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("Worker stop timed out")),
          5000,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

Deno.test(
  "napi worker disposal releases a blocked threadsafe call",
  () => disposeWithBlockedThreadsafeCalls("block_tsfn_queue"),
);

Deno.test(
  "napi worker disposal releases every blocked threadsafe call",
  () => disposeWithBlockedThreadsafeCalls("block_tsfn_queue_two_waiters"),
);
