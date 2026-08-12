// Test for https://github.com/denoland/deno/issues/35369
// `process.on('workerMessage', (value, source) => ...)` must receive the
// sender's thread id as `source`, matching Node.js. A two-thread ping/pong:
// the worker posts to thread 0 (main), and main echoes back using `source`.
import process from "node:process";
import {
  postMessageToThread,
  threadId,
  Worker,
  workerData,
} from "node:worker_threads";

if ((workerData?.level ?? 0) === 0) {
  // Main thread: echo every message back to whoever sent it.
  process.on("workerMessage", (value, source) => {
    console.log(`${source} -> ${threadId}:`, value);
    postMessageToThread(source, { message: "pong" });
  });

  const worker = new Worker(new URL(import.meta.url), {
    workerData: { level: 1 },
  });
  worker.on("exit", () => process.exit(0));
} else {
  // Worker thread: ping thread 0, then verify the echoed `source`.
  process.on("workerMessage", (value, source) => {
    console.log(`${source} -> ${threadId}:`, value);
    process.exit(0);
  });

  await postMessageToThread(0, { message: "ping" });
}
