// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals } from "./common.js";

function waitForMessage(worker, expected) {
  return new Promise((resolve, reject) => {
    worker.onmessage = (event) => {
      if (event.data === expected) {
        resolve();
      } else {
        reject(
          new Error(
            `expected worker message ${JSON.stringify(expected)}, got ${
              JSON.stringify(event.data)
            }`,
          ),
        );
      }
    };
    worker.onerror = (event) => {
      event.preventDefault();
      reject(event.error ?? new Error(event.message));
    };
  });
}

async function testActivePollTerminationProtocol(message) {
  const worker = new Worker(
    new URL("./worker_termination_worker.js", import.meta.url),
    { type: "module" },
  );
  await waitForMessage(worker, "ready");
  worker.postMessage(message);
  await waitForMessage(worker, "armed");
  // This is end-to-end smoke coverage for Web Worker termination. The worker
  // sends "armed" only after registering cleanup for the ready active poll,
  // but this API cannot observe runtime destruction. The Rust unit test
  // `active_napi_poll_teardown_joins_runtime_thread` separately waits for it.
  worker.terminate();
}

// Unix-only: this fixture uses POSIX pipes through the Unix poll bridge.
Deno.test({
  name: "napi uv poll worker termination arms a refed poll",
  ignore: Deno.build.os === "windows",
  sanitizeOps: true,
  sanitizeResources: true,
  async fn() {
    await testActivePollTerminationProtocol("arm_refed_poll");
  },
});

Deno.test({
  name: "napi uv poll worker termination arms an unrefed poll",
  ignore: Deno.build.os === "windows",
  sanitizeOps: true,
  sanitizeResources: true,
  async fn() {
    await testActivePollTerminationProtocol("arm_unrefed_poll");
  },
});

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
