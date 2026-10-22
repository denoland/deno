// Copyright 2018-2026 the Deno authors. MIT license.

import { Worker as NodeWorker } from "node:worker_threads";

if (Deno.args[0] === "node") {
  // Node's terminate() resolves without waiting, so observe the interrupt.
  const counter = new Int32Array(new SharedArrayBuffer(4));
  const worker = new NodeWorker(
    `const { parentPort, workerData } = require("node:worker_threads");
    const counter = new Int32Array(workerData);
    parentPort.postMessage("ready");
    while (true) Atomics.add(counter, 0, 1);`,
    { eval: true, workerData: counter.buffer },
  );
  await new Promise<void>((resolve) => worker.once("message", () => resolve()));
  await worker.terminate();
  await new Promise((resolve) => setTimeout(resolve, 100));
  const count = Atomics.load(counter, 0);
  await new Promise((resolve) => setTimeout(resolve, 50));
  if (Atomics.load(counter, 0) !== count) console.log("still running");
} else {
  const url = URL.createObjectURL(
    new Blob([
      `postMessage("ready"); while (true) {}`,
    ], { type: "application/javascript" }),
  );
  const worker = new Worker(url, { type: "module" });
  await new Promise<void>((resolve) => worker.onmessage = () => resolve());
  await worker[Symbol.asyncDispose]();
  URL.revokeObjectURL(url);
}
console.log("stopped");
