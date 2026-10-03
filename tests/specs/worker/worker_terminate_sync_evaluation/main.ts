// Copyright 2018-2026 the Deno authors. MIT license.

import { Worker as NodeWorker } from "node:worker_threads";

if (Deno.args[0] === "node") {
  const worker = new NodeWorker(
    `require("node:worker_threads").parentPort.postMessage("ready"); while (true) {}`,
    { eval: true },
  );
  await new Promise<void>((resolve) => worker.once("message", () => resolve()));
  await worker.terminate();
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
