// Test that parentPort is isolated from globalThis, so that overriding
// globalThis.postMessage (as Emscripten does) doesn't cause infinite
// recursion, and that messages aren't delivered twice when both
// parentPort.on('message') and self.onmessage are set.
// Regression test for https://github.com/denoland/deno/issues/17171

import { Worker } from "node:worker_threads";

// Test 1: Overriding globalThis.postMessage doesn't break parentPort.postMessage
{
  const worker = new Worker(
    `
    const { parentPort } = require("worker_threads");

    // Simulate what Emscripten does: override globalThis.postMessage
    // to call parentPort.postMessage (would cause infinite recursion
    // if parentPort === globalThis).
    Object.assign(globalThis, {
      postMessage: (msg) => parentPort.postMessage(msg),
    });

    parentPort.postMessage("post_message_ok");
    `,
    { eval: true },
  );

  const msg = await new Promise((resolve, reject) => {
    worker.on("message", resolve);
    worker.on("error", reject);
  });
  console.log("postMessage isolation:", msg);
  await worker.terminate();
}

// Test 2: No double message delivery when both parentPort.on('message')
// and self.onmessage are used (as Emscripten does).
{
  const worker = new Worker(
    `
    const { parentPort } = require("worker_threads");

    let messageCount = 0;

    // Register Node-style listener (like Emscripten does)
    parentPort.on("message", (msg) => {
      messageCount++;
      // Use setTimeout to let any duplicate delivery happen first
      setTimeout(() => {
        parentPort.postMessage("count:" + messageCount);
      }, 100);
    });

    // Also set self.onmessage (like Emscripten does for web compat)
    globalThis.onmessage = (ev) => {
      messageCount++;
    };
    `,
    { eval: true },
  );

  // Send a message and check it's only received once
  worker.postMessage("hello");
  const result = await new Promise((resolve, reject) => {
    worker.on("message", resolve);
    worker.on("error", reject);
  });
  console.log("no double delivery:", result);
  await worker.terminate();
}
