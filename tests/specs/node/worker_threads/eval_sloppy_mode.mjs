import { Worker } from "node:worker_threads";
import { once } from "node:events";

// Test that eval workers run in sloppy mode (not strict/ESM),
// matching Node.js behavior where eval code runs as CommonJS.
// Libraries like fflate use bare variable assignments (e.g. `u8 = Uint8Array`)
// which only work in sloppy mode.
// See https://github.com/denoland/deno/issues/26739
const worker = new Worker(
  `
// Bare assignment without var/let/const - only valid in sloppy mode
myGlobal = 42;
const { parentPort } = require("worker_threads");
parentPort.postMessage("sloppy mode works, myGlobal=" + myGlobal);
`,
  { eval: true },
);

const [msg] = await once(worker, "message");
console.log(msg);
worker.terminate();
