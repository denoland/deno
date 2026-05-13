// Tests for worker_threads.postMessageToThread (Node v22+).
//
// Validates real cross-thread delivery: main posts a message to the worker
// by id, the worker handles it via `process.on("workerMessage", ...)` and
// echoes it back, also via `postMessageToThread`. The promise returned by
// each `postMessageToThread` call resolves once the destination thread's
// listener has run.
//
// Error-path coverage (invalid threadId, same thread, no listener) lives
// in the upstream Node compat test
// `test/parallel/test-worker-messaging-errors-invalid.js`.

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
      postMessageToThread(msg.from, {
        from: threadId,
        echo: msg.value,
      }).catch(() => {});
    }
  });

  parentPort.postMessage({ kind: "ready", id: threadId });
`;

const worker = new Worker(workerSrc, { eval: true });
const [readyMsg] = await once(worker, "message");
assert.strictEqual(readyMsg.kind, "ready");
const workerThreadId = readyMsg.id;
assert.strictEqual(workerThreadId, worker.threadId);

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

await worker.terminate();
