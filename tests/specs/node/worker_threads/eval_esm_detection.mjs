import { Worker } from "node:worker_threads";
import { once } from "node:events";
import { strictEqual } from "node:assert";

// Test that eval workers auto-detect ESM vs CJS syntax, matching Node.js:
// - Code with import/export declarations runs as ESM (strict mode)
// - Code without runs as CJS (sloppy mode)
// See https://github.com/denoland/deno/issues/26739

// 1. CJS path: no import/export → sloppy mode (bare assignment works)
const w1 = new Worker(
  `
myGlobal = 42;
const { parentPort } = require("node:worker_threads");
parentPort.postMessage("cjs:" + myGlobal);
`,
  { eval: true },
);
const [msg1] = await once(w1, "message");
strictEqual(msg1, "cjs:42");
w1.terminate();

// 2. ESM path: has import → strict mode (import works)
const w2 = new Worker(
  `
import { parentPort } from "node:worker_threads";
parentPort.postMessage("esm:ok");
`,
  { eval: true },
);
const [msg2] = await once(w2, "message");
strictEqual(msg2, "esm:ok");
w2.terminate();

// 3. ESM path: has import → strict mode (bare assignment throws)
const w3 = new Worker(
  `
import { parentPort } from "node:worker_threads";
try {
  bareVar = 1;
  parentPort.postMessage("strict:no");
} catch (e) {
  parentPort.postMessage("strict:yes");
}
`,
  { eval: true },
);
const [msg3] = await once(w3, "message");
strictEqual(msg3, "strict:yes");
w3.terminate();

console.log("ok");
