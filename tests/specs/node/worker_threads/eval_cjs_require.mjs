import { Worker } from "node:worker_threads";
import { once } from "node:events";

// Test that eval workers can use require() (CJS globals),
// matching Node.js behavior where eval code runs as CommonJS.
// See https://github.com/denoland/deno/issues/27181
const worker = new Worker(
  `
const { parentPort } = require("worker_threads");
parentPort.postMessage("ok");
`,
  { eval: true },
);

const [msg] = await once(worker, "message");
console.log(msg);
worker.terminate();
