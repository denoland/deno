// Tests for worker_threads.postMessageToThread (Node v22+).
//
// Exercises real cross-thread delivery on top of the error-path checks
// covered by tests/node_compat/.../test-worker-messaging-errors-invalid.js.

import { postMessageToThread, threadId, Worker } from "node:worker_threads";
import { once } from "node:events";
import process from "node:process";
import assert from "node:assert";

const workerSrc = `
  const { parentPort, threadId, postMessageToThread } =
    require("node:worker_threads");
  const process = require("node:process");

  process.on("workerMessage", (msg) => {
    if (msg && msg.__cmd === "echo") {
      // Reply back to the main thread by id.
      postMessageToThread(msg.from, {
        from: threadId,
        echo: msg.value,
      }).catch((err) => {
        parentPort.postMessage({ kind: "echo_post_failed", err: err.code });
      });
    } else if (msg && msg.__cmd === "throw") {
      throw new Error("handler boom");
    }
  });

  parentPort.postMessage({ kind: "ready", id: threadId });
`;

async function run() {
  const worker = new Worker(workerSrc, { eval: true });
  const [readyMsg] = await once(worker, "message");
  assert.strictEqual(readyMsg.kind, "ready");
  const workerThreadId = readyMsg.id;
  assert.strictEqual(workerThreadId, worker.threadId);

  // 1. Round-trip a message to the worker and back.
  const echoBack = new Promise((resolve) => {
    function onMain(msg) {
      if (msg && msg.from === workerThreadId) {
        process.off("workerMessage", onMain);
        resolve(msg);
      }
    }
    process.on("workerMessage", onMain);
  });
  await postMessageToThread(workerThreadId, {
    __cmd: "echo",
    from: threadId,
    value: "hello",
  });
  const echoed = await echoBack;
  assert.strictEqual(echoed.echo, "hello");
  console.log("round_trip_ok");

  // 2. Listener that throws should reject with ERR_WORKER_MESSAGING_ERRORED.
  await assert.rejects(
    () =>
      postMessageToThread(workerThreadId, {
        __cmd: "throw",
      }),
    { code: "ERR_WORKER_MESSAGING_ERRORED" },
  );
  console.log("errored_ok");

  // 3. Posting to a terminated worker rejects with ERR_WORKER_MESSAGING_FAILED.
  await worker.terminate();
  await assert.rejects(
    () => postMessageToThread(workerThreadId, "after_terminate"),
    { code: "ERR_WORKER_MESSAGING_FAILED" },
  );
  console.log("after_terminate_ok");
}

run().catch((err) => {
  console.error("test failed:", err);
  process.exit(1);
});
